//! Example: `compile_fail` — the errors, with the compiler's own words.
//!
//! Some lessons are only teachable as a rejection. "Two `&mut` to the same
//! value cannot coexist" is a claim; the diagnostic that says so, with the
//! first borrow and the conflicting use both underlined, is the lesson.
//!
//! Each case lives in `tests/ui/` as a complete program that must **fail** to
//! compile. `trybuild` builds every one of them under `cargo test`, asserts the
//! failure, and diffs the compiler's output against the committed `.stderr`. A
//! case that quietly starts compiling — because the borrow checker got smarter,
//! or because the case was subtly wrong — breaks the build instead of becoming
//! a paragraph that is no longer true.
//!
//! `run()` reads back exactly those two committed files, so the browser shows
//! diagnostics that were produced by rustc rather than typed by hand. This is
//! the one example whose output is not computed live: it cannot be, since there
//! is no compiler in the page. The `cargo test` run is what keeps it honest.
//!
//! After a rustc upgrade the wording of a diagnostic may change and the test
//! will fail on the diff. That is working as intended; regenerate with:
//!
//! ```text
//! TRYBUILD=overwrite cargo test -p compile_fail
//! ```
//!
//! and read the new output before committing it — a changed message is
//! sometimes a changed rule.

/// One case: the file stem, the point it makes, and its two committed files.
struct Case {
    name: &'static str,
    lesson: &'static str,
    source: &'static str,
    stderr: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "use_after_move",
        lesson: "Using a value after it moved. One allocation, one owner.",
        source: include_str!("../tests/ui/use_after_move.rs"),
        stderr: include_str!("../tests/ui/use_after_move.stderr"),
    },
    Case {
        name: "two_mutable_borrows",
        lesson: "Two `&mut` to one value. Exclusive means exclusive.",
        source: include_str!("../tests/ui/two_mutable_borrows.rs"),
        stderr: include_str!("../tests/ui/two_mutable_borrows.stderr"),
    },
    Case {
        name: "mutate_while_borrowed",
        lesson: "Growing a String while a &str into it is still alive.",
        source: include_str!("../tests/ui/mutate_while_borrowed.rs"),
        stderr: include_str!("../tests/ui/mutate_while_borrowed.stderr"),
    },
    Case {
        name: "move_out_of_ref",
        lesson: "Taking a field out from behind a shared reference.",
        source: include_str!("../tests/ui/move_out_of_ref.rs"),
        stderr: include_str!("../tests/ui/move_out_of_ref.stderr"),
    },
];

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();
    out.push_str(
        "Four programs that must not compile, and what rustc says about them.\n\
         The diagnostics below are committed output, checked against the real\n\
         compiler by trybuild on every `cargo test`.\n",
    );

    for case in CASES {
        out.push('\n');
        out.push_str(&"=".repeat(72));
        out.push('\n');
        out.push_str(&format!("{}\n{}\n\n", case.name, case.lesson));
        // Printed whole, comments included: the diagnostic quotes the file by
        // line number, and a trimmed listing would not line up with it.
        out.push_str(case.source.trim_end());
        out.push_str("\n\n");
        out.push_str(case.stderr.trim_end());
        out.push('\n');
    }

    out.push_str(
        "\nThe pattern across all four: the compiler is not tracking your style,\n\
         it is tracking whether a pointer can outlive what it points at, or two\n\
         writers can reach one place. Every fix is a decision about which of\n\
         those you meant — borrow, clone, or restructure so the borrow ends\n\
         sooner.\n",
    );

    out
}
