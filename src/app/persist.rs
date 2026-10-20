//! Loading the previous session's comments and saving them.

use super::App;
use super::state::Modal;
use crate::comments::CommentsError;
use crate::comments::model::CommentSet;
use crate::comments::store;

impl App {
    /// Shows the "Load existing" modal unless there was no `comments.json`.
    pub fn load_existing(&mut self, loaded: Result<Option<CommentSet>, CommentsError>) {
        if let Some(loaded) = loaded.transpose() {
            let loaded = loaded.map_err(|e| e.to_string());
            self.modals.push(Modal::LoadExisting(loaded));
        }
    }

    /// Replaces the comments with a loaded set, rebased on the current commit.
    pub(super) fn keep(&mut self, mut set: CommentSet) {
        set.base = self.comments.base.clone();
        self.comments = set;
    }

    /// Writes (or deletes) the comment files, without empty-body comments; safe to call mid-session.
    /// Does nothing while the "Load existing" modal is unanswered, so the previous files survive.
    pub fn save(&self) -> Result<(), CommentsError> {
        if self
            .modals
            .iter()
            .any(|m| matches!(m, Modal::LoadExisting(_)))
        {
            return Ok(());
        }
        let mut set = self.comments.clone();
        set.comments.retain(|c| !c.body.trim().is_empty());
        store::save(
            &self.root.join(".reviewy"),
            &set,
            &self.config.comments.preamble,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crossterm::event::{KeyCode, KeyEvent};

    use super::*;
    use crate::comments::model::{self, Anchor, Base, Selection, Side};
    use crate::config;
    use crate::highlight::Theme;

    fn app(root: PathBuf) -> App {
        let config = config::from_sources(&[]).unwrap();
        let theme = Theme::bundled_default(config.ui.tint);
        App::new(config, root, "c".repeat(40), Vec::new(), theme, None)
    }

    fn add(set: &mut CommentSet, body: &str) -> u32 {
        let anchor = Anchor {
            file: "a.rs".into(),
            side: Side::New,
            selection: Selection::Line { line: 1 },
        };
        set.add(model::draft(anchor, None, &["x".into()], 0, body.into()))
    }

    #[test]
    fn save_drops_empty_bodies() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".reviewy")).unwrap();
        let mut app = app(dir.path().to_owned());
        let empty = add(&mut app.comments, " \n");
        let kept = add(&mut app.comments, "Fix this.");

        app.save().unwrap();
        let json = fs::read_to_string(dir.path().join(".reviewy/comments.json")).unwrap();
        let saved: CommentSet = serde_json::from_str(&json).unwrap();
        let ids: Vec<_> = saved.comments.iter().map(|c| c.id).collect();
        assert_eq!(ids, [kept]);
        assert!(app.comments.get(empty).is_some(), "in-memory set unchanged");
    }

    #[test]
    fn keep_preserves_ids_and_next_id() {
        let mut app = app(Path::new("").to_owned());
        let mut loaded = CommentSet::new(Base {
            ref_name: "HEAD".into(),
            commit: "o".repeat(40),
        });
        for body in ["a", "b", "c"] {
            add(&mut loaded, body);
        }
        loaded.remove(2);
        app.load_existing(Ok(Some(loaded)));

        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(app.modals.is_empty());
        let ids: Vec<_> = app.comments.comments.iter().map(|c| c.id).collect();
        assert_eq!(ids, [1, 3]);
        assert_eq!(app.comments.base.commit, "c".repeat(40));
        assert_eq!(add(&mut app.comments, "d"), 4);
    }
}
