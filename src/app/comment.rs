//! Comment flows: create, edit, delete, and the editor's input.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::App;
use super::state::{Modal, Mode};
use super::view::FileView;
use crate::comments::model::{self, Anchor, Selection, Side};
use crate::editor::{self, Editor};
use crate::git::ChangedFile;
use crate::highlight::languages::Language;
use crate::ui;
use crate::vim::keys::{Command, Context};
use crate::vim::motion::Pos;

impl App {
    /// Enter: edit the cursor line's comment, or create one on the cursor line or selection.
    pub(super) fn comment(&mut self) {
        if self.mode == Mode::Normal
            && let Some(id) = self.comment_at_cursor()
        {
            self.open_editor(id, editor::Mode::Normal);
            return;
        }
        let Some((file, view)) = self.current_file().zip(self.view()) else {
            return;
        };
        let Some(anchor) = anchor(self.mode, view, file) else {
            return;
        };
        if let Some(id) = self.comments.overlaps(&anchor) {
            self.status = Some(format!("Overlaps comment #{id}"));
            return;
        }
        let language = Language::detect(&anchor.file).map(Language::name);
        let lines = &view.text(anchor.side).unwrap().lines;
        let context = self.config.comments.context_lines;
        let draft = model::draft(anchor, language, lines, context, String::new());
        let id = self.comments.add(draft);
        if self.mode != Mode::Normal {
            let comment = self.comments.get_mut(id).unwrap();
            comment.body = model::prefill_body(&comment.selected_text(), language);
        }
        self.mode = Mode::Normal;
        self.open_editor(id, editor::Mode::Insert);
    }

    pub(super) fn delete_comment(&mut self) {
        match self.comment_at_cursor() {
            Some(id) => self.modals.push(Modal::DeleteComment(id)),
            None => self.status = Some("No comment on this line".into()),
        }
    }

    /// Keys while the editor is the top modal: raw in Insert mode, through the key parser in
    /// Normal mode. Esc in Normal mode with nothing pending asks to close the editor.
    pub(super) fn editor_key(&mut self, key: KeyEvent) {
        let Some(Modal::Editor(editor)) = self.modals.last_mut() else {
            return;
        };
        if key.kind == KeyEventKind::Release {
            return;
        }
        // Ctrl-q asks to save or delete; an empty comment is just closed (and so removed).
        if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let (id, empty) = (editor.id, editor.text().trim().is_empty());
            self.keys = Default::default();
            if empty {
                self.close_editor();
            } else {
                self.modals.push(Modal::SaveComment(id));
            }
            return;
        }
        let changed = match editor.mode() {
            editor::Mode::Insert => editor.insert_key(key),
            mode => {
                let normal = mode == editor::Mode::Normal;
                let ctx = if normal {
                    Context::EditorNormal
                } else {
                    Context::EditorVisual
                };
                let pending = !self.keys.pending().is_empty();
                match self.keys.handle(key, ctx) {
                    Some(input) if input.command == Command::Escape && normal && !pending => {
                        self.modals.push(Modal::CloseComment);
                        false
                    }
                    Some(input) => editor.input(input, &mut self.registers),
                    None => false,
                }
            }
        };
        if changed {
            self.sync_body();
        }
    }

    /// Bracketed paste; ignored unless the editor is the top modal.
    pub fn paste(&mut self, text: &str) {
        self.reset_blink();
        if let Some(Modal::Editor(editor)) = self.modals.last_mut() {
            editor.paste(text, self.config.ui.tab_width);
            self.sync_body();
            self.dirty = true;
        }
    }

    /// Pops the editor; a comment left empty is removed.
    pub(super) fn close_editor(&mut self) {
        if let Some(Modal::Editor(editor)) = self.modals.pop()
            && (self.comments.get(editor.id)).is_some_and(|c| c.body.trim().is_empty())
        {
            self.comments.remove(editor.id);
        }
    }

    /// Pops the editor and removes its comment.
    pub(super) fn delete_editor_comment(&mut self) {
        if let Some(Modal::Editor(editor)) = self.modals.pop() {
            self.comments.remove(editor.id);
        }
    }

    fn open_editor(&mut self, id: u32, mode: editor::Mode) {
        let comment = self.comments.get(id).unwrap();
        let anchor = &comment.anchor;
        let range = anchor.selection.line_range();
        let lines = if range.start() == range.end() {
            range.start().to_string()
        } else {
            format!("{}-{}", range.start(), range.end())
        };
        let side = match anchor.side {
            Side::Old => "old",
            Side::New => "new",
        };
        let title = format!("Comment - {}:{lines} ({side})", anchor.file.display());
        let levels = self.config.editor.undo_levels;
        let mut editor = Editor::new(id, title, &comment.body, mode, levels);
        let area = ui::editor::inner(self.view_area());
        editor.resize(area.width, area.height);
        self.modals.push(Modal::Editor(editor));
    }

    fn sync_body(&mut self) {
        if let Some(Modal::Editor(editor)) = self.modals.last()
            && let Some(comment) = self.comments.get_mut(editor.id)
        {
            comment.body = editor.text();
        }
    }

    /// Comment covering the cursor line on the active side.
    fn comment_at_cursor(&self) -> Option<u32> {
        let (file, view) = self.current_file().zip(self.view())?;
        let line = view.cursor_line()? + 1;
        let path = side_path(file, view.side);
        Some(self.comments.at(path, view.side, line)?.id)
    }
}

