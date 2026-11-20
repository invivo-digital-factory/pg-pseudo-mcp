pub fn split_qualified(input: &str) -> (Option<String>, String) {
    let mut parts = vec![String::new()];
    let mut quoted = false;
    let mut chars = input.trim().chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                if let Some(part) = parts.last_mut() {
                    part.push('"');
                }
            }
            '"' => quoted = !quoted,
            '.' if !quoted => parts.push(String::new()),
            other => {
                if let Some(part) = parts.last_mut() {
                    part.push(other);
                }
            }
        }
    }
    let name = parts.pop().unwrap_or_default();
    let schema = parts.pop().filter(|schema| !schema.is_empty());
    (schema, name)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn un_nom_qualifie_se_decoupe_sur_les_points_hors_guillemets() {
        assert_eq!(
            split_qualified("public.orders"),
            (Some("public".into()), "orders".into())
        );
        assert_eq!(split_qualified("orders"), (None, "orders".into()));
        assert_eq!(
            split_qualified("\"My.Schema\".\"Odd\"\"Name\""),
            (Some("My.Schema".into()), "Odd\"Name".into())
        );
        assert_eq!(
            split_qualified("db.sales.orders"),
            (Some("sales".into()), "orders".into())
        );
    }
}
