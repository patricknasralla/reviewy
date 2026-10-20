//! Yank registers.

/// `text` has no trailing line break, also when `linewise`.
#[derive(Debug, Default, Clone)]
pub struct Register {
    pub text: String,
    pub linewise: bool,
}

/// Only the unnamed register for now; named registers and the clipboard come later.
#[derive(Debug, Default)]
pub struct Registers {
    pub unnamed: Register,
}
