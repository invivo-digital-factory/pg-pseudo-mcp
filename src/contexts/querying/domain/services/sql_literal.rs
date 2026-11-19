pub fn literal(text: &str) -> String {
    let escaped = text
        .replace('\0', "")
        .replace('\\', "\\\\")
        .replace('\'', "''");
    format!("E'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_cannot_escape_their_quotes() {
        assert_eq!(literal("it's"), "E'it''s'");
        assert_eq!(literal("a\\'b"), "E'a\\\\''b'");
        assert_eq!(literal("x\0y"), "E'xy'");
    }
}
