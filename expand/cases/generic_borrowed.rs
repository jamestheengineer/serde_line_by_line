struct Wrapper<'a, T> {
    #[serde(borrow)]
    name: &'a str,
    inner: T,
}
