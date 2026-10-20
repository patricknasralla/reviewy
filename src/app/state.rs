//! Mode, modal stack and pending file open.

use crate::git::{FileContent, FileInfo};
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
}

/// A file whose content is read and that waits for difft.
pub struct PendingOpen {
    pub index: usize,
    pub content: FileContent,
}
