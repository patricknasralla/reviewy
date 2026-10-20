//! Status line: mode, file, position, pending keys, message.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::app::state::Mode;
use crate::editor;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let mode = match app.mode {
        _ if let Some(e) = app.editor() => match e.mode() {
            editor::Mode::Normal => "NORMAL",
            editor::Mode::Insert => "INSERT",
            editor::Mode::Visual { .. } => "VISUAL",
            editor::Mode::VisualLine { .. } => "V-LINE",
        },
        Mode::Explorer => "EXPLORER",
        Mode::Normal => "NORMAL",
        Mode::Visual { .. } => "VISUAL",
        Mode::VisualLine { .. } => "V-LINE",
    };
    let mut spans = vec![Span::styled(
        format!(" {mode} "),
        Style::new()
            .fg(theme.background)
            .bg(theme.foreground)
            .add_modifier(Modifier::BOLD),
    )];
    if let (Some(file), Some(view)) = (app.current_file(), app.view()) {
        let p = view.cursor.pos;
        let pos = view
            .cursor_line()
            .map_or("0:0".into(), |_| format!("{}:{}", p.line + 1, p.col + 1));
        spans.push(Span::raw(format!(" {}  {pos}", file.path.display())));
    }
    let pending = app.keys.pending();
    if !pending.is_empty() {
        spans.push(Span::styled(
            format!("  {pending}"),
            Style::new().fg(theme.modified),
        ));
    }
    if let Some(status) = &app.status {
        spans.push(Span::styled(
            format!("  {status}"),
            Style::new().fg(theme.line_number),
        ));
    }
    frame.render_widget(
        Line::from(spans).style(Style::new().bg(theme.background).fg(theme.foreground)),
        area,
    );
}
