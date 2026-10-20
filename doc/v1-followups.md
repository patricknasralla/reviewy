# v1 follow-ups

- **Tabs in comments** (task 06). Excerpts and `Selection::Chars` cols come from tab-expanded `SideText::lines`, so a comment shows spaces where the source has tabs. Not a priority: only cosmetic in `comments.md`. Fix: keep raw (CR-stripped, unexpanded) lines in `SideText`, pass them to `comments::model::draft`, and map display col -> raw char col when building `Selection::Chars` (callers only: `anchor` and the `draft` call in `app/comment.rs`; `comments/` unchanged).
- **`git rm --cached` file listed twice** (task 02). With the file still on disk, it shows as `Deleted` and `Untracked`. Rare; merging the two needs a decision on what to show. Fix in `git::changed_files`.
- **`.gitattributes` `diff=<driver>`** (task 03). A configured `diff.<driver>.command` overrides `GIT_EXTERNAL_DIFF`, so that file fails JSON parsing and shows a per-file error. Rare; the fix (call difft directly on temp files instead of through git) is a bigger change to `diff/difft.rs`.
- **Global install** (developer decision, v1.1). Package an installable crate; the README documents `cargo install --path .` until then.
- **Better Cursor**: Currently the cursor is a non-animating block that is the inverse of the current text color. We should have a Block to line change for normal to insert modes. Both cursors should blink.
- **Word Boundary Wrapping**: if a word is broken by end of column wrap, we should wrap the whole word rather than cut it in two.
- Modal dialogues currently have duplication of intent Save in title and then Save? in body. I should manually edit these.
