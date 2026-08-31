// Unit 14 — the difference between `Fn` and `FnMut`, as a rejection.
//
// `map_three` promises to call its argument through `&self`, so the closure
// it is given may only read what it captured. This one adds to `calls`, which
// needs `&mut self` on the call — that is `FnMut`, one step weaker than what
// the bound asks for.
//
// The fix is almost always to relax the bound rather than to change the
// closure: `F: FnMut(u32) -> u32` and `mut f`, as `closure_kinds`'s `feed`
// does. `Fn` is only worth demanding when the caller needs to call through a
// shared reference — from several places at once, or from several threads.

fn map_three<F: Fn(u32) -> u32>(f: F) -> Vec<u32> {
    (1..=3).map(f).collect()
}

fn main() {
    let mut calls = 0;

    let counted = |n: u32| {
        calls += 1;
        n + calls
    };

    println!("{:?} in {calls} calls", map_three(counted));
}
