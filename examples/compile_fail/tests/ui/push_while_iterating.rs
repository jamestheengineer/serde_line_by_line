// Unit 14 — an iterator is a borrow, and it lasts as long as the loop.
//
// `iter()` hands out a `slice::Iter` holding a pointer into the vector's
// buffer. `push` may reallocate that buffer, which would leave the iterator
// pointing at freed memory — the same failure `mutate_while_borrowed` shows
// for `String`, now arriving through a `for` loop where the borrow is not
// written down anywhere.
//
// The loop is also the reason "just clone it" is the wrong instinct here: the
// question this program cannot answer is whether the pushed element should be
// visited by the loop that pushed it.

fn main() {
    let mut lengths = vec![1_usize, 2, 3];

    for len in lengths.iter() {
        if *len == 2 {
            lengths.push(99);
        }
    }

    println!("{lengths:?}");
}
