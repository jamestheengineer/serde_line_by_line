//! Example: `compile_fail` — the errors, with the compiler's own words.
//!
//! Some lessons are only teachable as a rejection. "Two `&mut` to the same
//! value cannot coexist" is a claim; the diagnostic that says so, with the
//! first borrow and the conflicting use both underlined, is the lesson.
//!
//! This is the harness for every unit that has one, so each case names the unit
//! it belongs to: units 06, 07 and 14 so far — ownership errors, lifetime
//! errors, and the two ways an iterator or a closure is refused.
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

/// One case: the unit it belongs to, the file stem, the point it makes, and
/// its two committed files.
struct Case {
    unit: &'static str,
    name: &'static str,
    lesson: &'static str,
    source: &'static str,
    stderr: &'static str,
}

const CASES: &[Case] = &[
    Case {
        unit: "06",
        name: "use_after_move",
        lesson: "Using a value after it moved. One allocation, one owner.",
        source: include_str!("../tests/ui/use_after_move.rs"),
        stderr: include_str!("../tests/ui/use_after_move.stderr"),
    },
    Case {
        unit: "06",
        name: "two_mutable_borrows",
        lesson: "Two `&mut` to one value. Exclusive means exclusive.",
        source: include_str!("../tests/ui/two_mutable_borrows.rs"),
        stderr: include_str!("../tests/ui/two_mutable_borrows.stderr"),
    },
    Case {
        unit: "06",
        name: "mutate_while_borrowed",
        lesson: "Growing a String while a &str into it is still alive.",
        source: include_str!("../tests/ui/mutate_while_borrowed.rs"),
        stderr: include_str!("../tests/ui/mutate_while_borrowed.stderr"),
    },
    Case {
        unit: "06",
        name: "move_out_of_ref",
        lesson: "Taking a field out from behind a shared reference.",
        source: include_str!("../tests/ui/move_out_of_ref.rs"),
        stderr: include_str!("../tests/ui/move_out_of_ref.stderr"),
    },
    Case {
        unit: "07",
        name: "missing_lifetime",
        lesson: "Two input references, one output. Elision refuses to guess.",
        source: include_str!("../tests/ui/missing_lifetime.rs"),
        stderr: include_str!("../tests/ui/missing_lifetime.stderr"),
    },
    Case {
        unit: "07",
        name: "returns_local",
        lesson: "Returning a reference to a value the function is about to free.",
        source: include_str!("../tests/ui/returns_local.rs"),
        stderr: include_str!("../tests/ui/returns_local.stderr"),
    },
    Case {
        unit: "07",
        name: "struct_outlives_input",
        lesson: "A borrowing struct outliving the buffer it is a view into.",
        source: include_str!("../tests/ui/struct_outlives_input.rs"),
        stderr: include_str!("../tests/ui/struct_outlives_input.stderr"),
    },
    Case {
        unit: "07",
        name: "not_static",
        lesson: "`&'static str` asked of an ordinary runtime String.",
        source: include_str!("../tests/ui/not_static.rs"),
        stderr: include_str!("../tests/ui/not_static.stderr"),
    },
    Case {
        unit: "14",
        name: "for_loop_moves",
        lesson: "`for x in v` consumes v, because that is the impl it picked.",
        source: include_str!("../tests/ui/for_loop_moves.rs"),
        stderr: include_str!("../tests/ui/for_loop_moves.stderr"),
    },
    Case {
        unit: "14",
        name: "push_while_iterating",
        lesson: "Pushing to a Vec while a loop over it is still running.",
        source: include_str!("../tests/ui/push_while_iterating.rs"),
        stderr: include_str!("../tests/ui/push_while_iterating.stderr"),
    },
    Case {
        unit: "14",
        name: "closure_types_differ",
        lesson: "Two closures, identical text, one Vec. Two types.",
        source: include_str!("../tests/ui/closure_types_differ.rs"),
        stderr: include_str!("../tests/ui/closure_types_differ.stderr"),
    },
    Case {
        unit: "14",
        name: "closure_mutates_under_fn",
        lesson: "A closure that counts, offered where `Fn` was demanded.",
        source: include_str!("../tests/ui/closure_mutates_under_fn.rs"),
        stderr: include_str!("../tests/ui/closure_mutates_under_fn.stderr"),
    },
];

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();
    out.push_str(
        "Twelve programs that must not compile, and what rustc says about them.\n\
         Four belong to unit 06, four to unit 07 and four to unit 14; each is\n\
         labelled with its unit. The diagnostics below are committed output,\n\
         checked against the real compiler by trybuild on every `cargo test`.\n",
    );

    for case in CASES {
        out.push('\n');
        out.push_str(&"=".repeat(72));
        out.push('\n');
        out.push_str(&format!(
            "unit {} · {}\n{}\n\n",
            case.unit, case.name, case.lesson
        ));
        // Printed whole, comments included: the diagnostic quotes the file by
        // line number, and a trimmed listing would not line up with it.
        out.push_str(case.source.trim_end());
        out.push_str("\n\n");
        out.push_str(case.stderr.trim_end());
        out.push('\n');
    }

    out.push_str(
        "\nThe pattern across all twelve: the compiler is not tracking your style,\n\
         it is tracking whether a pointer can outlive what it points at, or two\n\
         writers can reach one place. The unit 06 cases are that question asked\n\
         about a value — who owns it, who may look. The unit 07 cases are the\n\
         same question asked about a *signature*, where the compiler has only\n\
         what you wrote to reason from: two of them are fixed by writing a\n\
         lifetime parameter, and two by admitting the data really does die\n\
         first, so a reference to it cannot be what you return.\n\n\
         The unit 14 cases are the same two questions arriving through syntax\n\
         that hides them. `for x in v` is a move because `IntoIterator` takes\n\
         `self`; an iterator is a live borrow for as long as the loop runs; and\n\
         the last two are about closures being ordinary values with ordinary\n\
         types — one type each, and one of `Fn`, `FnMut` or `FnOnce` depending\n\
         on what the body does to what it captured.\n",
    );

    out
}
