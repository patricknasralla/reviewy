//! Top-level drawing.

pub mod diff_view;
pub mod editor;
pub mod explorer;
pub mod modals;
pub mod status;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;

use crate::app::App;
use crate::app::state::Mode;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    frame.render_widget(
        Block::new().style(
            Style::new()
                .bg(app.theme.background)
                .fg(app.theme.foreground),
        ),
        area,
    );
    let view = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };
    let status = Rect {
        y: view.bottom(),
        height: area.height - view.height,
        ..area
    };
    diff_view::draw(frame, view, app);
    if app.mode == Mode::Explorer {
        explorer::draw(frame, view, app);
    }
    for modal in &app.modals {
        modals::draw(frame, view, &app.theme, modal);
    }
    status::draw(frame, status, app);
}

/// `width` × `height` rect centered in `area`, clamped to it.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let (width, height) = (width.min(area.width), height.min(area.height));
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crossterm::event::{KeyCode, KeyEvent};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;

    use super::*;
    use crate::app::state::{Modal, PendingOpen};
    use crate::config;
    use crate::diff::align;
    use crate::git::{ChangedFile, FileContent, FileInfo, FileStatus};
    use crate::highlight::Theme;

    const OLD: &str = "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n";
    const NEW: &str = "fn main() {\n    let x = 2;\n    let long = \"this line is long enough to wrap in a column\";\n    println!(\"{x}\");\n}\n";
    const MODIFIED: &str = r#"{"status":"changed",
        "aligned_lines":[[0,0],[1,1],[null,2],[2,3],[3,4],[4,5]],
        "chunks":[[
            {"lhs":{"line_number":1,"changes":[{"start":12,"end":13}]},
             "rhs":{"line_number":1,"changes":[{"start":12,"end":13}]}},
            {"rhs":{"line_number":2,"changes":[{"start":4,"end":7}]}}
        ]]}"#;

    fn file(path: &str, status: FileStatus, binary: bool) -> ChangedFile {
        ChangedFile {
            path: path.into(),
            status,
            binary,
        }
    }

    fn app(files: Vec<ChangedFile>, wrap: bool) -> App {
        let mut config = config::from_sources(&[]).unwrap();
        config.ui.wrap = wrap;
        let theme = Theme::bundled_default(config.ui.tint);
        let mut app = App::new(config, PathBuf::new(), "0".repeat(40), files, theme, None);
        app.resize(80, 24);
        app
    }

    fn open(app: &mut App, json: &str, old: Option<&str>, new: Option<&str>) {
        let content = FileContent {
            old: old.map(Into::into),
            new: new.map(Into::into),
        };
        let diff = align::build(&serde_json::from_str(json).unwrap(), &content, 4);
        app.insert_view(0, diff);
    }

    fn render(app: &App) -> TestBackend {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal.backend().clone()
    }

    #[test]
    fn explorer() {
        let app = app(
            vec![
                file("src/main.rs", FileStatus::Modified, false),
                file("notes.txt", FileStatus::Untracked, false),
                file(
                    "src/new.rs",
                    FileStatus::Renamed {
                        from: "src/old.rs".into(),
                    },
                    false,
                ),
                file("logo.png", FileStatus::Modified, true),
            ],
            true,
        );
        insta::assert_snapshot!(render(&app));
    }

    #[test]
    fn modified_wrap_on() {
        let mut app = app(vec![file("src/main.rs", FileStatus::Modified, false)], true);
        open(&mut app, MODIFIED, Some(OLD), Some(NEW));
        insta::assert_snapshot!(render(&app));
    }

    #[test]
    fn modified_wrap_off() {
        let mut app = app(
            vec![file("src/main.rs", FileStatus::Modified, false)],
            false,
        );
        open(&mut app, MODIFIED, Some(OLD), Some(NEW));
        insta::assert_snapshot!(render(&app));
    }

    #[test]
    fn added_file() {
        let mut app = app(
            vec![file("src/main.rs", FileStatus::Untracked, false)],
            true,
        );
        open(&mut app, r#"{"status":"created"}"#, None, Some(NEW));
        insta::assert_snapshot!(render(&app));
    }

    #[test]
    fn charwise_selection_over_two_lines() {
        let mut app = app(vec![file("src/main.rs", FileStatus::Modified, false)], true);
        open(&mut app, MODIFIED, Some(OLD), Some(NEW));
        // Opens on `let` of new line 2; select to the end of `let` on line 3.
        for c in ['v', 'j', 'e'] {
            app.handle_key(KeyEvent::from(KeyCode::Char(c)));
        }
        let backend = render(&app);
        // Right text column starts at 40 + gutter 5; new lines 2 and 3 are on screen lines 1, 2.
        let buf = backend.buffer();
        assert_eq!(buf[(49, 1)].bg, app.theme.selection);
        assert_eq!(buf[(45, 2)].bg, app.theme.selection);
        assert_eq!(buf[(51, 2)].bg, app.theme.selection);
        assert_ne!(buf[(52, 2)].bg, app.theme.selection);
        assert!(buf[(51, 2)].modifier.contains(Modifier::REVERSED));
        insta::assert_snapshot!(backend);
    }

    #[test]
    fn large_file_modal() {
        let mut app = app(vec![file("big.rs", FileStatus::Modified, false)], true);
        app.modals.push(Modal::LargeFile {
            pending: PendingOpen {
                index: 0,
                content: FileContent {
                    old: None,
                    new: None,
                },
            },
            info: FileInfo {
                lines: 6000,
                bytes: 2_000_000,
                longest_line: 1200,
            },
        });
        insta::assert_snapshot!(render(&app));
    }
}
