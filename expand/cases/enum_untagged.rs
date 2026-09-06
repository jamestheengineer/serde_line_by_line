#[serde(untagged)]
enum Value {
    Number(u64),
    Text(String),
}
