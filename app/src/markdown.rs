//! Renders annotation bodies from Markdown.
//!
//! Annotation prose is authored in Markdown inside TOML, so it stays readable
//! in the source files and renders as HTML in the site.

use pulldown_cmark::{html, Options, Parser};

pub fn render(source: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(source, options);
    let mut out = String::with_capacity(source.len() * 3 / 2);
    html::push_html(&mut out, parser);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn renders_inline_code_and_emphasis() {
        let out = super::render("`type Ok` is **chosen** by the format.");
        assert!(out.contains("<code>type Ok</code>"), "{out}");
        assert!(out.contains("<strong>chosen</strong>"), "{out}");
    }

    #[test]
    fn renders_lists() {
        let out = super::render("- one\n- two\n");
        assert!(
            out.contains("<ul>") && out.contains("<li>one</li>"),
            "{out}"
        );
    }
}
