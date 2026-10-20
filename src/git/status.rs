//! Changed-file listing: `git status` and numstat parsing, classification, ignore globs.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use tracing::warn;

use super::{DiffTarget, GitError, git, run_command, stdout_or_error};

const NO_MODE: &[u8] = b"000000";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed {
        from: PathBuf,
    },
    Untracked,
    Conflicted,
    Submodule,
    /// File mode changed, content identical.
    ModeOnly,
}

#[derive(Debug, Clone)]
pub struct ChangedFile {
    /// Repo-relative; the new path for renames.
    pub path: PathBuf,
    pub status: FileStatus,
    pub binary: bool,
}

impl ChangedFile {
    /// False for binary, conflicted, submodule and mode-only files. The UI shows a status message instead of opening them.
    pub fn viewable(&self) -> bool {
        !self.binary
            && !matches!(
                self.status,
                FileStatus::Conflicted | FileStatus::Submodule | FileStatus::ModeOnly
            )
    }
}

#[derive(Debug)]
struct Record {
    path: PathBuf,
    status: FileStatus,
    mode_changed: bool,
}

struct Numstat {
    binary: bool,
    zero: bool,
}

pub fn changed_files(
    root: &Path,
    target: DiffTarget,
    ignore: &GlobSet,
) -> Result<Vec<ChangedFile>, GitError> {
    // Optional locks off so a concurrent git command isn't blocked by our index refresh.
    let status = run_command(git(root).env("GIT_OPTIONAL_LOCKS", "0").args([
        "status",
        "--porcelain=v2",
        "-z",
        "--untracked-files=all",
    ]))?;
    let mut records = parse_porcelain_v2(&status)?;
    records.retain(|r| !ignore.is_match(&r.path));
    let tracked = run_command(git(root).args(["diff", "--numstat", "-z", target.rev(), "--"]))?;
    let mut numstat = parse_numstat(&tracked)?;
    for record in records.iter().filter(|r| r.status == FileStatus::Untracked) {
        match untracked_numstat(root, &record.path) {
            Ok(stat) => numstat.extend(stat),
            // Treated as text; opening the file reports the error.
            Err(e) => warn!(path = %record.path.display(), "untracked numstat failed: {e}"),
        }
    }
    Ok(classify(records, &numstat))
}

/// Untracked files aren't in the tracked numstat pass; diffing against `/dev/null` lets git
/// decide `binary` for them too, `.gitattributes` included.
fn untracked_numstat(root: &Path, path: &Path) -> Result<HashMap<PathBuf, Numstat>, GitError> {
    let mut cmd = git(root);
    cmd.args(["diff", "--no-index", "--numstat", "-z", "--", "/dev/null"])
        .arg(path);
    let out = cmd.output()?;
    // `--no-index` exits 1 when the files differ, which is always the case here.
    let stdout = if out.status.code() == Some(1) {
        out.stdout
    } else {
        stdout_or_error(&cmd, out)?
    };
    parse_numstat(&stdout)
}

pub fn build_ignore(globs: &[String]) -> Result<GlobSet, GitError> {
    let mut builder = GlobSetBuilder::new();
    for glob in globs {
        builder.add(Glob::new(glob)?);
    }
    Ok(builder.build()?)
}

/// Parses `git status --porcelain=v2 -z`. `1` records are classified by their HEAD and
/// worktree modes rather than XY, so intent-to-add (`.A`) is `Added` and entries with no
/// change against HEAD (`AD`) are dropped.
fn parse_porcelain_v2(out: &[u8]) -> Result<Vec<Record>, GitError> {
    let mut records = Vec::new();
    let mut entries = out.split(|&b| b == 0).filter(|e| !e.is_empty());
    while let Some(entry) = entries.next() {
        let bad = || GitError::Parse(format!("status: {}", String::from_utf8_lossy(entry)));
        let (path, status, mode_changed) = match entry[0] {
            b'1' => {
                // 1 XY sub mH mI mW hH hI path
                let f = fields(entry, 9).ok_or_else(bad)?;
                let (head, worktree) = (f[3], f[5]);
                let status = if is_submodule(f[2]) {
                    FileStatus::Submodule
                } else {
                    match (head == NO_MODE, worktree == NO_MODE) {
                        (true, true) => continue,
                        (true, false) => FileStatus::Added,
                        (false, true) => FileStatus::Deleted,
                        (false, false) => FileStatus::Modified,
                    }
                };
                (f[8], status, head != worktree)
            }
            b'2' => {
                // 2 XY sub mH mI mW hH hI score path \0 origPath
                let f = fields(entry, 10).ok_or_else(bad)?;
                let from = entries.next().ok_or_else(bad)?;
                let status = if is_submodule(f[2]) {
                    FileStatus::Submodule
                } else {
                    FileStatus::Renamed {
                        from: to_path(from),
                    }
                };
                (f[9], status, false)
            }
            b'u' => {
                // u XY sub m1 m2 m3 mW h1 h2 h3 path
                let f = fields(entry, 11).ok_or_else(bad)?;
                let status = if is_submodule(f[2]) {
                    FileStatus::Submodule
                } else {
                    FileStatus::Conflicted
                };
                (f[10], status, false)
            }
            b'?' => (
                entry.get(2..).ok_or_else(bad)?,
                FileStatus::Untracked,
                false,
            ),
            b'#' | b'!' => continue,
            _ => return Err(bad()),
        };
        records.push(Record {
            path: to_path(path),
            status,
            mode_changed,
        });
    }
    Ok(records)
}

