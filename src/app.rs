//! Application state and actions.

pub mod actions;
pub mod open;
pub mod state;
pub mod view;

use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::KeyEvent;

use crate::comments::model::{Base, CommentSet};
use crate::config::Config;
use crate::diff::DiffEngine;
use crate::diff::difft::DifftEngine;
use crate::git::{ChangedFile, DiffTarget};
use crate::highlight::{Highlighter, Theme};
use crate::vim::keys::KeyParser;
use state::{Modal, Mode, PendingOpen};
use view::FileView;

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub highlighter: Highlighter,
    pub root: PathBuf,
    pub files: Vec<ChangedFile>,
    /// Opened files, by path.
    pub views: HashMap<PathBuf, FileView>,
    /// Index into `files` of the open file.
    pub current: Option<usize>,
    /// Explorer selection, index into `files`.
    pub explorer: usize,
    pub mode: Mode,
    /// Last is on top and receives input.
    pub modals: Vec<Modal>,
    /// Transient status line message; cleared on the next key press.
    pub status: Option<String>,
    /// Set before diffing so the loop can draw "Diffing…" first; see `finish_open`.
    pub pending: Option<PendingOpen>,
    pub comments: CommentSet,
    /// Terminal (width, height).
    pub size: (u16, u16),
    pub dirty: bool,
    pub should_quit: bool,
    pub keys: KeyParser,
    engine: Box<dyn DiffEngine>,
}

impl App {
    pub fn new(
        config: Config,
        root: PathBuf,
        base_commit: String,
        files: Vec<ChangedFile>,
        theme: Theme,
        status: Option<String>,
    ) -> Self {
        let base = Base {
            ref_name: DiffTarget::WorkingTreeVsHead.rev().to_owned(),
            commit: base_commit,
        };
        Self {
            engine: Box::new(DifftEngine {
                binary: config.difft.binary.clone(),
            }),
            config,
            highlighter: Highlighter::new(&theme),
            theme,
            root,
            files,
            views: HashMap::new(),
            current: None,
            explorer: 0,
            mode: Mode::Explorer,
            modals: Vec::new(),
            status,
            pending: None,
            comments: CommentSet::new(base),
            size: (0, 0),
            dirty: true,
            should_quit: false,
            keys: KeyParser::default(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.status = None;
        if let Some(action) = self.keys.handle(key).and_then(|input| self.map(input)) {
            self.apply(action);
        }
        self.dirty = true;
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        self.refresh_layout();
        self.dirty = true;
    }

    pub fn current_file(&self) -> Option<&ChangedFile> {
        self.current.map(|i| &self.files[i])
    }

    pub fn view(&self) -> Option<&FileView> {
        self.views.get(&self.current_file()?.path)
    }

    fn view_mut(&mut self) -> Option<&mut FileView> {
        let path = &self.files[self.current?].path;
        self.views.get_mut(path)
    }

    /// Diff view height: the terminal minus the status line.
    fn view_height(&self) -> usize {
        usize::from(self.size.1.saturating_sub(1))
    }

    /// Recomputes the open file's row heights if needed and scrolls the cursor into view.
    fn refresh_layout(&mut self) {
        let (width, height, wrap) = (self.size.0, self.view_height(), self.config.ui.wrap);
        if let Some(view) = self.view_mut() {
            view.refresh_layout(width, height, wrap);
        }
    }
}
