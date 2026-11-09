# 01: Thread a `Wrap` layout parameter through the wrapping code

**What to build:** a prefactor with no visible change. Soft-wrap layout takes a `Wrap { width, indent }` (spec §11 `wrap.rs`) instead of a bare width, and every place that positions a wrapped segment on screen goes through `Wrap::row_x`, so the continuation offset (task 03, 04) becomes a one-line change. `offset()` returns 0 for now.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `vim/wrap.rs` has `Wrap { width: u16, indent: u16 }` with `offset()` (always 0 in this task) and `row_x(seg)`; `wrap_line`, `line_height`, `to_screen`, `from_screen` take a `Wrap`; `to_screen` returns screen x including `row_x`, `from_screen` takes it.
- [x] Continuation rows are laid out `width - offset()` wide (a no-op while the offset is 0).
- [x] `MotionCtx.wrap` is `Option<Wrap>` at every construction site (`app/actions.rs`, `editor.rs`, `app/comment.rs`, tests).
- [x] Diff view (`app/view.rs` layout and focus, `ui/diff_view.rs` text, selection and cursor) and editor (`segments`, `locate`, `col_at`, `cursor_cell`, `vertical`, `scroll_to_cursor`, `rows`, `ui/editor.rs`) use `Wrap` and add `row_x` where they place a segment's x.
- [x] Existing tests and insta snapshots pass unchanged; `cargo clippy --all-targets -- -D warnings` is clean.
