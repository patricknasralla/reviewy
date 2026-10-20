//! Comment, Anchor, Selection, Excerpt and CommentSet.

use std::fmt;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// `comments.json` schema version.
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentSet {
    pub version: u32,
    pub base: Base,
    /// Ids only increase, never reused.
    pub next_id: u32,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Base {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub commit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Old,
    New,
}

/// 1-based; `col` counts chars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

/// What a comment covers. Build with `from_lines`/`from_chars`, which order the ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Selection {
    Line {
        line: u32,
    },
    Lines {
        start: u32,
        end: u32,
    },
    /// Inclusive at both ends.
    Chars {
        start: Pos,
        end: Pos,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub file: PathBuf,
    pub side: Side,
    pub selection: Selection,
}

/// Contiguous lines from `start`: the selected lines plus context on either side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Excerpt {
    pub start: u32,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: u32,
    pub anchor: Anchor,
    pub language: Option<String>,
    pub excerpt: Excerpt,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentDraft {
    pub anchor: Anchor,
    pub language: Option<String>,
    pub excerpt: Excerpt,
    pub body: String,
}

impl Selection {
    pub fn from_lines(a: u32, b: u32) -> Self {
        if a == b {
            Self::Line { line: a }
        } else {
            Self::Lines {
                start: a.min(b),
                end: a.max(b),
            }
        }
    }

    pub fn from_chars(a: Pos, b: Pos) -> Self {
        Self::Chars {
            start: a.min(b),
            end: a.max(b),
        }
    }

    pub fn line_range(&self) -> RangeInclusive<u32> {
        match *self {
            Self::Line { line } => line..=line,
            Self::Lines { start, end } => start..=end,
            Self::Chars { start, end } => start.line..=end.line,
        }
    }
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Line { line } => write!(f, "line {line}"),
            Self::Lines { start, end } => write!(f, "lines {start}-{end}"),
            Self::Chars { start, end } => {
                let (l1, c1, l2, c2) = (start.line, start.col, end.line, end.col);
                write!(f, "line {l1} col {c1} - line {l2} col {c2}")
            }
        }
    }
}

impl Excerpt {
    /// Text of 1-based `line`, or "" outside the excerpt.
    pub fn line(&self, line: u32) -> &str {
        line.checked_sub(self.start)
            .and_then(|i| self.lines.get(i as usize))
            .map_or("", String::as_str)
    }
}

impl Comment {
    /// The selected lines joined with `\n`; for `Chars`, only the selected chars.
    pub fn selected_text(&self) -> String {
        let sel = &self.anchor.selection;
        let lines = sel.line_range().map(|n| {
            let text = self.excerpt.line(n);
            let (from, to) = match *sel {
                Selection::Chars { start, end } => (
                    if n == start.line { start.col - 1 } else { 0 },
                    if n == end.line { end.col } else { u32::MAX },
                ),
                _ => (0, u32::MAX),
            };
            text.chars()
                .skip(from as usize)
                .take(to.saturating_sub(from) as usize)
                .collect::<String>()
        });
        lines.collect::<Vec<_>>().join("\n")
    }
}

impl CommentSet {
    pub fn new(base: Base) -> Self {
        Self {
            version: VERSION,
            base,
            next_id: 1,
            comments: Vec::new(),
        }
    }

