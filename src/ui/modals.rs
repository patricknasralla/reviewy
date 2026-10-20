//! Confirmation modals.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Padding, Paragraph, Wrap};

use super::{centered, editor};
use crate::app::state::Modal;
use crate::highlight::Theme;

/// `top`: the modal receives input (the editor then shows its cursor); `cursor_on`: the
/// block cursor's blink phase.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    modal: &Modal,
    top: bool,
    cursor_on: bool,
) {
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
        Modal::LoadExisting(Ok(set)) => {
            let n = set.comments.len();
            let (noun, them) = if n == 1 {
                ("comment", "it")
            } else {
                ("comments", "them")
            };
            let base = set.base.commit.get(..7).unwrap_or(&set.base.commit);
            confirm(
                frame,
                area,
                theme,
                "Previous comments",
                &format!("Found {n} {noun} from a previous session (base {base}). Discard {them}?"),
                "Enter: discard / Esc: keep",
            )
        }
        Modal::LoadExisting(Err(e)) => confirm(
            frame,
            area,
            theme,
            "Previous comments",
            e,
            "Enter/Esc: discard (file replaced on exit)",
        ),
        Modal::Editor(e) => editor::draw(frame, area, e, theme, top, cursor_on),
        Modal::DeleteComment(id) => confirm(
            frame,
            area,
            theme,
            "Delete comment",
            &format!("Delete comment #{id}?"),
            "Enter: delete / Esc: cancel",
        ),
        Modal::CloseComment => confirm(
            frame,
            area,
            theme,
            "Close comment",
            "Close comment?",
            "Enter: close / Esc: edit",
        ),
        Modal::SaveComment(id) => confirm(
            frame,
            area,
            theme,
            "Save comment",
            &format!("Save comment #{id}?"),
            "Enter: save / d: delete / Esc: edit",
        ),
    }
}

/// Centered box: wrapped message, blank line, key hint.
fn confirm(frame: &mut Frame, area: Rect, theme: &Theme, title: &str, message: &str, hint: &str) {
    let width = (message.chars().count().max(hint.chars().count()) as u16 + 4).min(60);
    let block = Block::bordered()
        .title(format!(" {title} "))
        .padding(Padding::horizontal(1))
        .style(Style::new().bg(theme.background).fg(theme.foreground));
    let text = vec![
        Line::raw(message),
        Line::raw(""),
        Line::styled(hint, Style::new().fg(theme.line_number)),
    ];
    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    // Wraps at the text width (inside borders and padding); the count includes the borders.
    let height = paragraph.line_count(width.saturating_sub(4));
    let rect = centered(area, width, height as u16);
    frame.render_widget(Clear, rect);
    frame.render_widget(paragraph, rect);
}
