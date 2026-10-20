//! Per-file view: diff plus row cursor, active side, scroll and cached row layout.

use std::ops::Range;

use crate::comments::model::Side;
use crate::diff::model::{FileDiff, LineKind, SideText};
use crate::ui::diff_view::columns;
use crate::vim::motion::{self, Cursor, Motion, MotionCtx, Pos};
use crate::vim::wrap;

const SCROLL_MARGIN: usize = 3;

pub struct FileView {
    pub diff: FileDiff,
    pub side: Side,
    /// Side last chosen with `h`/`l`; hunk jumps prefer it.
    home: Side,
    /// Cursor row index into `diff.rows`.
    pub row: usize,
    /// Text cursor on the active side.
    pub cursor: Cursor,
    /// First visible screen line.
    pub scroll: usize,
    /// First visible display column when not wrapping; shared by both columns.
    pub hscroll: usize,
    /// Row of each line, per side (old, new).
    row_of: [Vec<usize>; 2],
    layout: Layout,
}

#[derive(Default)]
struct Layout {
    /// (total width, wrap) the heights were computed for.
    key: Option<(u16, bool)>,
    /// Screen line where each row starts; one extra entry for the end.
    tops: Vec<usize>,
    /// Text widths of the (old, new) columns.
    text: [u16; 2],
}

impl FileView {
    /// Home side is new unless the file is deleted; the cursor starts on the first hunk.
    pub fn new(diff: FileDiff) -> Self {
        let side = if diff.rhs.is_some() {
            Side::New
        } else {
            Side::Old
        };
        let row_of = [Side::Old, Side::New].map(|side| rows_of(&diff, side));
        let mut view = Self {
            diff,
            side,
            home: side,
            row: 0,
            cursor: Cursor::default(),
            scroll: 0,
            hscroll: 0,
            row_of,
            layout: Layout::default(),
        };
        match view.targets().first() {
            Some(&(row, side)) => (view.row, view.side) = (row, side),
            None => view.row = view.nearest(0).unwrap_or(0),
        }
        view.land();
        view
    }

    pub fn text(&self, side: Side) -> Option<&SideText> {
        side_text(&self.diff, side)
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

    /// Text width of the active column; valid after `refresh_layout`.
    pub fn text_width(&self) -> u16 {
        self.layout.text[index(self.side)]
    }

    /// Cursor motion over the active side; the cursor row follows the line.
    pub fn cursor_motion(&mut self, motion: Motion, count: Option<usize>, ctx: &MotionCtx) {
        let Some(text) = self.text(self.side) else {
            return;
        };
        let cursor = motion::apply(motion, count, self.cursor, text, ctx);
        if let Some(&row) = self.row_of[index(self.side)].get(cursor.pos.line) {
            (self.cursor, self.row) = (cursor, row);
        }
    }

    /// Makes `side` active and moves to the nearest row with a line there; no-op if the side is absent.
    pub fn switch(&mut self, side: Side) {
        if self.text(side).is_none() {
            return;
        }
        self.side = side;
        self.home = side;
        if let Some(row) = self.nearest(self.row) {
            self.row = row;
        }
        self.land();
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
        self.land();
        true
    }

    /// Recomputes row heights if the width or wrap changed, then scrolls the text cursor into
    /// view (its screen line, and its column when not wrapping).
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
                text: [l.text, r.text],
            };
        }
        if self.diff.rows.is_empty() {
            self.scroll = 0;
            return;
        }
        let span = self.focus(wrap);
        self.keep_visible(height, span);
        if !wrap {
            self.keep_column();
        }
    }

    fn has_line(&self, row: usize) -> bool {
        self.line(row, self.side).is_some()
    }

    /// The text cursor's line on the active side.
    fn cursor_text(&self) -> Option<&str> {
        let text = self.text(self.side)?;
        text.lines.get(self.cursor.pos.line).map(String::as_str)
    }

    /// Puts the text cursor on the first non-blank char of the cursor row's line.
    fn land(&mut self) {
        let Some(line) = self.cursor_line() else {
            return;
        };
        let text = &self.text(self.side).unwrap().lines[line as usize];
        let col = motion::first_non_blank(text);
        self.cursor = Cursor {
            pos: Pos {
                line: line as usize,
                col,
            },
            want: wrap::display_col(text, col),
        };
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
        let sides = [self.home, other(self.home)];
        (self.diff.hunks.iter())
            .filter_map(|h| {
                sides.into_iter().find_map(|side| {
                    let row = h.clone().find(|&r| self.changed(r, side))?;
                    Some((row, side))
                })
            })
            .collect()
    }

    fn changed(&self, row: usize, side: Side) -> bool {
        match (self.text(side), self.line(row, side)) {
            (Some(text), Some(i)) => text.kinds[i as usize] != LineKind::Unchanged,
            _ => false,
        }
    }

    /// The text cursor's screen line.
    fn focus(&self, wrap: bool) -> Range<usize> {
        let tops = &self.layout.tops;
        let seg = match self.cursor_text() {
            Some(line) if wrap => wrap::to_screen(line, self.text_width(), self.cursor.pos.col).0,
            _ => 0,
        };
        let top = tops[self.row] + seg;
        top..top + 1
    }

    /// Keeps `span` `SCROLL_MARGIN` screen lines from the edges, where lines exist there.
    fn keep_visible(&mut self, height: usize, span: Range<usize>) {
        let end = self.layout.tops[self.layout.tops.len() - 1];
        let margin = SCROLL_MARGIN.min(height / 2);
        let up = span.start.saturating_sub(margin);
        if self.scroll > up {
            self.scroll = up;
        }
        let below = margin.min(end - span.end);
        let need = (span.end + below).saturating_sub(height);
        if self.scroll < need {
            // A span taller than the viewport still starts on screen.
            self.scroll = need.min(span.start);
        }
    }

    /// Scrolls horizontally so the text cursor's cell is inside the active column.
    fn keep_column(&mut self) {
        let Some(line) = self.cursor_text() else {
            return;
        };
        let col = self.cursor.pos.col;
        let start = wrap::display_col(line, col);
        let end = wrap::display_col(line, col + 1).max(start + 1);
        let width = usize::from(self.text_width());
        if start < self.hscroll {
            self.hscroll = start;
        } else if end > self.hscroll + width {
            self.hscroll = end.saturating_sub(width);
        }
    }
}

