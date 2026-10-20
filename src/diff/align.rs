//! Builds whole-file aligned rows from difft output.

use std::ops::Range;

use tracing::warn;
use unicode_width::UnicodeWidthChar;

use super::difft::{Change, RawDiff, SideEntry, Status};
use super::model::{FileDiff, LineKind, Row, RowRange, SideText};
use crate::git::FileContent;

/// Sides come from `content`, rows and kinds from `raw`. A mismatch between the two (file
/// edited between reads) is clamped and logged, never a panic.
pub fn build(raw: &RawDiff, content: &FileContent, tab_width: usize) -> FileDiff {
    let mut lhs = content.old.as_deref().map(|t| Side::new(t, tab_width));
    let mut rhs = content.new.as_deref().map(|t| Side::new(t, tab_width));
    let (nl, nr) = (Side::len(&lhs), Side::len(&rhs));
    let index = |n: usize| move |i: u32| ((i as usize) < n).then_some(i);
    let rows: Vec<Row> = match raw.status {
        Status::Changed => raw
            .aligned_lines
            .iter()
            .map(|&[l, r]| Row {
                lhs: l.and_then(index(nl)),
                rhs: r.and_then(index(nr)),
            })
            .filter(|row| row.lhs.is_some() || row.rhs.is_some())
            .collect(),
        Status::Created => (0..nr as u32)
            .map(|i| Row {
                lhs: None,
                rhs: Some(i),
            })
            .collect(),
        Status::Deleted => (0..nl as u32)
            .map(|i| Row {
                lhs: Some(i),
                rhs: None,
            })
            .collect(),
        Status::Unchanged => (0..nl.max(nr) as u32)
            .map(|i| Row {
                lhs: index(nl)(i),
                rhs: index(nr)(i),
            })
            .collect(),
    };
    if rows.iter().filter(|r| r.lhs.is_some()).count() != nl
        || rows.iter().filter(|r| r.rhs.is_some()).count() != nr
    {
        warn!("diff rows don't cover every line once; file changed while diffing?");
    }

    match raw.status {
        Status::Created => Side::fill(&mut rhs, LineKind::Added),
        Status::Deleted => Side::fill(&mut lhs, LineKind::Removed),
        Status::Changed => {
            let (mut paired_l, mut paired_r) = (vec![false; nl], vec![false; nr]);
            for row in &rows {
                if let (Some(l), Some(r)) = (row.lhs, row.rhs) {
                    paired_l[l as usize] = true;
                    paired_r[r as usize] = true;
                }
            }
            for entry in raw.chunks.iter().flatten() {
                Side::mark(&mut lhs, entry.lhs.as_ref(), &paired_l, LineKind::Removed);
                Side::mark(&mut rhs, entry.rhs.as_ref(), &paired_r, LineKind::Added);
            }
        }
        Status::Unchanged => {}
    }

    let (lhs, rhs) = (lhs.map(|s| s.text), rhs.map(|s| s.text));
    let hunks = hunks(&rows, lhs.as_ref(), rhs.as_ref());
    FileDiff {
        lhs,
        rhs,
        rows,
        hunks,
    }
}

/// A side being built: the normalized text plus what's needed to convert difft offsets.
struct Side<'a> {
    /// CR-stripped, before tab expansion; what difft's offsets index into.
    raw: Vec<&'a str>,
    /// Raw byte → expanded byte, for lines containing tabs.
    maps: Vec<Option<Vec<usize>>>,
    text: SideText,
}

impl<'a> Side<'a> {
    fn new(text: &'a str, tab_width: usize) -> Self {
        let raw: Vec<&str> = text
            .lines()
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect();
        let (lines, maps) = raw.iter().map(|l| expand_tabs(l, tab_width)).unzip();
        let n = raw.len();
        Side {
            raw,
            maps,
            text: SideText {
                lines,
                kinds: vec![LineKind::Unchanged; n],
                changes: vec![Vec::new(); n],
                highlights: vec![Vec::new(); n],
            },
        }
    }

    fn len(side: &Option<Self>) -> usize {
        side.as_ref().map_or(0, |s| s.raw.len())
    }

    fn fill(side: &mut Option<Self>, kind: LineKind) {
        if let Some(side) = side {
            side.text.kinds.fill(kind);
        }
    }

    /// Marks a chunk line `Changed` if its row has an opposite side, else `unpaired`.
    fn mark(
        side: &mut Option<Self>,
        entry: Option<&SideEntry>,
        paired: &[bool],
        unpaired: LineKind,
    ) {
        let (Some(side), Some(entry)) = (side.as_mut(), entry) else {
            return;
        };
        let line = entry.line_number as usize;
        let Some(&is_paired) = paired.get(line) else {
            warn!(line, "difft chunk line out of range");
            return;
        };
        side.text.kinds[line] = if is_paired {
            LineKind::Changed
        } else {
            unpaired
        };
        side.text.changes[line] = entry
            .changes
            .iter()
            .filter_map(|c| map_span(side.raw[line], side.maps[line].as_deref(), c))
            .collect();
    }
}

/// Expands tabs to the next multiple of `tab_width` display columns. The map is `None` when
/// the line has no tabs (offsets unchanged).
pub fn expand_tabs(line: &str, tab_width: usize) -> (String, Option<Vec<usize>>) {
    if !line.contains('\t') {
        return (line.to_owned(), None);
    }
    let tab_width = tab_width.max(1);
    let mut out = String::with_capacity(line.len());
    // Only char-boundary entries are meaningful; spans are snapped to boundaries first.
    let mut map = vec![0; line.len() + 1];
    let mut col = 0;
    for (i, c) in line.char_indices() {
        map[i] = out.len();
        if c == '\t' {
            let n = tab_width - col % tab_width;
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(c);
            col += c.width().unwrap_or(0);
        }
    }
    map[line.len()] = out.len();
    (out, Some(map))
}

