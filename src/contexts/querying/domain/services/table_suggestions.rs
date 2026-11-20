use crate::contexts::querying::domain::constants::catalog::MAX_SUGGESTIONS;

pub fn closest_names(name: &str, known: &[(String, String)]) -> Vec<String> {
    let wanted = name.to_lowercase();
    let wanted_len = wanted.chars().count();
    let tolerance = (wanted_len / 3).max(2);
    let mut scored: Vec<(usize, usize, String)> = known
        .iter()
        .filter_map(|(schema, table)| {
            let candidate = table.to_lowercase();
            let length_gap = candidate.chars().count().abs_diff(wanted_len);
            let score = if !wanted.is_empty()
                && (candidate.contains(&wanted) || wanted.contains(&candidate))
            {
                0
            } else {
                let distance = edit_distance(&wanted, &candidate);
                if distance > tolerance {
                    return None;
                }
                distance
            };
            Some((score, length_gap, format!("{schema}.{table}")))
        })
        .collect();
    scored.sort();
    scored
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, _, name)| name)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1; right.len() + 1];
        for (column, right_char) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left_char != *right_char);
            current[column + 1] = substitution
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        previous = current;
    }
    previous[right.len()]
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn les_suggestions_privilegient_les_sous_chaines_puis_les_petites_fautes() {
        let known = vec![
            ("public".to_string(), "customers".to_string()),
            ("public".to_string(), "orders".to_string()),
            ("public".to_string(), "order_lines".to_string()),
            ("public".to_string(), "notes".to_string()),
        ];
        assert_eq!(
            closest_names("order", &known),
            vec!["public.orders", "public.order_lines"]
        );
        assert_eq!(closest_names("custmers", &known), vec!["public.customers"]);
        assert!(closest_names("zzz", &known).is_empty());
    }
}
