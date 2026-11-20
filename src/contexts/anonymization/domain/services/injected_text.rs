pub fn replace_injected(
    haystack: &str,
    needle: &str,
    replacement: &str,
    embedded: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let pattern = regex::RegexBuilder::new(&regex::escape(needle))
        .case_insensitive(true)
        .build()
        .ok()?;
    let mut output = String::with_capacity(haystack.len());
    let mut cursor = 0;
    let mut search = 0;
    let mut replaced = false;
    while let Some(found) = pattern.find_at(haystack, search) {
        let before = haystack[..found.start()].chars().next_back();
        let after = haystack[found.end()..].chars().next();
        if before.is_some_and(is_word_char) || after.is_some_and(is_word_char) {
            let start = cursor.max(word_start(haystack, found.start()));
            let end = word_end(haystack, found.end());
            if let Some(token) = embedded(&haystack[start..end]) {
                output.push_str(&haystack[cursor..start]);
                output.push_str(&token);
                cursor = end;
                search = end;
                replaced = true;
            } else {
                search = found.start()
                    + haystack[found.start()..]
                        .chars()
                        .next()
                        .map_or(1, char::len_utf8);
            }
            continue;
        }
        output.push_str(&haystack[cursor..found.start()]);
        output.push_str(replacement);
        cursor = found.end();
        search = cursor;
        replaced = true;
    }
    if !replaced {
        return None;
    }
    output.push_str(&haystack[cursor..]);
    Some(output)
}

fn word_start(text: &str, index: usize) -> usize {
    text[..index]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word_char(*c))
        .last()
        .map_or(index, |(position, _)| position)
}

fn word_end(text: &str, index: usize) -> usize {
    text[index..]
        .char_indices()
        .find(|(_, c)| !is_word_char(*c))
        .map_or(text.len(), |(position, _)| index + position)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '\u{0300}'..='\u{036F}')
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn la_valeur_injectee_est_remplacee_dans_toutes_ses_variantes_de_casse() {
        assert_eq!(
            replace_injected("A b@C.fr and B@c.FR", "b@c.fr", "x@y.org", |_| None).as_deref(),
            Some("A x@y.org and x@y.org")
        );
        assert_eq!(
            replace_injected("İstanbul MAËLOUZ", "maëlouz", "x", |_| None).as_deref(),
            Some("İstanbul x")
        );
        assert_eq!(replace_injected("nothing", "b@c.fr", "x", |_| None), None);
    }

    #[test]
    fn une_valeur_injectee_n_est_remplacee_que_comme_mot_entier() {
        assert_eq!(replace_injected("Troyes", "Roy", "Zed", |_| None), None);
        assert_eq!(
            replace_injected("leroy.marc@gmail.com", "Roy", "Zed", |_| None),
            None
        );
        assert_eq!(replace_injected("Roy2", "Roy", "Zed", |_| None), None);
        assert_eq!(replace_injected("Royé", "Roy", "Zed", |_| None), None);
        assert_eq!(
            replace_injected("xa-a-a", "a-a", "Z", |_| None).as_deref(),
            Some("xa-Z")
        );
        assert_eq!(
            replace_injected("Roy, ROY et roy-Dupont", "Roy", "Zed", |_| None).as_deref(),
            Some("Zed, Zed et Zed-Dupont")
        );
    }
}