/// Raw difft span → expanded-line byte range. Clamped to the line and snapped to char
/// boundaries (lossy UTF-8 decoding shifts offsets); empty spans are dropped.
fn map_span(raw: &str, map: Option<&[usize]>, change: &Change) -> Option<Range<usize>> {
    if change.end > raw.len() {
        warn!(?change.start, ?change.end, len = raw.len(), "difft span past end of line");
    }
    let start = raw.floor_char_boundary(change.start.min(raw.len()));
    let end = raw.ceil_char_boundary(change.end.min(raw.len()));
    let at = |i: usize| map.map_or(i, |m| m[i]);
    (start < end).then(|| at(start)..at(end))
}

/// Maximal runs of rows with a non-`Unchanged` line on either side.
fn hunks(rows: &[Row], lhs: Option<&SideText>, rhs: Option<&SideText>) -> Vec<RowRange> {
    let changed = |side: Option<&SideText>, line: Option<u32>| matches!((side, line), (Some(s), Some(l)) if s.kinds[l as usize] != LineKind::Unchanged);
    let mut hunks = Vec::new();
    let mut start = None;
    for (i, row) in rows.iter().enumerate() {
        match (changed(lhs, row.lhs) || changed(rhs, row.rhs), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                hunks.push(s..i);
                start = None;
            }
            _ => {}
        }
    }
    hunks.extend(start.map(|s| s..rows.len()));
    hunks
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init, reason = "expected span lists")]
mod tests {
    use super::*;
    use LineKind::*;

    macro_rules! fixture {
        ($case:literal, $ext:literal, $tab_width:expr) => {
            build(
                &serde_json::from_str(include_str!(concat!(
                    "../../tests/fixtures/difft/",
                    $case,
                    "/difft.json"
                )))
                .unwrap(),
                &FileContent {
                    old: Some(
                        include_str!(concat!("../../tests/fixtures/difft/", $case, "/old.", $ext))
                            .into(),
                    ),
                    new: Some(
                        include_str!(concat!("../../tests/fixtures/difft/", $case, "/new.", $ext))
                            .into(),
                    ),
                },
                $tab_width,
            )
        };
    }

    fn row(lhs: Option<u32>, rhs: Option<u32>) -> Row {
        Row { lhs, rhs }
    }

    fn inline(json: &str, old: Option<&str>, new: Option<&str>) -> FileDiff {
        let content = FileContent {
            old: old.map(Into::into),
            new: new.map(Into::into),
        };
        build(&serde_json::from_str(json).unwrap(), &content, 4)
    }

    #[test]
    fn mod_fixture() {
        let diff = fixture!("mod", "rs", 4);
        // The phantom trailing pair is dropped.
        assert_eq!(diff.rows.len(), 20);
        assert!(diff.rows.contains(&row(None, Some(1))));
        assert!(diff.rows.contains(&row(Some(17), None)));
        let (lhs, rhs) = (diff.lhs.unwrap(), diff.rhs.unwrap());
        assert_eq!(lhs.kinds[2], Changed);
        assert!(lhs.changes[2].is_empty());
        assert_eq!(rhs.kinds[3], Changed);
        assert_eq!(diff.hunks, [1..2, 3..4, 5..6, 17..19]);
    }

    #[test]
    fn reformat_fixture() {
        let diff = fixture!("reformat", "rs", 4);
        let rhs = diff.rhs.unwrap();
        for i in 1..=6 {
            assert_eq!(diff.rows[i], row(None, Some(i as u32)));
            assert_eq!(rhs.kinds[i], Unchanged);
        }
        assert_eq!(diff.hunks, [9..10]);
    }

    #[test]
    fn uni_fixture() {
        let lhs = fixture!("uni", "py", 4).lhs.unwrap();
        assert!(
            lhs.changes[1]
                .iter()
                .any(|r| &lhs.lines[1][r.clone()] == "αβγ")
        );
    }

    #[test]
    fn tab_fixture() {
        let lhs = fixture!("tab", "rs", 4).lhs.unwrap();
        assert_eq!(lhs.changes[1], [12..13]);
        assert_eq!(&lhs.lines[1][12..13], "1");
    }

    #[test]
    fn created_and_deleted_are_one_sided() {
        let created = inline(
            r#"{"path":"a","language":"Text","status":"created"}"#,
            None,
            Some("a\nb\n"),
        );
        assert!(created.lhs.is_none());
        assert_eq!(created.rows, [row(None, Some(0)), row(None, Some(1))]);
        assert_eq!(created.rhs.unwrap().kinds, [Added, Added]);

        let deleted = inline(
            r#"{"path":"a","language":"Text","status":"deleted"}"#,
            Some("a\nb"),
            None,
        );
        assert!(deleted.rhs.is_none());
        assert_eq!(deleted.rows, [row(Some(0), None), row(Some(1), None)]);
        assert_eq!(deleted.lhs.unwrap().kinds, [Removed, Removed]);
    }

    #[test]
    fn zero_width_span_dropped() {
        let diff = inline(
            r#"{"status":"changed","aligned_lines":[[0,0],[1,1]],"chunks":[[
                {"lhs":{"line_number":0,"changes":[{"start":1,"end":1}]},
                 "rhs":{"line_number":0,"changes":[{"start":0,"end":1}]}}]]}"#,
            Some("ab\n"),
            Some("cb\n"),
        );
        assert!(diff.lhs.unwrap().changes[0].is_empty());
        assert_eq!(diff.rhs.unwrap().changes[0], [0..1]);
    }
}
