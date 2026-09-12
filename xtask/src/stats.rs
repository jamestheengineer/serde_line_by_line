//! Structural inventory of the pinned source.
//!
//! The effort model in PLAN.md came from this analysis. Keeping it as a command
//! means the numbers are reproducible rather than a one-off measurement.

use anyhow::Result;
use slbl_core::vendor;
use std::path::Path;

struct FileStats {
    file: String,
    lines: u32,
    doc: u32,
    code: u32,
    items: u32,
    macro_defs: u32,
    macro_uses: u32,
    /// `quote!` / `quote_spanned!` invocations. The opposite of a macro use as
    /// a compression lever: each one emits different code and needs its own
    /// explanation next to what it emits, which is why `serde_derive` costs
    /// more annotations than `serde_core` despite being 25% smaller
    /// (PLAN.md §12).
    quote_uses: u32,
}

impl FileStats {
    /// Macro-driven files compress heavily: a `macro_rules!` body is explained
    /// once and its invocations cost a reference each, not a paragraph.
    fn estimated_units(&self) -> u32 {
        if self.macro_uses > 40 {
            self.macro_defs * 3 + self.macro_uses / 4 + self.items
        } else if self.quote_uses > 10 {
            // Codegen density, measured against what this project achieved and
            // not against the macro-driven files: one annotation per ~14 lines.
            self.items.max(self.lines / 14)
        } else {
            self.items.max(self.lines / 18)
        }
    }

    fn profile(&self) -> &'static str {
        if self.macro_uses > 40 {
            "macro-driven"
        } else if self.quote_uses > 10 {
            "quote-driven"
        } else if self.lines > 0 && self.doc * 10 / self.lines >= 4 {
            "doc-heavy"
        } else {
            "logic"
        }
    }
}

pub fn run(repo: &Path) -> Result<()> {
    for source in vendor::load_pin(repo)?.coverage()? {
        println!("\n== {} ==", source.source_id());
        run_source(&source.dir(repo))?;
    }
    Ok(())
}

fn run_source(root: &Path) -> Result<()> {
    let mut stats: Vec<FileStats> = Vec::new();

    for rel in vendor::source_files_in(root)? {
        let text = std::fs::read_to_string(root.join(&rel))?;
        let mut s = FileStats {
            file: rel,
            lines: 0,
            doc: 0,
            code: 0,
            items: 0,
            macro_defs: 0,
            macro_uses: 0,
            quote_uses: 0,
        };
        for line in text.lines() {
            s.lines += 1;
            let t = line.trim_start();
            if t.starts_with("///") || t.starts_with("//!") {
                s.doc += 1;
            } else if t.is_empty() || t.starts_with("//") {
                // blank or ordinary comment
            } else {
                s.code += 1;
            }
            if t.starts_with("macro_rules!") {
                s.macro_defs += 1;
            } else if is_macro_use(t) {
                s.macro_uses += 1;
            }
            s.quote_uses +=
                t.matches("quote!").count() as u32 + t.matches("quote_spanned!").count() as u32;
            if is_item(t) {
                s.items += 1;
            }
        }
        stats.push(s);
    }

    stats.sort_by_key(|s| std::cmp::Reverse(s.lines));

    println!(
        "\n{:<26}{:>7}{:>6}{:>6}{:>7}{:>6}{:>6}{:>7}{:>7}  profile",
        "file", "lines", "doc", "code", "items", "mdef", "muse", "quote", "units"
    );
    let (mut tl, mut td, mut tu) = (0, 0, 0);
    for s in &stats {
        println!(
            "{:<26}{:>7}{:>6}{:>6}{:>7}{:>6}{:>6}{:>7}{:>7}  {}",
            s.file,
            s.lines,
            s.doc,
            s.code,
            s.items,
            s.macro_defs,
            s.macro_uses,
            s.quote_uses,
            s.estimated_units(),
            s.profile()
        );
        tl += s.lines;
        td += s.doc;
        tu += s.estimated_units();
    }
    println!("\n{:<26}{:>7}{:>6}{:>26}{:>14}", "TOTAL", tl, td, "", tu);
    println!(
        "\n{} files, {tl} lines, {td} doc ({:.0}%), ~{tu} estimated annotation units, \
         ~{:.1} lines/unit\n",
        stats.len(),
        td as f64 * 100.0 / tl as f64,
        tl as f64 / tu as f64,
    );
    Ok(())
}

fn is_macro_use(t: &str) -> bool {
    let Some(bang) = t.find('!') else {
        return false;
    };
    if bang == 0 {
        return false;
    }
    let name = &t[..bang];
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    matches!(
        t[bang + 1..].trim_start().chars().next(),
        Some('{') | Some('(') | Some('[')
    )
}

fn is_item(t: &str) -> bool {
    let t = t
        .strip_prefix("pub(crate) ")
        .or_else(|| t.strip_prefix("pub "))
        .unwrap_or(t);
    let t = t.strip_prefix("unsafe ").unwrap_or(t);
    [
        "fn ", "struct ", "enum ", "trait ", "impl ", "const ", "type ", "mod ",
    ]
    .iter()
    .any(|k| t.starts_with(k))
        || t.starts_with("macro_rules!")
}
