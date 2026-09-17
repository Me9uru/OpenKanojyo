use schemars::JsonSchema;

pub(crate) fn parameters<T: JsonSchema>() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(T)).expect("JSON schema 应可序列化")
}
