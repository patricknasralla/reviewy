//! Git CLI integration.

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("not a git repository")]
    NotARepo,
    #[error("failed to run git: {0}")]
    Io(#[from] std::io::Error),
}

pub fn repo_root() -> Result<PathBuf, GitError> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !out.status.success() {
        return Err(GitError::NotARepo);
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim_end(),
    ))
}
