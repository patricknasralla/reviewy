//! Mode, modal stack and pending file open.

use crate::git::{FileContent, FileInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Explorer,
    Row,
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
