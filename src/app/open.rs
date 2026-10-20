//! Open-file pipeline: content, limits, difft, alignment, highlighting.

use super::App;
use super::state::{Modal, Mode, PendingOpen};
use super::view::FileView;
use crate::diff::align;
use crate::diff::model::FileDiff;
use crate::git::{self, ChangedFile, DiffTarget, FileStatus};

impl App {
    /// Switches to a cached file, or reads it and either asks (large file) or queues the diff.
    pub fn open(&mut self, index: usize) {
        let file = &self.files[index];
        if self.views.contains_key(&file.path) {
            self.show(index);
            return;
        }
        if let Some(reason) = unviewable(file) {
            self.status = Some(reason.into());
            return;
        }
        match git::read_content(&self.root, DiffTarget::WorkingTreeVsHead, file) {
            Err(e) => self.status = Some(format!("{}: {e}", file.path.display())),
            Ok(content) => {
                let pending = PendingOpen { index, content };
                match git::check_limits(&pending.content, &self.config.limits) {
                    Some(info) => self.modals.push(Modal::LargeFile { pending, info }),
                    None => self.start_diff(pending),
                }
            }
        }
    }

    /// Queues the diff; the event loop draws the status, then calls `finish_open`.
    pub fn start_diff(&mut self, pending: PendingOpen) {
        self.status = Some("Diffing…".into());
        self.pending = Some(pending);
    }

    pub fn finish_open(&mut self) {
        let Some(PendingOpen { index, content }) = self.pending.take() else {
            return;
        };
        let file = &self.files[index];
        match self
            .engine
            .diff(&self.root, DiffTarget::WorkingTreeVsHead, file)
        {
            Err(e) => self.status = Some(format!("{}: {e}", file.path.display())),
            Ok(raw) => {
                let mut diff = align::build(&raw, &content, self.config.ui.tab_width);
                self.highlighter.apply(&mut diff, file);
                self.status = None;
                self.insert_view(index, diff);
            }
        }
        self.dirty = true;
    }

    /// Caches `diff` for file `index` and shows it.
    pub fn insert_view(&mut self, index: usize, diff: FileDiff) {
        let path = self.files[index].path.clone();
        self.views.insert(path, FileView::new(diff));
        self.show(index);
    }

    fn show(&mut self, index: usize) {
        self.current = Some(index);
        self.explorer = index;
        self.mode = Mode::Row;
        self.refresh_layout();
    }
}

fn unviewable(file: &ChangedFile) -> Option<&'static str> {
    if file.binary {
        return Some("Binary file");
    }
    match file.status {
        FileStatus::Conflicted => Some("Conflicted file: not viewable in v1"),
        FileStatus::Submodule => Some("Submodule: not viewable in v1"),
        FileStatus::ModeOnly => Some("Mode change only: not viewable in v1"),
        _ => None,
    }
}
