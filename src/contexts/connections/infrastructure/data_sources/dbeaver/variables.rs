use std::sync::OnceLock;

use regex::Regex;

fn pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\$\{([\w.\-]+)(:[^${}]+)?\}").expect("variable pattern is a valid regex")
    })
}

pub fn resolve<F, E>(input: &str, own_field: F, env: E) -> Result<String, String>
where
    F: Fn(&str) -> Option<String>,
    E: Fn(&str) -> Option<String>,
{
    let mut output = String::with_capacity(input.len());
    let mut last = 0;
    for captures in pattern().captures_iter(input) {
        let whole = captures.get(0).expect("group 0 always matches");
        let name = &captures[1];
        let value = own_field(name)
            .filter(|value| !value.is_empty())
            .or_else(|| env(name))
            .or_else(|| {
                captures
                    .get(2)
                    .map(|default| default.as_str()[1..].to_string())
            });
        let value = value.ok_or_else(|| format!("unresolved variable ${{{name}}}"))?;
        output.push_str(&input[last..whole.start()]);
        output.push_str(&value);
        last = whole.end();
    }
    output.push_str(&input[last..]);
    Ok(output)
}

pub fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

#[cfg(test)]
mod unit {
    use super::*;

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn un_texte_sans_variable_reste_inchange() {
        assert_eq!(resolve("localhost", none, none).unwrap(), "localhost");
    }

    #[test]
    fn une_variable_prend_ses_champs_puis_l_environnement_puis_son_defaut() {
        let own = |name: &str| (name == "host").then(|| "own-host".to_string());
        let env = |name: &str| (name == "region").then(|| "eu".to_string());
        let resolved = resolve("${host}-${region}-${tier:gold}", own, env).unwrap();
        assert_eq!(resolved, "own-host-eu-gold");
    }

    #[test]
    fn une_variable_non_resolue_sans_defaut_est_nommee_dans_l_erreur() {
        let error = resolve("db-${MISSING}", none, none).unwrap_err();
        assert_eq!(error, "unresolved variable ${MISSING}");
    }

    #[test]
    fn le_defaut_sert_quand_rien_d_autre_ne_correspond() {
        assert_eq!(resolve("${a:b}", none, none).unwrap(), "b");
    }
}
