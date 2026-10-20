# Task 02 - Git integration

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

Add startup steps from §4 at the `// task 02:` markers task 01 left: base commit (step 3) and collecting changed files (step 6). No changed files → print "No changes" and exit 0 before entering the terminal. Store the base commit and the file list where the `App` stub can take them (task 07 builds the real `App`).

Remove the `#[allow(dead_code)]` on `LimitsConfig` and `IgnoreConfig` in `config.rs` once you read them.

## Tests

Unit tests over inline byte strings (no git repo needed):
- `parse_porcelain_v2`: one input containing a modified file, staged add, delete, rename (with the orig-path field), untracked, unmerged, and submodule record. Example lines are in `doc/spec.md` §6 and can be taken from real `git status` output.
- `parse_numstat`: binary, `0\t0`, normal counts, and a rename record (`0\t0\t\0old\0new\0`).
- Mode-only classification: a `1 .M` record whose `mH` and `mW` differ, plus numstat `0\t0` → `ModeOnly`.

## Acceptance

- In a repo with changes, startup collects the files without errors (log them at `debug`; run with `REVIEWY_LOG=debug` to get `.reviewy/reviewy.log`, logging is opt-in).
- In a clean repo, `reviewy` prints "No changes" and exits 0.
- In a repo with no commits, it prints the no-commits error.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

`git.rs` re-exports everything below, so callers use `git::…`.

- `git::GitError::{NotARepo, NoCommits, Command { cmd, stderr }, Read { path, source }, Glob(globset::Error), Parse(String), Io(io::Error)}`.
- `git::DiffTarget::WorkingTreeVsHead` (`Clone + Copy`), `fn rev(self) -> &'static str` (`"HEAD"`).
- `git::base_commit(root: &Path) -> Result<String, GitError>`.
- `git/status.rs`: `FileStatus`, `ChangedFile { path, status, binary }` + `viewable()`, `build_ignore(&[String]) -> Result<GlobSet, GitError>`, `changed_files(root: &Path, target: DiffTarget, ignore: &GlobSet) -> Result<Vec<ChangedFile>, GitError>`.
- `git/content.rs`: `FileContent { old, new }`, `FileInfo { lines, bytes, longest_line }` (renamed from `LargeFileInfo`), `read_content(root: &Path, target: DiffTarget, file: &ChangedFile) -> Result<FileContent, GitError>`, `check_limits(&FileContent, &LimitsConfig) -> Option<FileInfo>`.
- Private helpers in `git.rs`: `git(root) -> Command`, `run_command(&mut Command) -> Result<Vec<u8>, GitError>` (stdout, non-zero exit → `GitError::Command`), `stdout_or_error(&Command, Output)` for callers that inspect the exit code first.
- `App` gained `base_commit: String`, `files: Vec<ChangedFile>`; built in `main` and passed to `run(terminal, terminate, app)`.

### Decisions and deviations

- **`git.rs` split** into `git/status.rs` and `git/content.rs` (spec §5 updated).
- **`1` records are classified by modes, not XY** (deviates from this task's XY rule; spec §6 updated): `mH`/`mW` both zero → dropped (e.g. `AD`: no change vs HEAD); `mH` zero → `Added` (this catches intent-to-add `.A`, which the XY rule called `Modified`, so reading the old side failed); `mW` zero → `Deleted`; else `Modified`. Submodule check still comes first.
- **`DiffTarget` is a parameter of `read_content`** too (the task's signature omitted it). The old side uses `target.rev()` (`HEAD`), not the startup SHA; HEAD moving mid-session is out of scope for v1.
- **Old side via `git cat-file blob <rev>:<path>`** instead of `git show` (plumbing; unaffected by user config). Same exit-128 behaviour for a missing path → `GitError::Command`.
- **Symlinks:** the new side is the link's target path (`read_link`), matching what git stores and what difft gets; git's numstat also counts an untracked symlink as its link text.
- **`GIT_OPTIONAL_LOCKS=0`** on `git status`, so its index refresh doesn't make a concurrent git command fail on `index.lock`.
- **No commits** detected with `git rev-parse --verify -q HEAD` (exit 1, empty stdout).
- **Binary detection is git's for every file** (deviates from this task's NUL-probe step 3; spec §6 updated): untracked files get a per-file `git diff --no-index --numstat -z -- /dev/null <path>` whose output goes through the same `parse_numstat`, so `.gitattributes` (`binary`, `-diff`) is honoured for untracked files too. `classify` is the only place `binary` is set. Cost: one git process per untracked file at startup. Neither `diff.external` nor textconv affects `--numstat` (checked on git 2.53). Failures (e.g. unreadable file) are logged at `warn` and treated as text; opening the file surfaces the error.
- **`check_limits`** reports each field as the max over both sides. Lines are counted with `str::lines()` (a trailing newline adds no line, as in task 03).
- **Ignore globs** use `globset` defaults (`*` crosses `/`, so `*.lock` matches nested lockfiles) and match the new path only.
- **Paths** are converted bytes ↔ `PathBuf` via `OsStrExt` (no lossy conversion); this makes `git` Unix-only.
- **Dead code:** items first used by task 07 carry `#[expect(…, reason = "used by task 07")]`: `mod content` and its `pub use` in `git.rs`, `ChangedFile::viewable`, `App::{base_commit, files}`. Task 07 file updated to remove them.

### Left undone / accepted

- After `git rm --cached <f>` with the file still on disk, `<f>` is listed twice (`Deleted` and `Untracked`). Accepted for v1.
- A record present in status but absent from numstat (staged change reverted in the worktree) is listed as `Modified`; `git diff HEAD` prints nothing for it (difft is never called), and task 03 maps empty output to `unchanged`.
