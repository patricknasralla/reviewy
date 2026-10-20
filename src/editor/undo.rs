//! Undo/redo history of editor states.

use std::collections::VecDeque;

use super::buffer::Buffer;
use crate::vim::motion::Cursor;

/// Editor state before a change.
pub struct Snapshot {
    pub buffer: Buffer,
    pub cursor: Cursor,
}

pub struct Undo {
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    /// Most undo steps kept; 0 disables undo.
    levels: usize,
}

impl Undo {
    pub fn new(levels: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            levels,
        }
    }

    /// Records a new step, dropping the oldest past the limit; clears redo.
    pub fn record(&mut self, before: Snapshot) {
        self.redo.clear();
        self.push(before);
    }

    /// The state to restore; `current` becomes redoable.
    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let prev = self.undo.pop_back()?;
        self.redo.push(current);
        Some(prev)
    }

    /// The state to restore; `current` becomes undoable.
    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.redo.pop()?;
        self.push(current);
        Some(next)
    }

    fn push(&mut self, snapshot: Snapshot) {
        if self.levels == 0 {
            return;
        }
        if self.undo.len() == self.levels {
            self.undo.pop_front();
        }
        self.undo.push_back(snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::Undo;
    use crate::editor::tests::run;

    #[test]
    fn change_and_insertion_undo_in_one_step() {
        run("foo bar", 20, (0, 0))
            .keys("cwxyz\x1b")
            .is("xyz bar", (0, 2))
            .keys("u")
            .is("foo bar", (0, 0));
    }

    #[test]
    fn oldest_step_dropped_at_limit() {
        let mut r = run("abcd", 20, (0, 0));
        r.e.undo = Undo::new(2);
        r.keys("xxx").is("d", (0, 0)).keys("uuu").is("bcd", (0, 0));
    }

    #[test]
    fn new_change_clears_redo() {
        run("abc", 20, (0, 0))
            .keys("xu")
            .is("abc", (0, 0))
            .keys("x\x12")
            .is("bc", (0, 0));
    }

    #[test]
    fn no_op_changes_record_nothing() {
        // `x` on the empty line and `i` + Esc change nothing, so `u` undoes the first `x`.
        run("ab\n", 20, (0, 0)).keys("xjxi\x1bu").is("ab\n", (0, 0));
    }
}