/// Repo-relative path of `file` on `side`: the old path of a rename on the old side.
pub fn side_path(file: &ChangedFile, side: Side) -> &Path {
    match side {
        Side::Old => file.old_path(),
        Side::New => &file.path,
    }
}

/// What Enter comments on; `None` in the explorer or when the side has no lines.
fn anchor(mode: Mode, view: &FileView, file: &ChangedFile) -> Option<Anchor> {
    view.cursor_line()?;
    let pos = |p: Pos| model::Pos {
        line: p.line as u32 + 1,
        col: p.col as u32 + 1,
    };
    let cursor = pos(view.cursor.pos);
    let selection = match mode {
        Mode::Normal => Selection::from_lines(cursor.line, cursor.line),
        Mode::Visual { anchor } => Selection::from_chars(pos(anchor), cursor),
        Mode::VisualLine { anchor } => Selection::from_lines(pos(anchor).line, cursor.line),
        Mode::Explorer => return None,
    };
    Some(Anchor {
        file: side_path(file, view.side).into(),
        side: view.side,
        selection,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::config;
    use crate::diff::model::{FileDiff, LineKind, Row, SideText};
    use crate::git::FileStatus;
    use crate::highlight::Theme;
    use crate::vim::motion::{Motion, MotionCtx};

    fn diff() -> FileDiff {
        let lines = ["let value = 1;", "call(value);"];
        let n = lines.len();
        FileDiff {
            lhs: None,
            rhs: Some(SideText {
                lines: lines.map(String::from).to_vec(),
                kinds: vec![LineKind::Added; n],
                changes: vec![Vec::new(); n],
                highlights: vec![Vec::new(); n],
            }),
            rows: (0..n as u32)
                .map(|l| Row {
                    lhs: None,
                    rhs: Some(l),
                })
                .collect(),
            hunks: std::iter::once(0..n).collect(),
        }
    }

    fn file() -> ChangedFile {
        ChangedFile {
            path: "a.rs".into(),
            status: FileStatus::Modified,
            binary: false,
        }
    }

    #[test]
    fn anchors_for_each_mode() {
        let file = file();
        let ctx = MotionCtx {
            height: 10,
            wrap: None,
        };
        let mut view = FileView::new(diff());
        let target = |mode, view: &FileView| anchor(mode, view, &file).unwrap().selection;
        let at = |line, col| model::Pos { line, col };

        assert_eq!(target(Mode::Normal, &view), Selection::Line { line: 1 });

        view.cursor_motion(Motion::WordForward, None, &ctx);
        let start = view.cursor.pos;
        view.cursor_motion(Motion::WordEnd, None, &ctx);
        assert_eq!(
            target(Mode::Visual { anchor: start }, &view),
            Selection::Chars {
                start: at(1, 5),
                end: at(1, 9)
            }
        );

        view.cursor_motion(Motion::Down, None, &ctx);
        assert_eq!(
            target(Mode::VisualLine { anchor: start }, &view),
            Selection::Lines { start: 1, end: 2 }
        );

        // Backwards: the anchor is after the cursor.
        let start = view.cursor.pos;
        view.cursor_motion(Motion::LineStart, None, &ctx);
        view.cursor_motion(Motion::WordBackward, None, &ctx);
        assert_eq!(
            target(Mode::Visual { anchor: start }, &view),
            Selection::Chars {
                start: at(1, 14),
                end: at(2, 9)
            }
        );
    }

    fn app() -> App {
        let config = config::from_sources(&[]).unwrap();
        let theme = Theme::bundled_default(config.ui.tint);
        let mut app = App::new(
            config,
            PathBuf::new(),
            "0".repeat(40),
            vec![file()],
            theme,
            None,
        );
        app.resize(80, 24);
        app.insert_view(0, diff());
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn create_from_visual_line_then_close_empty() {
        let mut app = app();
        for c in ['V', 'j'] {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);

        let comment = &app.comments.comments[0];
        assert_eq!(
            comment.anchor.selection,
            Selection::Lines { start: 1, end: 2 }
        );
        assert_eq!(comment.body, "```rust\nlet value = 1;\ncall(value);\n```\n");
        assert!(matches!(app.modals.last(), Some(Modal::Editor(_))));

        for _ in 0..comment.body.chars().count() {
            press(&mut app, KeyCode::Backspace);
        }
        assert_eq!(app.comments.comments[0].body, "");
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Enter);
        assert!(app.modals.is_empty());
        assert!(app.comments.comments.is_empty());
    }

    #[test]
    fn editor_modes_close_and_save() {
        let mut app = app();
        let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        let mode = |app: &App| app.editor().unwrap().mode();
        let top_is = |app: &App, f: fn(&Modal) -> bool| app.modals.last().is_some_and(f);

        // A new comment opens in Insert; Esc goes to Normal, then asks to close.
        press(&mut app, KeyCode::Enter);
        assert_eq!(mode(&app), editor::Mode::Insert);
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(mode(&app), editor::Mode::Normal);
        // Esc only clears a pending count.
        press(&mut app, KeyCode::Char('5'));
        press(&mut app, KeyCode::Esc);
        assert!(top_is(&app, |m| matches!(m, Modal::Editor(_))));
        press(&mut app, KeyCode::Esc);
        assert!(top_is(&app, |m| matches!(m, Modal::CloseComment)));
        press(&mut app, KeyCode::Enter);
        assert!(app.modals.is_empty());

        // An existing comment opens in Normal; Ctrl-q then Enter keeps it.
        press(&mut app, KeyCode::Enter);
        assert_eq!(mode(&app), editor::Mode::Normal);
        app.handle_key(ctrl_q);
        assert!(top_is(&app, |m| matches!(m, Modal::SaveComment(_))));
        // A second Ctrl-q can't quit past the open comment.
        app.handle_key(ctrl_q);
        assert!(top_is(&app, |m| matches!(m, Modal::SaveComment(_))));
        press(&mut app, KeyCode::Enter);
        assert!(app.modals.is_empty());
        assert_eq!(app.comments.comments.len(), 1);

        // Ctrl-q then `d` deletes it.
        press(&mut app, KeyCode::Enter);
        app.handle_key(ctrl_q);
        press(&mut app, KeyCode::Char('d'));
        assert!(app.modals.is_empty());
        assert!(app.comments.comments.is_empty());

        // With nothing to save, Ctrl-q closes and removes the comment without asking.
        press(&mut app, KeyCode::Enter);
        app.handle_key(ctrl_q);
        assert!(app.modals.is_empty());
        assert!(app.comments.comments.is_empty());
    }
}
