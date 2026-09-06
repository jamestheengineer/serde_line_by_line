#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Point {
    x_coord: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}
