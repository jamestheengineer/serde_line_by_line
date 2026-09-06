#[serde(tag = "type", rename_all = "snake_case")]
enum Shape {
    Circle { r: f64 },
    Square { side: f64 },
}
