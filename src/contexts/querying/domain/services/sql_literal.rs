pub fn literal(text: &str) -> String {
    let escaped = text
        .replace('\0', "")
        .replace('\\', "\\\\")
        .replace('\'', "''");
    format!("E'{escaped}'")
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn un_litteral_ne_peut_pas_s_echapper_de_ses_guillemets() {
        assert_eq!(literal("it's"), "E'it''s'");
        assert_eq!(literal("a\\'b"), "E'a\\\\''b'");
        assert_eq!(literal("x\0y"), "E'xy'");
    }
}
