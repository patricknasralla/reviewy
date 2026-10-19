//! DifftEngine: spawns difftastic, parses its JSON, checks its version.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use super::{DiffEngine, DiffError};
use crate::git::{ChangedFile, DiffTarget, FileStatus};

pub const MIN_VERSION: Version = Version(0, 71, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// `difft --display json` output for one file. Line numbers are 0-based; change offsets are
/// bytes into the raw line (tab = 1 byte).
#[derive(Debug, Deserialize)]
pub struct RawDiff {
    pub status: Status,
    /// `[lhs, rhs]` per row, with one phantom pair past the end. Only for `changed`.
    #[serde(default)]
    pub aligned_lines: Vec<[Option<u32>; 2]>,
    #[serde(default)]
    pub chunks: Vec<Vec<Entry>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Changed,
    Created,
    Deleted,
    Unchanged,
}

#[derive(Debug, Deserialize)]
pub struct Entry {
    pub lhs: Option<SideEntry>,
    pub rhs: Option<SideEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SideEntry {
    pub line_number: u32,
    pub changes: Vec<Change>,
}

#[derive(Debug, Deserialize)]
pub struct Change {
    pub start: usize,
    pub end: usize,
}

pub struct DifftEngine {
    pub binary: PathBuf,
}

impl DiffEngine for DifftEngine {
    fn diff(
        &self,
        root: &Path,
        target: DiffTarget,
        file: &ChangedFile,
    ) -> Result<RawDiff, DiffError> {
        let mut cmd = if file.status == FileStatus::Untracked {
            let mut cmd = Command::new(&self.binary);
            cmd.args(["--display", "json", "/dev/null"]).arg(&file.path);
            cmd
        } else {
            let mut cmd = Command::new("git");
            cmd.args(["diff", target.rev(), "--"]);
            // Both paths, or git doesn't pair them and difft reports `created`.
            if let FileStatus::Renamed { from } = &file.status {
                cmd.arg(from);
            }
            cmd.arg(&file.path)
                .env("GIT_EXTERNAL_DIFF", &self.binary)
                .env("DFT_DISPLAY", "json");
            cmd
        };
        let out = cmd.current_dir(root).env("DFT_UNSTABLE", "yes").output()?;
        if !out.status.success() {
            let args: Vec<_> = std::iter::once(cmd.get_program())
                .chain(cmd.get_args())
                .map(|a| a.to_string_lossy())
                .collect();
            return Err(DiffError::Command {
                cmd: args.join(" "),
                stderr: String::from_utf8_lossy(&out.stderr).trim_end().to_owned(),
            });
        }
        // git skips the external diff when the worktree matches HEAD (staged change reverted).
        if out.stdout.is_empty() {
            return Ok(RawDiff {
                status: Status::Unchanged,
                aligned_lines: Vec::new(),
                chunks: Vec::new(),
            });
        }
        Ok(serde_json::from_slice(&out.stdout)?)
    }
}

pub fn check_version(binary: &Path) -> Result<Version, DiffError> {
    let out = Command::new(binary)
        .arg("--version")
        .output()
        .map_err(|source| DiffError::NotInstalled {
            binary: binary.to_owned(),
            source,
        })?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    check_line(stdout.lines().next().unwrap_or_default())
}

/// Checks the first line of `difft --version`, e.g. `Difftastic 0.71.0`.
fn check_line(line: &str) -> Result<Version, DiffError> {
    let version = parse_version(line).ok_or_else(|| DiffError::BadVersion(line.to_owned()))?;
    if version < MIN_VERSION {
        return Err(DiffError::TooOld(version));
    }
    Ok(version)
}

/// First word in `line` that reads as `X.Y.Z`, ignoring any prefix or suffix on it.
fn parse_version(line: &str) -> Option<Version> {
    line.split_whitespace().find_map(|word| {
        // Skips prefixes like `v` and keeps leading digits only, so `-dev` suffixes parse.
        let mut parts = word
            .trim_start_matches(|c: char| !c.is_ascii_digit())
            .split('.')
            .map(|p| {
                let end = p.find(|c: char| !c.is_ascii_digit()).unwrap_or(p.len());
                p[..end].parse().ok()
            });
        Some(Version(parts.next()??, parts.next()??, parts.next()??))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_check() {
        assert_eq!(check_line("Difftastic 0.71.0").unwrap(), Version(0, 71, 0));
        assert!(matches!(
            check_line("Difftastic 0.70.2"),
            Err(DiffError::TooOld(Version(0, 70, 2)))
        ));
        for line in ["v0.71.0", "difftastic0.71.0", "difft 0.73.0-dev"] {
            assert!(parse_version(line).is_some(), "{line}");
        }
        assert_eq!(parse_version("Difftastic 1.2"), None);
    }
}
