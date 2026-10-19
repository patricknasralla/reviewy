//! Diff engine trait and diff model.

pub mod align;
pub mod difft;
pub mod model;

use std::io;
use std::path::{Path, PathBuf};

use crate::git::{ChangedFile, DiffTarget};
use difft::{MIN_VERSION, RawDiff, Version};

#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error(
        "difftastic not found at `{}`; install it with `cargo install difftastic` (>= {MIN_VERSION})",
        binary.display()
    )]
    NotInstalled { binary: PathBuf, source: io::Error },
    #[error("difftastic {0} is too old; >= {MIN_VERSION} required (`cargo install difftastic`)")]
    TooOld(Version),
    #[error("unexpected `difft --version` output: {0:?}")]
    BadVersion(String),
    #[error("`{cmd}` failed: {stderr}")]
    Command { cmd: String, stderr: String },
    #[error("unexpected difft output: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to run diff: {0}")]
    Io(#[from] io::Error),
}

pub trait DiffEngine {
    fn diff(
        &self,
        root: &Path,
        target: DiffTarget,
        file: &ChangedFile,
    ) -> Result<RawDiff, DiffError>;
}
