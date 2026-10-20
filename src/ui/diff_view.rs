//! Two-column diff rendering with gutters, tints and the row cursor.

use std::ops::Range;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::app::comment::side_path;
use crate::app::state::Mode;
use crate::app::view::FileView;
use crate::comments::model::Side;
use crate::diff::model::{FileDiff, LineKind, SideText};
use crate::highlight::Theme;
use crate::highlight::highlighter::StyledSpan;
use crate::vim::{selection, wrap};

/// Gutter cells besides the line number: leading space, change marker, comment slot, space.
const GUTTER_EXTRA: u16 = 4;

#[derive(Debug, Clone, Copy)]
pub struct Column {
    pub gutter: u16,
    pub text: u16,
}

/// Left and right column widths for a total `width`; the right column gets the odd cell.
pub fn columns(diff: &FileDiff, width: u16) -> [Column; 2] {
    let column = |side: &Option<SideText>, width: u16| {
        let digits = side.as_ref().map_or(1, |s| s.lines.len().to_string().len()) as u16;
        let gutter = (digits + GUTTER_EXTRA).min(width);
        Column {
            gutter,
            text: width - gutter,
        }
    };
    let left = width / 2;
    [column(&diff.lhs, left), column(&diff.rhs, width - left)]
}

/// Draws visible rows only, from the view's scroll line down to the bottom of `area`.
pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let Some((file, view)) = app.current_file().zip(app.view()) else {
        return;
    };
    let diff = &view.diff;
    let tops = view.tops();
    let cols = columns(diff, area.width);
    let show_cursor = app.diff_focused() && app.cursor_on;
    let xs = [area.x, area.x + area.width / 2];
    let buf = frame.buffer_mut();
    let first = tops.partition_point(|&t| t <= view.scroll) - 1;
    let mut skip = view.scroll - tops[first];
    let mut y = area.y;
    for r in first..diff.rows.len() {
        if y >= area.bottom() {
            break;
        }
        let height = (tops[r + 1] - tops[r] - skip).min(usize::from(area.bottom() - y)) as u16;
        for (i, side) in [Side::Old, Side::New].into_iter().enumerate() {
            let rect = Rect::new(xs[i], y, cols[i].gutter + cols[i].text, height);
            let active = view.side == side;
            let cell = Cell {
                theme: &app.theme,
                wrap: app.config.ui.wrap,
                hscroll: view.hscroll,
                skip,
                side,
                active,
                cursor: active && view.row == r,
            };
            if let Some((text, line)) = view.text(side).zip(view.line(r, side)) {
                let comment = (app.comments)
                    .at(side_path(file, side), side, line + 1)
                    .is_some();
                let line = line as usize;
                let marks = Marks {
                    comment,
                    ..if active {
                        marks(app.mode, view, line, show_cursor)
                    } else {
                        Marks::default()
                    }
                };
                cell.draw(buf, rect, cols[i], text, line, marks);
            }
        }
        y += height;
        skip = 0;
    }
}

/// Comment marker, text cursor and selection on one line, in chars.
#[derive(Default)]
struct Marks {
    comment: bool,
    /// Half-open; may extend past the line end.
    selection: Option<Range<usize>>,
    cursor: Option<usize>,
}

fn marks(mode: Mode, view: &FileView, line: usize, show_cursor: bool) -> Marks {
    let cur = view.cursor.pos;
    let selection = match mode {
        Mode::Visual { anchor } => selection::line_cols(anchor, cur, false, line),
        Mode::VisualLine { anchor } => selection::line_cols(anchor, cur, true, line),
        _ => None,
    };
    Marks {
        comment: false,
        selection,
        cursor: (show_cursor && cur.line == line).then_some(cur.col),
    }
}

struct Cell<'a> {
    theme: &'a Theme,
    wrap: bool,
    hscroll: usize,
    /// Leading screen lines of the row scrolled out of view.
    skip: usize,
    side: Side,
    active: bool,
    /// Cursor row on the active side.
    cursor: bool,
}

