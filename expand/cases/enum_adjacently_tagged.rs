#[serde(tag = "t", content = "c")]
enum Event {
    Tick,
    Moved(i32, i32),
}
