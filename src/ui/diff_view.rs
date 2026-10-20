//! Two-column diff rendering with gutters, tints and the row cursor.

use std::ops::Range;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::comments::model::Side;
use crate::diff::model::{FileDiff, LineKind, SideText};
use crate::highlight::Theme;
use crate::highlight::highlighter::StyledSpan;
use crate::vim::wrap;

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

/// Draws visible rows only, from the view's scroll row down to the bottom of `area`.
pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let Some(view) = app.view() else { return };
    let diff = &view.diff;
    let tops = view.tops();
    let cols = columns(diff, area.width);
    let xs = [area.x, area.x + area.width / 2];
    let buf = frame.buffer_mut();
    let mut y = area.y;
    for r in view.scroll..diff.rows.len() {
        if y >= area.bottom() {
            break;
        }
        let height = (tops[r + 1] - tops[r]).min(usize::from(area.bottom() - y)) as u16;
        for (i, side) in [Side::Old, Side::New].into_iter().enumerate() {
            let rect = Rect::new(xs[i], y, cols[i].gutter + cols[i].text, height);
            let text = match side {
                Side::Old => diff.lhs.as_ref(),
                Side::New => diff.rhs.as_ref(),
            };
            let cell = Cell {
                theme: &app.theme,
                wrap: app.config.ui.wrap,
                side,
                active: view.side == side,
                cursor: view.side == side && view.row == r,
            };
            if let Some((text, line)) = text.zip(view.line(r, side)) {
                cell.draw(buf, rect, cols[i], text, line as usize);
            }
        }
        y += height;
    }
}

struct Cell<'a> {
    theme: &'a Theme,
    wrap: bool,
    side: Side,
    active: bool,
    cursor: bool,
}

impl Cell<'_> {
    /// Line `i` of one side: gutter on the first screen line, wrapped segments below it.
    fn draw(&self, buf: &mut Buffer, rect: Rect, col: Column, text: &SideText, i: usize) {
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
            Span::raw("  "),
        ]);
        buf.set_line(rect.x, rect.y, &gutter, col.gutter);

        let line = &text.lines[i];
        let segments = if self.wrap {
            wrap::wrap_line(line, col.text)
        } else {
            std::iter::once(0..line.len()).collect()
        };
        let base = Style::new().fg(theme.foreground).bg(bg);
        for (k, seg) in segments
            .into_iter()
            .take(usize::from(rect.height))
            .enumerate()
        {
            let spans = spans(
                line,
                seg,
                &text.highlights[i],
                &text.changes[i],
                base,
                token_tint,
            );
            buf.set_line(text_rect.x, rect.y + k as u16, &Line::from(spans), col.text);
        }
    }
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