    pub fn add(&mut self, draft: CommentDraft) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.comments.push(Comment {
            id,
            anchor: draft.anchor,
            language: draft.language,
            excerpt: draft.excerpt,
            body: draft.body,
        });
        id
    }

    pub fn get(&self, id: u32) -> Option<&Comment> {
        self.comments.iter().find(|c| c.id == id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Comment> {
        self.comments.iter_mut().find(|c| c.id == id)
    }

    pub fn remove(&mut self, id: u32) -> Option<Comment> {
        let i = self.comments.iter().position(|c| c.id == id)?;
        Some(self.comments.remove(i))
    }

    /// First comment covering 1-based `line`.
    pub fn at(&self, file: &Path, side: Side, line: u32) -> Option<&Comment> {
        self.comments.iter().find(|c| {
            c.anchor.file == file
                && c.anchor.side == side
                && c.anchor.selection.line_range().contains(&line)
        })
    }

    /// Id of a comment on the same file and side whose lines intersect `anchor`'s.
    pub fn overlaps(&self, anchor: &Anchor) -> Option<u32> {
        let range = anchor.selection.line_range();
        self.comments
            .iter()
            .find(|c| {
                let other = c.anchor.selection.line_range();
                c.anchor.file == anchor.file
                    && c.anchor.side == anchor.side
                    && range.start() <= other.end()
                    && other.start() <= range.end()
            })
            .map(|c| c.id)
    }

    /// Comments on a file: old-side ones on `old`, new-side ones on `new`.
    pub fn count_for(&self, old: &Path, new: &Path) -> usize {
        self.comments
            .iter()
            .filter(|c| {
                c.anchor.file
                    == match c.anchor.side {
                        Side::Old => old,
                        Side::New => new,
                    }
            })
            .count()
    }
}

/// `side_lines` are the whole side's lines; the excerpt adds up to `context_lines` around the selection.
pub fn draft(
    anchor: Anchor,
    language: Option<&str>,
    side_lines: &[String],
    context_lines: usize,
    body: String,
) -> CommentDraft {
    let range = anchor.selection.line_range();
    let first = (*range.start() as usize)
        .saturating_sub(context_lines)
        .max(1);
    let last = (*range.end() as usize + context_lines).min(side_lines.len());
    let lines = side_lines
        .iter()
        .skip(first - 1)
        .take((last + 1).saturating_sub(first))
        .cloned()
        .collect();
    CommentDraft {
        anchor,
        language: language.map(str::to_owned),
        excerpt: Excerpt {
            start: first as u32,
            lines,
        },
        body,
    }
}

/// A backtick fence longer than any backtick run in `text` (at least 3).
pub fn fence(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat((longest + 1).max(3))
}

/// `text` in a fenced block, ending with an empty line for the cursor.
pub fn prefill_body(text: &str, language: Option<&str>) -> String {
    let fence = fence(text);
    format!("{fence}{}\n{text}\n{fence}\n", language.unwrap_or(""))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    fn lines(n: u32) -> Vec<String> {
        (1..=n).map(|i| format!("line {i} text")).collect()
    }

    fn anchor(file: &str, side: Side, selection: Selection) -> Anchor {
        Anchor {
            file: file.into(),
            side,
            selection,
        }
    }

    /// One new-side charwise comment in `src/foo.rs`, one old-side line comment in `src/bar.rs`.
    pub(in crate::comments) fn sample_set() -> CommentSet {
        let mut set = CommentSet::new(Base {
            ref_name: "HEAD".into(),
            commit: "3f2c9e1a77b0c4d5e6f708192a3b4c5d6e7f8091".into(),
        });
        let foo = lines(12);
        let chars = anchor(
            "src/foo.rs",
            Side::New,
            Selection::from_chars(Pos { line: 9, col: 3 }, Pos { line: 8, col: 6 }),
        );
        set.add(draft(
            chars,
            Some("rust"),
            &foo,
            3,
            "```rust\n8 text\nlin\n```\nWhy?".into(),
        ));
        set.add(draft(
            anchor("src/foo.rs", Side::New, Selection::from_lines(2, 2)),
            Some("rust"),
            &foo,
            3,
            "Rename this.".into(),
        ));
        set.add(draft(
            anchor("src/bar.py", Side::Old, Selection::from_lines(5, 4)),
            Some("python"),
            &lines(5),
            1,
            "Was this removed on purpose?".into(),
        ));
        set
    }

    #[test]
    fn json_round_trip() {
        let set = sample_set();
        let json = serde_json::to_string_pretty(&set).unwrap();
        assert_eq!(serde_json::from_str::<CommentSet>(&json).unwrap(), set);
        insta::assert_snapshot!(json);
    }

    #[test]
    fn overlaps_same_file_and_side_only() {
        let mut set = CommentSet::new(Base {
            ref_name: "HEAD".into(),
            commit: "abc".into(),
        });
        let id = set.add(draft(
            anchor("a.rs", Side::New, Selection::from_lines(5, 8)),
            None,
            &lines(20),
            3,
            String::new(),
        ));
        let chars = Selection::from_chars(Pos { line: 8, col: 1 }, Pos { line: 9, col: 1 });
        assert_eq!(set.overlaps(&anchor("a.rs", Side::New, chars)), Some(id));
        let other_side = anchor("a.rs", Side::Old, Selection::from_lines(5, 8));
        assert_eq!(set.overlaps(&other_side), None);
        let adjacent = anchor("a.rs", Side::New, Selection::from_lines(9, 10));
        assert_eq!(set.overlaps(&adjacent), None);
    }

    #[test]
    fn fence_outgrows_backtick_runs() {
        assert_eq!(fence("a ``` b"), "````");
        assert_eq!(fence("`x`"), "```");
    }

    #[test]
    fn multiline_chars_selected_text() {
        let set = sample_set();
        // Selected backwards from 9:3 to 8:6; start col > end col.
        assert_eq!(set.get(1).unwrap().selected_text(), "8 text\nlin");
    }
}
