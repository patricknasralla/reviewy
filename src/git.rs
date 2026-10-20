//! Git CLI integration.

pub mod content;
pub mod status;

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub use content::{FileContent, FileInfo, check_limits, read_content};
pub use status::{ChangedFile, FileStatus, build_ignore, changed_files};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("not a git repository")]
    NotARepo,
    #[error("repository has no commits; v1 needs at least one")]
    NoCommits,
    #[error("`git {cmd}` failed: {stderr}")]
    Command { cmd: String, stderr: String },
    #[error("reading {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("invalid ignore glob: {0}")]
    Glob(#[from] globset::Error),
    #[error("unexpected git output: {0}")]
    Parse(String),
    #[error("failed to run git: {0}")]
    Io(#[from] io::Error),
}

/// What the working tree is compared against. v1.1 adds variants.
#[derive(Debug, Clone, Copy)]
pub enum DiffTarget {
    WorkingTreeVsHead,
}

impl DiffTarget {
    pub fn rev(self) -> &'static str {
        match self {
            DiffTarget::WorkingTreeVsHead => "HEAD",
        }
    }
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

pub fn base_commit(root: &Path) -> Result<String, GitError> {
    let mut cmd = git(root);
    cmd.args(["rev-parse", "--verify", "-q", "HEAD"]);
    let out = cmd.output()?;
    // `--verify -q` exits 1 silently when HEAD doesn't resolve.
    if out.status.code() == Some(1) && out.stdout.is_empty() {
        return Err(GitError::NoCommits);
    }
    Ok(String::from_utf8_lossy(&stdout_or_error(&cmd, out)?)
        .trim_end()
        .to_owned())
}

fn git(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(root);
    cmd
}

fn run_command(cmd: &mut Command) -> Result<Vec<u8>, GitError> {
    let out = cmd.output()?;
    stdout_or_error(cmd, out)
}

fn stdout_or_error(cmd: &Command, out: std::process::Output) -> Result<Vec<u8>, GitError> {
    if out.status.success() {
        return Ok(out.stdout);
    }
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    Err(GitError::Command {
        cmd: args.join(" "),
        stderr: String::from_utf8_lossy(&out.stderr).trim_end().to_owned(),
    })
}
