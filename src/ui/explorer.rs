//! Explorer overlay: changed files in git status order.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear};

use super::centered;
use crate::app::App;
use crate::git::{ChangedFile, FileStatus};
use crate::highlight::Theme;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let rows = u16::try_from(app.files.len()).unwrap_or(u16::MAX);
    let rect = centered(
        area,
        area.width * 3 / 5,
        rows.saturating_add(2).min(area.height * 4 / 5),
    );
    let block = Block::bordered()
        .title(" Changed files ")
        .style(Style::new().bg(theme.background).fg(theme.foreground));
    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);

    let buf = frame.buffer_mut();
    let visible = usize::from(inner.height);
    let offset = (app.explorer + 1).saturating_sub(visible);
    let files = app.files.iter().enumerate().skip(offset).take(visible);
    for (y, (i, file)) in (inner.y..).zip(files) {
        if i == app.explorer {
            let row = Rect::new(inner.x, y, inner.width, 1);
            buf.set_style(row, Style::new().bg(theme.active_line_background));
        }
        let line = entry(file, app.comments.count_for(&file.path), theme);
        buf.set_line(inner.x, y, &line, inner.width);
    }
}

/// ` M src/foo.rs B [2]`: status letter and path in the status color, marker, comment count.
fn entry(file: &ChangedFile, comments: usize, theme: &Theme) -> Line<'static> {
    let (letter, color) = match &file.status {
        FileStatus::Added => ('A', theme.added),
        FileStatus::Untracked => ('?', theme.added),
        FileStatus::Deleted => ('D', theme.deleted),
        FileStatus::Renamed { .. } => ('R', theme.modified),
        FileStatus::Conflicted => ('U', theme.conflict),
        FileStatus::Modified | FileStatus::Submodule | FileStatus::ModeOnly => {
            ('M', theme.modified)
        }
    };
    let path = match &file.status {
        FileStatus::Renamed { from } => format!("{} → {}", from.display(), file.path.display()),
        _ => file.path.display().to_string(),
    };
    let marker = match file.status {
        _ if file.binary => " B",
        FileStatus::Conflicted => " C",
        FileStatus::Submodule => " S",
        FileStatus::ModeOnly => " P",
        _ => "",
    };
    let count = if comments > 0 {
        format!(" [{comments}]")
    } else {
        String::new()
    };
    Line::from(vec![
        Span::styled(format!(" {letter} {path}"), Style::new().fg(color)),
        Span::styled(marker, Style::new().fg(theme.line_number)),
        Span::raw(count),
    ])
}
