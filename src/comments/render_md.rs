//! comments.md rendering with preamble.

use std::fmt::Write;

use super::model::{Anchor, Comment, CommentSet, Side, fence};

pub fn render(set: &CommentSet, preamble: &str) -> String {
    let short = set.base.commit.get(..8).unwrap_or(&set.base.commit);
    let preamble = preamble.trim();
    let ref_name = &set.base.ref_name;
    let mut out =
        format!("# Review comments\n\n{preamble}\n\nBase commit: `{short}` ({ref_name})\n");
    if let Some(general) = set.general() {
        let body = general.body.trim_end();
        write!(out, "\n## General\n\n{body}\n").unwrap();
    }
    let mut comments: Vec<_> = (set.comments.iter())
        .filter_map(|c| Some((c, c.anchor.as_ref()?)))
        .collect();
    comments.sort_by_key(|(c, a)| (&a.file, *a.selection.line_range().start(), c.id));
    let mut file = None;
    for (c, anchor) in comments {
        if file != Some(&anchor.file) {
            file = Some(&anchor.file);
            let path = anchor.file.display();
            write!(out, "\n## {path}\n").unwrap();
        }
        render_comment(&mut out, c, anchor);
    }
    out
}

fn render_comment(out: &mut String, c: &Comment, anchor: &Anchor) {
    let side = match anchor.side {
        Side::New => "",
        Side::Old => " (old: base commit version)",
    };
    let (id, selection) = (c.id, &anchor.selection);
    write!(out, "\n### #{id} - {selection}{side}\n\n").unwrap();

    let ex = &c.excerpt;
    let selected = anchor.selection.line_range();
    let fence = fence(&ex.lines.join("\n"));
    let width = (ex.start as usize + ex.lines.len())
        .saturating_sub(1)
        .to_string()
        .len();
    let language = c.language.as_deref().unwrap_or("");
    writeln!(out, "{fence}{language}").unwrap();
    for (n, text) in (ex.start..).zip(&ex.lines) {
        let mark = if selected.contains(&n) { '>' } else { ' ' };
        writeln!(out, "{mark} {n:>width$} | {text}").unwrap();
    }
    let body = c.body.trim_end();
    writeln!(out, "{fence}\n\n{body}").unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comments::model::tests::sample_set;

    #[test]
    fn render_two_files() {
        insta::assert_snapshot!(render(&sample_set(), "Preamble text."));
    }

    #[test]
    fn general_comes_first_without_excerpt() {
        let mut set = sample_set();
        let id = set.add_general();
        set.get_mut(id).unwrap().body = "Overall.\n".into();
        let out = render(&set, "Preamble text.");
        assert!(out.contains("(HEAD)\n\n## General\n\nOverall.\n\n## src/bar.py\n"));
    }
}