/// Parses `git diff --numstat -z`, keyed by the new path.
fn parse_numstat(out: &[u8]) -> Result<HashMap<PathBuf, Numstat>, GitError> {
    let mut stats = HashMap::new();
    let mut entries = out.split(|&b| b == 0).filter(|e| !e.is_empty());
    while let Some(entry) = entries.next() {
        let bad = || GitError::Parse(format!("numstat: {}", String::from_utf8_lossy(entry)));
        let f = fields_by(entry, 3, b'\t').ok_or_else(bad)?;
        // Renames: `added\tdeleted\t\0old\0new`.
        let path = if f[2].is_empty() {
            entries.next().ok_or_else(bad)?;
            entries.next().ok_or_else(bad)?
        } else {
            f[2]
        };
        let numstat = Numstat {
            binary: f[0] == b"-" && f[1] == b"-",
            zero: f[0] == b"0" && f[1] == b"0",
        };
        stats.insert(to_path(path), numstat);
    }
    Ok(stats)
}

/// Sets `binary` from numstat and turns content-identical mode changes into `ModeOnly`.
fn classify(records: Vec<Record>, numstat: &HashMap<PathBuf, Numstat>) -> Vec<ChangedFile> {
    records
        .into_iter()
        .map(|r| {
            let stat = numstat.get(&r.path);
            let mode_only =
                r.status == FileStatus::Modified && r.mode_changed && stat.is_some_and(|s| s.zero);
            ChangedFile {
                status: if mode_only {
                    FileStatus::ModeOnly
                } else {
                    r.status
                },
                binary: stat.is_some_and(|s| s.binary),
                path: r.path,
            }
        })
        .collect()
}

fn is_submodule(sub: &[u8]) -> bool {
    sub.starts_with(b"S")
}

/// Splits into exactly `n` space-separated fields; the last keeps any spaces (paths).
fn fields(entry: &[u8], n: usize) -> Option<Vec<&[u8]>> {
    fields_by(entry, n, b' ')
}

fn fields_by(entry: &[u8], n: usize, sep: u8) -> Option<Vec<&[u8]>> {
    let f: Vec<_> = entry.splitn(n, |&b| b == sep).collect();
    (f.len() == n).then_some(f)
}

fn to_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(OsStr::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: &str = "1111111111111111111111111111111111111111";
    const Z: &str = "0000000000000000000000000000000000000000";

    fn statuses(records: &[Record]) -> Vec<(&str, &FileStatus)> {
        records
            .iter()
            .map(|r| (r.path.to_str().unwrap(), &r.status))
            .collect()
    }

    #[test]
    fn porcelain_v2_records() {
        let out = [
            format!("1 .M N... 100644 100644 100644 {H} {H} src/main.rs"),
            format!("1 A. N... 000000 100644 100644 {Z} {H} new.rs"),
            format!("1 D. N... 100644 000000 000000 {H} {Z} gone.rs"),
            format!("2 R. N... 100644 100644 100644 {H} {H} R100 new name.rs\0old name.rs"),
            "? notes.txt".to_owned(),
            format!("u UU N... 100644 100644 100644 100644 {H} {H} {H} conflict.rs"),
            format!("1 .M SC.. 160000 160000 160000 {H} {H} vendor/lib"),
            // Intent-to-add: `A` in the worktree slot, absent from HEAD.
            format!("1 .A N... 000000 000000 100644 {Z} {Z} ita.rs"),
            // Added then deleted: no change against HEAD.
            format!("1 AD N... 000000 100644 000000 {Z} {H} ad.rs"),
        ]
        .join("\0")
            + "\0";
        let records = parse_porcelain_v2(out.as_bytes()).unwrap();
        let renamed = FileStatus::Renamed {
            from: "old name.rs".into(),
        };
        assert_eq!(
            statuses(&records),
            [
                ("src/main.rs", &FileStatus::Modified),
                ("new.rs", &FileStatus::Added),
                ("gone.rs", &FileStatus::Deleted),
                ("new name.rs", &renamed),
                ("notes.txt", &FileStatus::Untracked),
                ("conflict.rs", &FileStatus::Conflicted),
                ("vendor/lib", &FileStatus::Submodule),
                ("ita.rs", &FileStatus::Added),
            ]
        );
    }

    #[test]
    fn numstat_records() {
        let out = b"-\t-\timg.png\x000\t0\trun.sh\x003\t1\tsrc/main.rs\x002\t1\t\0old.rs\0new.rs\0";
        let stats = parse_numstat(out).unwrap();
        let get = |p: &str| &stats[Path::new(p)];
        assert!(get("img.png").binary && !get("img.png").zero);
        assert!(get("run.sh").zero && !get("run.sh").binary);
        assert!(!get("src/main.rs").zero && !get("src/main.rs").binary);
        assert!(!get("new.rs").zero);
        assert!(!stats.contains_key(Path::new("old.rs")));
    }

    #[test]
    fn mode_change_without_content_change_is_mode_only() {
        let status = format!("1 .M N... 100644 100644 100755 {H} {H} run.sh\0");
        let records = parse_porcelain_v2(status.as_bytes()).unwrap();
        let files = classify(records, &parse_numstat(b"0\t0\trun.sh\0").unwrap());
        assert_eq!(files[0].status, FileStatus::ModeOnly);
    }
}
