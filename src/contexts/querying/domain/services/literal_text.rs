use sqlparser::ast::Value;

pub fn literal_text(value: &Value) -> Option<String> {
    match value {
        Value::SingleQuotedString(text)
        | Value::EscapedStringLiteral(text)
        | Value::UnicodeStringLiteral(text)
        | Value::NationalStringLiteral(text)
        | Value::TripleSingleQuotedString(text)
        | Value::TripleDoubleQuotedString(text) => Some(text.clone()),
        Value::DollarQuotedString(dollar) => Some(dollar.value.clone()),
        Value::Number(digits, _) => Some(digits.to_string()),
        _ => None,
    }
}

pub fn like_core(text: &str) -> Option<(&str, &str, &str)> {
    let start = text.len() - text.trim_start_matches('%').len();
    let end = text.trim_end_matches('%').len();
    if (start == 0 && end == text.len()) || start >= end {
        return None;
    }
    Some((&text[..start], &text[start..end], &text[end..]))
}

pub fn pattern_parts(text: &str) -> (&str, &str, &str) {
    like_core(text).unwrap_or(("", text, ""))
}

pub fn candidate(text: &str) -> &str {
    like_core(text).map_or(text, |(_, core, _)| core)
}

pub fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}
