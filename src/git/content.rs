//! Both sides of a changed file, and the large-file check.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;

use super::{ChangedFile, DiffTarget, FileStatus, GitError, git, run_command};
use crate::config::LimitsConfig;

/// Raw text of each side; no CRLF or tab normalization.
pub struct FileContent {
    /// `None` for added/untracked files.
    pub old: Option<String>,
    /// `None` for deleted files.
    pub new: Option<String>,
}

#[derive(Debug, Default)]
pub struct FileInfo {
    pub lines: usize,
    pub bytes: usize,
    pub longest_line: usize,
}

pub fn read_content(
    root: &Path,
    target: DiffTarget,
    file: &ChangedFile,
) -> Result<FileContent, GitError> {
    let old = match &file.status {
        FileStatus::Added | FileStatus::Untracked => None,
        FileStatus::Renamed { from } => Some(blob(root, target, from)?),
        _ => Some(blob(root, target, &file.path)?),
    };
    let new = match file.status {
        FileStatus::Deleted => None,
        _ => Some(worktree(&root.join(&file.path))?),
    };
    Ok(FileContent { old, new })
}

/// `Some` if either side exceeds a limit; each field is the max over both sides.
pub fn check_limits(content: &FileContent, limits: &LimitsConfig) -> Option<FileInfo> {
    let info = [&content.old, &content.new]
        .into_iter()
        .flatten()
        .map(|text| FileInfo {
            lines: text.lines().count(),
            bytes: text.len(),
            longest_line: text.lines().map(|l| l.chars().count()).max().unwrap_or(0),
        })
        .fold(FileInfo::default(), |a, b| FileInfo {
            lines: a.lines.max(b.lines),
            bytes: a.bytes.max(b.bytes),
            longest_line: a.longest_line.max(b.longest_line),
        });
    (info.lines > limits.max_lines
        || info.bytes > limits.max_bytes
        || info.longest_line > limits.max_line_length)
        .then_some(info)
}

fn blob(root: &Path, target: DiffTarget, path: &Path) -> Result<String, GitError> {
    let mut spec = OsString::from(format!("{}:", target.rev()));
    spec.push(path);
    let bytes = run_command(git(root).args(["cat-file", "blob"]).arg(spec))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn worktree(path: &Path) -> Result<String, GitError> {
    let read = || -> io::Result<Vec<u8>> {
        // git stores a symlink as its target path; show that, not the target's content.
        if fs::symlink_metadata(path)?.is_symlink() {
            return Ok(fs::read_link(path)?.into_os_string().into_vec());
        }
        fs::read(path)
    };
    let bytes = read().map_err(|source| GitError::Read {
        path: path.to_owned(),
        source,
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
