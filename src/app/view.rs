//! Per-file view: diff plus row cursor, active side, scroll and cached row layout.

use crate::comments::model::Side;
use crate::diff::model::{FileDiff, LineKind, SideText};
use crate::ui::diff_view::columns;
use crate::vim::motion::Motion;
use crate::vim::wrap;

const SCROLL_MARGIN: usize = 3;

pub struct FileView {
    pub diff: FileDiff,
    pub side: Side,
    /// Side last chosen with `h`/`l`; hunk jumps prefer it.
    home: Side,
    /// Cursor row index into `diff.rows`.
    pub row: usize,
    /// First visible row.
    pub scroll: usize,
    layout: Layout,
}

#[derive(Default)]
struct Layout {
    /// (total width, wrap) the heights were computed for.
    key: Option<(u16, bool)>,
    /// Screen line where each row starts; one extra entry for the end.
    tops: Vec<usize>,
}

impl FileView {
    /// Home side is new unless the file is deleted; the cursor starts on the first hunk.
    pub fn new(diff: FileDiff) -> Self {
        let side = if diff.rhs.is_some() {
            Side::New
        } else {
            Side::Old
        };
        let mut view = Self {
            diff,
            side,
            home: side,
            row: 0,
            scroll: 0,
            layout: Layout::default(),
        };
        match view.targets().first() {
            Some(&(row, side)) => (view.row, view.side) = (row, side),
            None => view.row = view.nearest(0).unwrap_or(0),
        }
        view
    }

    /// 0-based line of `side` in `row`; `None` for padding.
    pub fn line(&self, row: usize, side: Side) -> Option<u32> {
        let row = self.diff.rows.get(row)?;
        match side {
            Side::Old => row.lhs,
            Side::New => row.rhs,
        }
    }

    pub fn cursor_line(&self) -> Option<u32> {
        self.line(self.row, self.side)
    }

    /// Screen line where each row starts, plus the end; valid after `refresh_layout`.
    pub fn tops(&self) -> &[usize] {
        &self.layout.tops
    }

    /// Row-mode vertical motions; others are ignored.
    pub fn motion(&mut self, motion: Motion, count: Option<usize>, height: usize) {
        let n = count.unwrap_or(1);
        let half = (height / 2).max(1);
        match motion {
            Motion::Down => self.step(n, true),
            Motion::Up => self.step(n, false),
            Motion::HalfPageDown => self.step(half, true),
            Motion::HalfPageUp => self.step(half, false),
            Motion::FileStart | Motion::FileEnd if count.is_some() => self.goto_line(n),
            Motion::FileStart => self.row = self.nearest(0).unwrap_or(self.row),
            Motion::FileEnd => {
                let last = self.diff.rows.len().saturating_sub(1);
                self.row = self.nearest(last).unwrap_or(self.row);
            }
            _ => {}
        }
    }

    /// Makes `side` active and moves to the nearest row with a line there; no-op if the side is absent.
    pub fn switch(&mut self, side: Side) {
        let present = match side {
            Side::Old => self.diff.lhs.is_some(),
            Side::New => self.diff.rhs.is_some(),
        };
        if !present {
            return;
        }
        self.side = side;
        self.home = side;
        if let Some(row) = self.nearest(self.row) {
            self.row = row;
        }
    }

    /// Moves `count` hunks, switching side if the hunk changes only the other one; `false` if
    /// there is no hunk in that direction.
    pub fn hunk(&mut self, next: bool, count: usize) -> bool {
        let targets = self.targets();
        let targets: Vec<_> = if next {
            targets.into_iter().filter(|&(r, _)| r > self.row).collect()
        } else {
            (targets.into_iter().rev())
                .filter(|&(r, _)| r < self.row)
                .collect()
        };
        let Some(&target) = targets.get(count.clamp(1, targets.len().max(1)) - 1) else {
            return false;
        };
        (self.row, self.side) = target;
        true
    }

    /// Recomputes row heights if the width or wrap changed, then scrolls the cursor into view.
    pub fn refresh_layout(&mut self, width: u16, height: usize, wrap: bool) {
        if self.layout.key != Some((width, wrap)) {
            let [l, r] = columns(&self.diff, width);
            let mut tops = Vec::with_capacity(self.diff.rows.len() + 1);
            let mut top = 0;
            tops.push(top);
            for row in &self.diff.rows {
                top += if wrap {
                    height_of(&self.diff.lhs, row.lhs, l.text).max(height_of(
                        &self.diff.rhs,
                        row.rhs,
                        r.text,
                    ))
                } else {
                    1
                };
                tops.push(top);
            }
            self.layout = Layout {
                key: Some((width, wrap)),
                tops,
            };
        }
        self.keep_visible(height);
    }

    fn has_line(&self, row: usize) -> bool {
        self.line(row, self.side).is_some()
    }

    fn step(&mut self, n: usize, down: bool) {
        for _ in 0..n {
            let next = if down {
                (self.row + 1..self.diff.rows.len()).find(|&r| self.has_line(r))
            } else {
                (0..self.row).rev().find(|&r| self.has_line(r))
            };
            match next {
                Some(row) => self.row = row,
                None => break,
            }
        }
    }

