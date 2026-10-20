//! Confirmation modals.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Padding, Paragraph, Wrap};

use super::centered;
use crate::app::state::Modal;
use crate::highlight::Theme;

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, modal: &Modal) {
    match modal {
        Modal::LargeFile { info, .. } => confirm(
            frame,
            area,
            theme,
            "Large file",
            &format!(
                "Large file ({} lines, {} KB, longest line {}). View anyway?",
                info.lines,
                info.bytes.div_ceil(1024),
                info.longest_line
            ),
            "Enter: view / Esc: cancel",
        ),
        Modal::Quit => confirm(
            frame,
            area,
            theme,
            "Quit",
            "Save and quit?",
            "Enter: quit / Esc: cancel",
        ),
    }
}

/// Centered box: wrapped message, blank line, key hint.
fn confirm(frame: &mut Frame, area: Rect, theme: &Theme, title: &str, message: &str, hint: &str) {
    let width = (message.chars().count().max(hint.chars().count()) as u16 + 4).min(60);
    let lines = message
        .chars()
        .count()
        .div_ceil(usize::from(width.saturating_sub(4).max(1)));
    let rect = centered(area, width, lines as u16 + 4);
    let block = Block::bordered()
        .title(format!(" {title} "))
        .style(Style::new().bg(theme.background).fg(theme.foreground));
    let text = vec![
        Line::raw(message),
        Line::raw(""),
        Line::styled(hint, Style::new().fg(theme.line_number)),
    ];
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(text)
            .block(block.padding(Padding::horizontal(1)))
            .wrap(Wrap { trim: true }),
        rect,
    );
}
