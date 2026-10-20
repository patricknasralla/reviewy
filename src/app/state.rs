//! Mode, modal stack and pending file open.

use crate::comments::model::CommentSet;
use crate::git::{FileContent, FileInfo};
use crate::ui::editor::Editor;
use crate::vim::motion::Pos;

/// Visual modes store where the selection started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Explorer,
    Normal,
    Visual { anchor: Pos },
    VisualLine { anchor: Pos },
}

pub enum Modal {
    LargeFile {
        pending: PendingOpen,
        info: FileInfo,
    },
    Quit,
    /// Previous session's comments, or the error loading them.
    LoadExisting(Result<CommentSet, String>),
    /// Comment editor; the comment is updated on every edit.
    Editor(Editor),
    DeleteComment(u32),
    CloseComment,
}

/// A file whose content is read and that waits for difft.
pub struct PendingOpen {
    pub index: usize,
    pub content: FileContent,
}
