//! tree-sitter-highlight to per-line style spans.

use std::ops::Range;

/// Placeholder; task 04 adds the style.
#[allow(dead_code, reason = "used by task 04; remove then")]
#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub range: Range<usize>,
}
