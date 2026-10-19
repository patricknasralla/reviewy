# Task 02 — Git integration

## Goal

Implement `src/git.rs`: everything Reviewy needs from git, via the `git` CLI. Wire the git-related startup steps into `main.rs`.

Read first: `doc/spec.md` §6 (all of it), §4 (startup sequence), §13 (`[ignore]`, `[limits]`). Read the handoff notes in `doc/tasks/01-scaffold.md`.

## Context

Reviewy shows the working tree vs `HEAD` for every changed file. `git.rs` lists the changed files, classifies them, and fetches each side's content. Diffing itself (difftastic) is task 03 and does not belong here. `repo_root()` already exists from task 01.

All commands run with `std::process::Command`, `current_dir(repo_root)`. Capture stdout as bytes. A non-zero exit becomes a `GitError` carrying the command and stderr.

## Deliverables

### Types

```rust
pub enum DiffTarget { WorkingTreeVsHead }   // v1.1 adds variants; thread it through, don't hard-code HEAD

pub enum FileStatus {
    Modified, Added, Deleted,
    Renamed { from: PathBuf },
    Untracked, Conflicted, Submodule, ModeOnly,
}

pub struct ChangedFile {
    pub path: PathBuf,      // repo-relative; new path for renames
    pub status: FileStatus,
    pub binary: bool,
}

impl ChangedFile {
    /// False for binary, conflicted, submodule and mode-only files. The UI shows a status message instead of opening them.
    pub fn viewable(&self) -> bool;
}

pub struct FileContent {
    pub old: Option<String>,   // None for added/untracked
    pub new: Option<String>,   // None for deleted
}

pub struct LargeFileInfo { pub lines: usize, pub bytes: usize, pub longest_line: usize }
```

### Functions

- `base_commit(root) -> Result<String, GitError>`: `git rev-parse HEAD`. No commits → a distinct error variant ("repository has no commits; v1 needs at least one").
- `changed_files(root, target, ignore: &GlobSet) -> Result<Vec<ChangedFile>, GitError>`:
  1. `git status --porcelain=v2 -z --untracked-files=all`, parsed per the record formats in §6. Keep git's order.
  2. `git diff HEAD --numstat -z`, used for `binary` (`-\t-`) and `ModeOnly` (`0\t0`, file modes differ, not a rename).
  3. Untracked files: `binary` if a NUL byte is in the first 8000 bytes (read only that prefix).
  4. Drop paths matching the `ignore` globs. Build the `GlobSet` from `config.ignore.paths` in a helper here (`build_ignore(&[String]) -> Result<GlobSet, GitError>`).
- Split each step into a pure parsing function over `&[u8]` (`parse_porcelain_v2`, `parse_numstat`) and the code that runs the command, so parsing is testable without a repo.
- XY classification for `1` records: `D` in either position → `Deleted`; `A` in the index position → `Added`; otherwise `Modified` (unless numstat says `ModeOnly`). `2` records → `Renamed`. `u` → `Conflicted`. Submodule check (`sub` starts with `S`) takes precedence over the XY codes.
- `read_content(root, file: &ChangedFile) -> Result<FileContent, GitError>`:
  - Old side: `git show HEAD:<path>` (`HEAD:<from>` for renames). Exit 128 for a missing path is an error, not `None`; `None` comes only from status (added/untracked).
  - New side: read the working-tree file.
  - Bytes → `String` with `String::from_utf8_lossy`. **No** CRLF or tab normalization here; task 03 does it together with the diff-offset conversion.
- `check_limits(content: &FileContent, limits: &LimitsConfig) -> Option<LargeFileInfo>`: `Some` if either side exceeds `max_lines`, `max_bytes` or `max_line_length` (chars). The info reports the larger side's numbers.

The open-file order the app will use (task 07): `read_content` → `check_limits` → (large-file modal) → diff (task 03). Nothing in this task calls difft.

### `main.rs`

Add startup steps from §4: base commit (step 3) and collecting changed files (step 6). No changed files → print "No changes" and exit 0 before entering the terminal. Store the base commit and the file list where the `App` stub can take them (task 07 builds the real `App`).

## Tests

Unit tests over inline byte strings (no git repo needed):
- `parse_porcelain_v2`: one input containing a modified file, staged add, delete, rename (with the orig-path field), untracked, unmerged, and submodule record. Example lines are in `doc/spec.md` §6 and can be taken from real `git status` output.
- `parse_numstat`: binary, `0\t0`, normal counts, and a rename record (`0\t0\t\0old\0new\0`).
- Mode-only classification: a `1 .M` record whose `mH` and `mW` differ, plus numstat `0\t0` → `ModeOnly`.

## Acceptance

- In a repo with changes, startup collects the files without errors (log them at `debug` to `.reviewy/reviewy.log` to check).
- In a clean repo, `reviewy` prints "No changes" and exits 0.
- In a repo with no commits, it prints the no-commits error.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