    /// 1-based line `n` on the active side, or the last line past the end.
    fn goto_line(&mut self, n: usize) {
        let target = n.saturating_sub(1) as u32;
        let rows = 0..self.diff.rows.len();
        let found = rows
            .clone()
            .find(|&r| self.line(r, self.side).is_some_and(|l| l >= target))
            .or_else(|| rows.rev().find(|&r| self.has_line(r)));
        self.row = found.unwrap_or(self.row);
    }

    /// Nearest row with a line on the active side; ties go up.
    fn nearest(&self, row: usize) -> Option<usize> {
        let up = (0..=row).rev().find(|&r| self.has_line(r));
        let down = (row..self.diff.rows.len()).find(|&r| self.has_line(r));
        match (up, down) {
            (Some(u), Some(d)) => Some(if row - u <= d - row { u } else { d }),
            (u, d) => u.or(d),
        }
    }

    /// Per hunk, its first changed row on the home side, else on the other side (pure
    /// additions/deletions). Rows strictly increase, as hunks don't overlap.
    fn targets(&self) -> Vec<(usize, Side)> {
        let other = match self.home {
            Side::Old => Side::New,
            Side::New => Side::Old,
        };
        (self.diff.hunks.iter())
            .filter_map(|h| {
                [self.home, other].into_iter().find_map(|side| {
                    let row = h.clone().find(|&r| self.changed(r, side))?;
                    Some((row, side))
                })
            })
            .collect()
    }

    fn changed(&self, row: usize, side: Side) -> bool {
        let text = match side {
            Side::Old => &self.diff.lhs,
            Side::New => &self.diff.rhs,
        };
        match (text, self.line(row, side)) {
            (Some(text), Some(i)) => text.kinds[i as usize] != LineKind::Unchanged,
            _ => false,
        }
    }

    /// Keeps the cursor row `SCROLL_MARGIN` screen lines from the edges, where rows exist there.
    fn keep_visible(&mut self, height: usize) {
        let tops = &self.layout.tops;
        if self.diff.rows.is_empty() {
            self.scroll = 0;
            return;
        }
        let margin = SCROLL_MARGIN.min(height / 2);
        let (top, bottom) = (tops[self.row], tops[self.row + 1]);
        let up = top.saturating_sub(margin);
        if tops[self.scroll] > up {
            self.scroll = tops.partition_point(|&t| t <= up) - 1;
        }
        let below = margin.min(tops[tops.len() - 1] - bottom);
        let need = (bottom + below).saturating_sub(height);
        if tops[self.scroll] < need {
            // A row taller than the viewport still starts on screen.
            self.scroll = tops.partition_point(|&t| t < need).min(self.row);
        }
    }
}

fn height_of(side: &Option<SideText>, line: Option<u32>, width: u16) -> usize {
    match (side, line) {
        (Some(side), Some(i)) => wrap::line_height(&side.lines[i as usize], width),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::model::Row;
    use LineKind::{Changed as C, Removed as R, Unchanged as U};

    fn side(n: usize) -> Option<SideText> {
        text(&vec![U; n])
    }

    fn text(kinds: &[LineKind]) -> Option<SideText> {
        let n = kinds.len();
        Some(SideText {
            lines: vec![String::new(); n],
            kinds: kinds.to_vec(),
            changes: vec![Vec::new(); n],
            highlights: vec![Vec::new(); n],
        })
    }

    fn row(lhs: Option<u32>, rhs: Option<u32>) -> Row {
        Row { lhs, rhs }
    }

    #[test]
    fn j_skips_padding_on_active_side() {
        let diff = FileDiff {
            lhs: side(3),
            rhs: side(2),
            rows: vec![
                row(Some(0), Some(0)),
                row(Some(1), None),
                row(Some(2), None),
                row(None, Some(1)),
            ],
            hunks: Vec::new(),
        };
        let mut view = FileView::new(diff);
        assert_eq!((view.side, view.row), (Side::New, 0));
        view.motion(Motion::Down, None, 10);
        assert_eq!(view.row, 3);
        view.switch(Side::Old);
        assert_eq!(view.row, 2);
        view.motion(Motion::Up, Some(2), 10);
        assert_eq!(view.row, 0);
    }

    #[test]
    fn hunks_switch_to_the_changed_side_and_back_home() {
        let diff = FileDiff {
            lhs: text(&[U, C, U, R, U, C]),
            rhs: text(&[U, C, U, U, C]),
            rows: vec![
                row(Some(0), Some(0)),
                row(Some(1), Some(1)),
                row(Some(2), Some(2)),
                row(Some(3), None),
                row(Some(4), Some(3)),
                row(Some(5), Some(4)),
            ],
            hunks: vec![1..2, 3..4, 5..6],
        };
        let mut view = FileView::new(diff);
        assert_eq!((view.row, view.side), (1, Side::New));
        let mut path = Vec::new();
        for next in [true, true, false, false] {
            assert!(view.hunk(next, 1));
            path.push((view.row, view.side));
        }
        assert_eq!(
            path,
            [
                (3, Side::Old),
                (5, Side::New),
                (3, Side::Old),
                (1, Side::New)
            ]
        );
        assert!(!view.hunk(false, 1));
    }
}
