//! Comment editor box: frame, wrapped text and cursor.

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Clear, Padding};

use super::centered;
use crate::editor::{Editor, Mode};
use crate::highlight::Theme;

/// The editor box: centered, 70% × 50% of `area`.
pub fn rect(area: Rect) -> Rect {
    centered(area, area.width * 7 / 10, area.height / 2)
}

/// Text area inside the box.
pub fn inner(area: Rect) -> Rect {
    block().inner(rect(area))
}

fn block() -> Block<'static> {
    Block::bordered().padding(Padding::horizontal(1))
}

/// `focused`: the editor is the top modal; `cursor_on`: the block cursor's blink phase.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    editor: &Editor,
    theme: &Theme,
    focused: bool,
    cursor_on: bool,
) {
    let rect = rect(area);
    let block = block()
        .title(format!(" {} ", editor.title))
        .style(Style::new().bg(theme.background).fg(theme.foreground));
    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);
    let buf = frame.buffer_mut();
    let selected = Style::new().bg(theme.selection);
    for (y, (i, seg)) in (inner.y..inner.bottom()).zip(editor.rows()) {
        let line = editor.line(i);
        buf.set_stringn(
            inner.x,
            y,
            &line[seg.clone()],
            usize::from(inner.width),
            Style::new(),
        );
        let sel = editor.selection_cols(i);
        let cells = sel.and_then(|sel| super::selection_cells(line, seg, sel, inner.width));
        if let Some(cells) = cells {
            let rect = Rect::new(inner.x + cells.start, y, cells.len() as u16, 1);
            buf.set_style(rect, selected);
        }
    }
    let Some((row, x)) = editor.cursor_cell().filter(|_| focused) else {
        return;
    };
    let pos = Position::new(inner.x + x, inner.y + row);
    // A box too small for any text (tiny terminal) has no cell for the cursor.
    if !inner.contains(pos) {
        return;
    }
    match editor.mode() {
        Mode::Insert => frame.set_cursor_position(pos),
        _ if cursor_on => super::cursor_block(frame.buffer_mut(), pos, theme),
        _ => {}
    }
}
