# Task 12 - Cursor rendering

## Goal

Replace the two inconsistent cursors (a reversed cell in the diff view, the terminal's default cursor in the editor) with Zed-style cursors: a drawn, blinking block colored by the theme, and a blinking terminal bar for text insertion.

Read first: `doc/spec.md` §4 (event loop), §9 (Themes), §10 (Diff view, Comment editor), §13. Source: `src/main.rs`, `src/app.rs`, `src/highlight/theme.rs`, `src/ui/diff_view.rs` (cursor drawing around line 238), `src/ui/editor.rs` (`draw`), `src/config.rs`, `src/defaults/defaults.toml`.

## Context

First of four tasks (12-15) that lead to modal editing in the comment editor. Target terminal is ghostty (24-bit color is already required); no fallbacks for terminals without OSC 12 or cursor shapes.

The editor is still insert-only in this task, so it always shows the bar. Task 13 adds normal mode and switches the editor to the block there.

## Deliverables

### Theme

- `Theme::cursor: Color` from `players[0].cursor`, alpha-blended like the other keys. Fallback when missing: `editor.foreground`.

### Block cursor (drawn)

- One shared helper used by every drawn cursor (e.g. `ui::cursor_block(buf, pos, theme)`): cell background `theme.cursor`, foreground `theme.background`. The cell keeps its char; its syntax color is replaced.
- The diff view uses it instead of `Modifier::REVERSED`.
- Not drawn in the blink's off phase.

### Blink

- `App` holds the blink phase and the time of the next toggle.
- The event loop polls with a timeout of `min(250 ms, time to next toggle)`. On a toggle: flip the phase, set `dirty`.
- Any key or paste resets the phase to visible and restarts the interval.
- Config in `[ui]`: `cursor_blink = true`, `cursor_blink_ms = 500` (`defaults.toml` only, no code constants). With `cursor_blink = false` the block is always visible and the loop never wakes for it.

### Bar cursor (terminal)

- The terminal cursor is only ever a bar (blocks are drawn), so its shape and color are set once at startup, not per frame:
  - `SetCursorStyle::BlinkingBar` (`SteadyBar` when `cursor_blink = false`). The terminal blinks it at its own rate.
  - Color via OSC 12: `\x1b]12;#rrggbb\x07` with `theme.cursor`.
- Restored wherever the terminal is restored (normal exit, error, panic hook): `SetCursorStyle::DefaultUserShape` and OSC 112 (`\x1b]112\x07`).
- The editor keeps using `frame.set_cursor_position`.

## Tests

- Theme: `players[0].cursor` parsed; missing falls back to `editor.foreground`.
- Existing snapshots: render with the block visible; update snapshots if the output changes.

## Acceptance

- In ghostty: diff view cursor is a `players[0].cursor` block with background-colored text, blinking at `cursor_blink_ms`, solid while keys are pressed. Editor shows a blinking bar in the cursor color. Exiting restores the terminal's own cursor shape and color.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `Theme::cursor: Color` (`players[0].cursor`, fallback `editor.foreground`). `Variant::selection()` became `Variant::player(key)`.
- `UiConfig::{cursor_blink: bool, cursor_blink_ms: u64}`.
- `App`: `pub cursor_on: bool` (blink phase, starts visible), private `blink_at: Instant`; `diff_focused() -> bool` (not Explorer, no modal), `next_blink() -> Option<Instant>` (`None` unless blinking and a block is shown), `blink(now)` (toggles when due, sets `dirty`), private `reset_blink()` (called from `handle_key` and `paste`).
- `ui::cursor_block(buf, Position, &Theme)`: private to `ui`, so only its child modules (diff view, editor) use it. Sets fg `theme.background`, bg `theme.cursor`.
- `main::enter_terminal(&Theme, blink: bool)`: sets the bar shape and OSC 12 color. `restore_terminal()` always sends `DefaultUserShape` and OSC 112.

### Decisions (developer decisions; spec §4, §9, §10, §13 and README updated)

- The block is drawn only while the diff view has focus (no explorer, no modal); the blink timer runs only then, so nothing redraws for blinking under the explorer or a modal.
- Poll timeout is `min(250 ms, time to next blink)`, so the blink interval is exact and not rounded to 250 ms ticks.
- The terminal cursor is only ever the bar, so its shape and color are set once at startup rather than per mode.
- The cursor is drawn after the selection, so it covers the selection's last cell (test `charwise_selection_over_two_lines` updated).

### Left undone / flagged

- None. Manually verified in ghostty by the developer.
