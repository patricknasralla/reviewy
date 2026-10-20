//! comments.md rendering with preamble.

use std::fmt::Write;

use super::model::{Comment, CommentSet, Side, fence};

pub fn render(set: &CommentSet, preamble: &str) -> String {
    let short = set.base.commit.get(..8).unwrap_or(&set.base.commit);
    let preamble = preamble.trim();
    let ref_name = &set.base.ref_name;
    let mut out =
        format!("# Review comments\n\n{preamble}\n\nBase commit: `{short}` ({ref_name})\n");
    let mut comments: Vec<_> = set.comments.iter().collect();
    comments.sort_by_key(|c| {
        (
            &c.anchor.file,
            *c.anchor.selection.line_range().start(),
            c.id,
        )
    });
    let mut file = None;
    for c in comments {
        if file != Some(&c.anchor.file) {
            file = Some(&c.anchor.file);
            let path = c.anchor.file.display();
            write!(out, "\n## {path}\n").unwrap();
        }
        render_comment(&mut out, c);
    }
    out
}

fn render_comment(out: &mut String, c: &Comment) {
    let side = match c.anchor.side {
        Side::New => "",
        Side::Old => " (old: base commit version)",
    };
    let (id, selection) = (c.id, &c.anchor.selection);
    write!(out, "\n### #{id} - {selection}{side}\n\n").unwrap();

    let ex = &c.excerpt;
    let selected = c.anchor.selection.line_range();
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
}
