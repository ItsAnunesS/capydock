//! Language-neutral messages for the desktop view. Raw parameters (especially
//! filenames) are kept separate from text; only explicit nested diagnostics are
//! translated. Persisted activity can be rendered in any supported language.
use serde_json::{json, Value};

pub const MESSAGE_PREFIX: &str = "\u{001e}proton-i18n:";

pub fn message(key: &str, values: &[Value]) -> String {
    format!(
        "{MESSAGE_PREFIX}{}",
        json!({ "key": key, "values": values })
    )
}

pub fn nested(value: impl Into<String>) -> Value {
    json!({ "message": value.into() })
}

pub fn raw(value: impl Into<String>) -> String {
    message("{0}", &[json!(value.into())])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_preserve_unicode_paths_and_nested_errors_as_distinct_values() {
        let filename = "Documentos/{0}/férias \"2026\".pdf";
        let encoded = message("{0}: {1}", &[json!(filename), nested("Caminho inválido.")]);
        let value: Value =
            serde_json::from_str(encoded.strip_prefix(MESSAGE_PREFIX).unwrap()).unwrap();
        assert_eq!(value["values"][0], filename);
        assert_eq!(value["values"][1]["message"], "Caminho inválido.");
    }
}
