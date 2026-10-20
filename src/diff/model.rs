//! FileDiff, Row, LineKind and change spans.

use std::ops::Range;

use crate::highlight::highlighter::StyledSpan;
use crate::vim::motion::TextView;

/// Whole-file, row-aligned diff of one file.
#[derive(Debug)]
pub struct FileDiff {
    /// `None` for added/untracked files.
    pub lhs: Option<SideText>,
    /// `None` for deleted files.
    pub rhs: Option<SideText>,
    pub rows: Vec<Row>,
    pub hunks: Vec<RowRange>,
}

/// One side's text, with per-line data indexed like `lines`.
#[derive(Debug)]
pub struct SideText {
    /// CR-stripped and tab-expanded.
    pub lines: Vec<String>,
    pub kinds: Vec<LineKind>,
    /// Changed-token byte ranges into `lines[i]`.
    pub changes: Vec<Vec<Range<usize>>>,
    pub highlights: Vec<Vec<StyledSpan>>,
}

impl TextView for SideText {
    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn line(&self, idx: usize) -> &str {
        &self.lines[idx]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Unchanged,
    Changed,
    Added,
    Removed,
}

/// 0-based line indices; `None` is padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub lhs: Option<u32>,
    pub rhs: Option<u32>,
}

/// Half-open range of row indices.
pub type RowRange = Range<usize>;