impl Cell<'_> {
    /// Line `i` of one side: gutter on the first screen line, wrapped segments below it.
    fn draw(
        &self,
        buf: &mut Buffer,
        rect: Rect,
        col: Column,
        text: &SideText,
        i: usize,
        marks: Marks,
    ) {
        let theme = self.theme;
        let kind = text.kinds[i];
        let (line_tint, token_tint) = match self.side {
            Side::Old => (theme.line_deleted_bg, theme.token_deleted_bg),
            Side::New => (theme.line_added_bg, theme.token_added_bg),
        };
        let bg = if self.cursor {
            theme.active_line_background
        } else if kind != LineKind::Unchanged {
            line_tint
        } else {
            theme.background
        };
        let text_rect = Rect {
            x: rect.x + col.gutter,
            width: col.text,
            ..rect
        };
        buf.set_style(text_rect, Style::new().bg(bg));

        if self.skip == 0 {
            let (marker, marker_fg) = match kind {
                LineKind::Unchanged => (' ', theme.foreground),
                LineKind::Changed => ('~', theme.modified),
                LineKind::Added => ('+', theme.added),
                LineKind::Removed => ('-', theme.deleted),
            };
            let number_fg = if self.active {
                theme.foreground
            } else {
                theme.line_number
            };
            let digits = usize::from(col.gutter.saturating_sub(GUTTER_EXTRA));
            let gutter = Line::from(vec![
                Span::styled(format!(" {:>digits$}", i + 1), Style::new().fg(number_fg)),
                Span::styled(marker.to_string(), Style::new().fg(marker_fg)),
                Span::styled(
                    if marks.comment { "● " } else { "  " },
                    Style::new().fg(theme.modified),
                ),
            ]);
            buf.set_line(rect.x, rect.y, &gutter, col.gutter);
        }

        let line = &text.lines[i];
        let segments = if self.wrap {
            wrap::wrap_line(line, col.text)
        } else {
            std::iter::once(clip_start(line, self.hscroll)..line.len()).collect()
        };
        let base = Style::new().fg(theme.foreground).bg(bg);
        let selected = Style::new().bg(theme.selection);
        let visible = self.skip..self.skip + usize::from(rect.height);
        for (k, seg) in segments.iter().enumerate() {
            if !visible.contains(&k) {
                continue;
            }
            let y = rect.y + (k - self.skip) as u16;
            let spans = spans(
                line,
                seg.clone(),
                &text.highlights[i],
                &text.changes[i],
                base,
                token_tint,
            );
            buf.set_line(text_rect.x, y, &Line::from(spans), col.text);
            let sel = marks.selection.clone();
            let cells =
                sel.and_then(|sel| super::selection_cells(line, seg.clone(), sel, col.text));
            if let Some(cells) = cells {
                let rect = Rect::new(text_rect.x + cells.start, y, cells.len() as u16, 1);
                buf.set_style(rect, selected);
            }
        }

        if let Some(c) = marks.cursor {
            let (seg, x) = if self.wrap {
                wrap::to_screen(line, col.text, c)
            } else {
                let start = clip_start(line, self.hscroll);
                (
                    0,
                    wrap::display_col(line, c) - width(&line[..start]) as usize,
                )
            };
            if visible.contains(&seg) && x < usize::from(col.text) {
                let pos = Position::new(text_rect.x + x as u16, rect.y + (seg - self.skip) as u16);
                super::cursor_block(buf, pos, theme);
            }
        }
    }
}

/// Byte where the first char at display column `hscroll` or later starts.
fn clip_start(line: &str, hscroll: usize) -> usize {
    let mut x = 0;
    for (i, c) in line.char_indices() {
        if x >= hscroll {
            return i;
        }
        x += wrap::char_width(c);
    }
    line.len()
}

fn width(s: &str) -> u16 {
    s.chars().map(wrap::char_width).sum::<usize>() as u16
}

/// Splits `line[seg]` at highlight and change boundaries; changes override the background.
fn spans<'a>(
    line: &'a str,
    seg: Range<usize>,
    highlights: &[StyledSpan],
    changes: &[Range<usize>],
    base: Style,
    token_bg: Color,
) -> Vec<Span<'a>> {
    let mut cuts = vec![seg.start, seg.end];
    for r in highlights.iter().map(|h| &h.range).chain(changes) {
        cuts.extend([r.start, r.end].into_iter().filter(|p| seg.contains(p)));
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .map(|w| {
            let mut style = base;
            if let Some(h) = highlights.iter().find(|h| h.range.contains(&w[0])) {
                style = style.patch(h.style);
            }
            if changes.iter().any(|c| c.contains(&w[0])) {
                style = style.bg(token_bg);
            }
            Span::styled(&line[w[0]..w[1]], style)
        })
        .collect()
}