fn side_text(diff: &FileDiff, side: Side) -> Option<&SideText> {
    match side {
        Side::Old => diff.lhs.as_ref(),
        Side::New => diff.rhs.as_ref(),
    }
}

fn index(side: Side) -> usize {
    match side {
        Side::Old => 0,
        Side::New => 1,
    }
}

fn other(side: Side) -> Side {
    match side {
        Side::Old => Side::New,
        Side::New => Side::Old,
    }
}

/// Row of each of `side`'s lines; a line missing from the rows gets the previous line's row.
fn rows_of(diff: &FileDiff, side: Side) -> Vec<usize> {
    let n = side_text(diff, side).map_or(0, |t| t.lines.len());
    let mut rows = vec![None; n];
    for (r, row) in diff.rows.iter().enumerate() {
        let line = match side {
            Side::Old => row.lhs,
            Side::New => row.rhs,
        };
        if let Some(slot) = line.and_then(|l| rows.get_mut(l as usize)) {
            *slot = Some(r);
        }
    }
    let mut prev = 0;
    rows.into_iter()
        .map(|r| {
            prev = r.unwrap_or(prev);
            prev
        })
        .collect()
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

    fn lines(lines: &[&str]) -> Option<SideText> {
        let mut side = side(lines.len())?;
        side.lines = lines.iter().map(|l| l.to_string()).collect();
        Some(side)
    }

    fn row(lhs: Option<u32>, rhs: Option<u32>) -> Row {
        Row { lhs, rhs }
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

    #[test]
    fn screen_down_skips_row_padding_and_padding_rows() {
        // Width 30: both text columns are 10 wide. Row 0 is 3 lines tall (old), new's line 0
        // only 2; row 1 is padding on new.
        let diff = FileDiff {
            lhs: lines(&[&"a".repeat(30), "x", "y"]),
            rhs: lines(&[&"b".repeat(20), "z"]),
            rows: vec![
                row(Some(0), Some(0)),
                row(Some(1), None),
                row(Some(2), Some(1)),
            ],
            hunks: Vec::new(),
        };
        let mut view = FileView::new(diff);
        view.refresh_layout(30, 20, true);
        let ctx = MotionCtx {
            height: 20,
            wrap: Some(view.text_width()),
        };
        view.cursor_motion(Motion::ScreenDown, None, &ctx);
        assert_eq!((view.row, view.cursor.pos), (0, Pos { line: 0, col: 10 }));
        view.cursor_motion(Motion::ScreenDown, None, &ctx);
        assert_eq!((view.row, view.cursor.pos), (2, Pos { line: 1, col: 0 }));
    }

    #[test]
    fn switch_lands_on_first_non_blank() {
        let diff = FileDiff {
            lhs: lines(&["a", "    b"]),
            rhs: lines(&["a"]),
            rows: vec![row(Some(0), Some(0)), row(Some(1), None)],
            hunks: Vec::new(),
        };
        let mut view = FileView::new(diff);
        view.row = 1;
        view.switch(Side::Old);
        assert_eq!(view.cursor.pos, Pos { line: 1, col: 4 });
    }
}
