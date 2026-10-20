//! Status line: mode, file, position, pending keys, message.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::app::state::Mode;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let mode = match app.mode {
        Mode::Explorer => "EXPLORER",
        Mode::Row => "ROW",
    };
    let mut spans = vec![Span::styled(
        format!(" {mode} "),
        Style::new()
            .fg(theme.background)
            .bg(theme.foreground)
            .add_modifier(Modifier::BOLD),
    )];
    if let (Some(file), Some(view)) = (app.current_file(), app.view()) {
        let pos = view
            .cursor_line()
            .map_or("0:0".into(), |l| format!("{}:1", l + 1));
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
