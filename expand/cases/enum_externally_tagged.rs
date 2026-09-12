enum Shape {
    Empty,
    Radius(f64),
    Rect(f64, f64),
    Named { label: String },
}
