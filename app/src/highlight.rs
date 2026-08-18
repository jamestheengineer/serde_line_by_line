//! Build-time syntax highlighting (decision D2).
//!
//! Class-based rather than inline styles, so one rendering serves both light
//! and dark themes.
//!
//! The subtlety is line addressing. Annotations are keyed to line ranges, so
//! every line must be an independently renderable element — but Rust has
//! constructs that span lines (2.27% of serde_core's lines sit inside a
//! multi-line block comment or raw string). Highlighting each line in isolation
//! gets those wrong; highlighting the whole file at once produces spans that
//! cross line boundaries and cannot be split.
//!
//! So: parse the file once with carried state, then re-balance each line by
//! reopening the enclosing tag stack. Correct context, self-contained lines.

use anyhow::{Context, Result};
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, line_tokens_to_classed_spans, ClassStyle};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

const CLASS_STYLE: ClassStyle = ClassStyle::Spaced;

/// One source line, highlighted and self-contained.
#[derive(Debug, Clone)]
pub struct Line {
    pub number: u32,
    pub html: String,
}

pub struct Highlighter {
    syntaxes: SyntaxSet,
    themes: ThemeSet,
}

impl Highlighter {
    pub fn new() -> Self {
        Highlighter {
            syntaxes: SyntaxSet::load_defaults_newlines(),
            themes: ThemeSet::load_defaults(),
        }
    }

    fn rust(&self) -> Result<&SyntaxReference> {
        self.syntaxes
            .find_syntax_by_extension("rs")
            .context("no Rust syntax definition")
    }

    /// Highlights an entire file, returning one self-contained fragment per
    /// line, 1-indexed. Callers slice this by annotation range.
    pub fn file(&self, text: &str) -> Result<Vec<Line>> {
        let syntax = self.rust()?;
        let mut parse = ParseState::new(syntax);
        let mut stack = ScopeStack::new();
        let mut balancer = Balancer::default();
        let mut out = Vec::new();

        for (i, raw) in LinesWithEndings::from(text).enumerate() {
            let ops = parse
                .parse_line(raw, &self.syntaxes)
                .with_context(|| format!("parsing line {}", i + 1))?;
            let (html, _delta) =
                line_tokens_to_classed_spans(raw, ops.as_slice(), CLASS_STYLE, &mut stack)
                    .with_context(|| format!("highlighting line {}", i + 1))?;
            out.push(Line {
                number: i as u32 + 1,
                html: balancer.balance(html.trim_end_matches('\n')),
            });
        }
        Ok(out)
    }

    /// Theme CSS for the generated classes. Emitted once per site build.
    pub fn theme_css(&self, name: &str) -> Result<String> {
        let theme = self
            .themes
            .themes
            .get(name)
            .with_context(|| format!("no theme named {name}"))?;
        css_for_theme_with_class_style(theme, CLASS_STYLE).context("generating theme css")
    }
}

/// Makes each line stand alone.
///
/// `line_tokens_to_classed_spans` carries scope state across lines, so a line
/// in the middle of a block comment arrives with its opening `<span>` on some
/// earlier line. This tracks the literal open tags and reopens them at the
/// start of each line, closing them again at the end.
///
/// It works on the tag text itself rather than on syntect's scope types, so it
/// needs no knowledge of how classes are named.
#[derive(Default)]
struct Balancer {
    open: Vec<String>,
}

impl Balancer {
    fn balance(&mut self, raw: &str) -> String {
        let prefix: String = self.open.concat();
        self.track(raw);
        let suffix = "</span>".repeat(self.open.len());
        format!("{prefix}{raw}{suffix}")
    }

    fn track(&mut self, html: &str) {
        let bytes = html.as_bytes();
        let mut i = 0;
        while let Some(found) = html[i..].find('<') {
            let at = i + found;
            if html[at..].starts_with("</span>") {
                self.open.pop();
                i = at + "</span>".len();
            } else if html[at..].starts_with("<span") {
                match bytes[at..].iter().position(|&b| b == b'>') {
                    Some(end) => {
                        self.open.push(html[at..at + end + 1].to_string());
                        i = at + end + 1;
                    }
                    None => break,
                }
            } else {
                i = at + 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_line_block_comment_stays_highlighted() {
        let hl = Highlighter::new();
        let src = "fn a() {}\n/* comment\n   still comment\n   end */\nfn b() {}\n";
        let lines = hl.file(src).unwrap();
        assert_eq!(lines.len(), 5);
        // The interior line carries no `/*` of its own, so isolated
        // highlighting would treat it as code. With carried state it must be
        // marked as comment.
        assert!(
            lines[2].html.contains("comment"),
            "line 3 lost its comment scope: {}",
            lines[2].html
        );
    }

    #[test]
    fn every_line_is_balanced() {
        let hl = Highlighter::new();
        let src = "/* a\n b\n c */\nlet s = \"x\";\n";
        for line in hl.file(src).unwrap() {
            let opens = line.html.matches("<span").count();
            let closes = line.html.matches("</span>").count();
            assert_eq!(
                opens, closes,
                "unbalanced on line {}: {}",
                line.number, line.html
            );
        }
    }

    #[test]
    fn line_numbers_are_one_indexed_and_dense() {
        let hl = Highlighter::new();
        let lines = hl.file("a\nb\nc\n").unwrap();
        let nums: Vec<u32> = lines.iter().map(|l| l.number).collect();
        assert_eq!(nums, vec![1, 2, 3]);
    }
}
