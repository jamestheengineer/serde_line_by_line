// Unit 14 — two closures written identically are two different types.
//
// Each closure expression gets its own anonymous type, generated where it is
// written, exactly as two structs with identical fields are still two types.
// Identical text does not make them the same one, so they cannot share a
// `Vec` — and the diagnostic says "distinct closures" rather than naming
// them, because neither type has a name to print.
//
// Both closures here capture `bump`, and that is load-bearing: closures that
// capture nothing coerce to a plain `fn(u32) -> u32`, so the same `vec![]`
// with `|n: u32| n + 1` twice compiles and infers `Vec<fn(u32) -> u32>`. The
// coercion needs a closure with nothing to carry.
//
// The fix is in `closure_kinds` section 3: `Vec<Box<dyn Fn(u32) -> u32>>`
// erases the difference, at the cost of an allocation and an indirect call.

fn main() {
    let bump = 1;

    let add_one = move |n: u32| n + bump;
    let also_add_one = move |n: u32| n + bump;

    let table = vec![add_one, also_add_one];

    println!("{}", table.len());
}
