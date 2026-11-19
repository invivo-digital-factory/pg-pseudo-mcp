#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub oid: i64,
    pub schema: String,
    pub name: String,
    pub path_rank: Option<i64>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Pick {
    One(Candidate),
    Ambiguous(Vec<String>),
    None,
}

pub fn pick(candidates: Vec<Candidate>, schema: Option<&str>, name: &str) -> Pick {
    let exact: Vec<Candidate> = candidates
        .iter()
        .filter(|candidate| {
            candidate.name == name && schema.is_none_or(|schema| candidate.schema == schema)
        })
        .cloned()
        .collect();
    let mut pool = if exact.is_empty() { candidates } else { exact };
    pool.sort_by(|left, right| {
        (
            left.path_rank.is_none(),
            left.path_rank,
            &left.schema,
            &left.name,
        )
            .cmp(&(
                right.path_rank.is_none(),
                right.path_rank,
                &right.schema,
                &right.name,
            ))
    });
    match pool.as_slice() {
        [] => Pick::None,
        [only] => Pick::One(only.clone()),
        [first, second, ..] if first.path_rank.is_some() && first.path_rank != second.path_rank => {
            Pick::One(first.clone())
        }
        _ => Pick::Ambiguous(pool.iter().map(qualified).collect()),
    }
}

pub fn qualified(candidate: &Candidate) -> String {
    format!("{}.{}", candidate.schema, candidate.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(schema: &str, name: &str, path_rank: Option<i64>) -> Candidate {
        Candidate {
            oid: 1,
            schema: schema.into(),
            name: name.into(),
            path_rank,
        }
    }

    #[test]
    fn search_path_order_resolves_bare_names() {
        let picked = pick(
            vec![
                candidate("sales", "orders", Some(2)),
                candidate("public", "orders", Some(1)),
            ],
            None,
            "orders",
        );
        assert_eq!(picked, Pick::One(candidate("public", "orders", Some(1))));
    }

    #[test]
    fn names_outside_the_search_path_are_ambiguous() {
        let picked = pick(
            vec![
                candidate("sales", "orders", None),
                candidate("archive", "orders", None),
            ],
            None,
            "orders",
        );
        assert_eq!(
            picked,
            Pick::Ambiguous(vec!["archive.orders".into(), "sales.orders".into()])
        );
    }

    #[test]
    fn exact_case_wins_over_case_insensitive_matches() {
        let picked = pick(
            vec![
                candidate("public", "Orders", Some(1)),
                candidate("public", "orders", Some(1)),
            ],
            None,
            "Orders",
        );
        assert_eq!(picked, Pick::One(candidate("public", "Orders", Some(1))));
    }
}
