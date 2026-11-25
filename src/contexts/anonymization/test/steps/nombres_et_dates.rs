use cucumber::{then, when};

use crate::anonymization_steps::support::{read_column, rendered_cell, typed_cell};
use crate::anonymization_world::AnonymizationWorld;

const MIN_SHIFT_WEEKS: i64 = 1;

const MAX_SHIFT_WEEKS: i64 = 52;

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn day_number(text: &str) -> i64 {
    let number = |range: std::ops::Range<usize>| -> i64 {
        text[range]
            .parse()
            .unwrap_or_else(|_| panic!("{text} is not a date"))
    };
    if text.as_bytes()[4] == b'-' {
        days_from_civil(number(0..4), number(5..7), number(8..10))
    } else {
        days_from_civil(number(6..10), number(3..5), number(0..2))
    }
}

#[when(regex = r#"^l'agent lit "([^"]*)" dans la colonne calculée "([^"]+)" de type "([^"]+)"$"#)]
async fn agent_reads_typed_derived_column(
    world: &mut AnonymizationWorld,
    value: String,
    column: String,
    type_name: String,
) {
    let (cell, clear) = typed_cell(&type_name, &value);
    read_column(world, &column, false, &type_name, cell, clear);
}

#[then(
    regex = r"^la valeur rendue est la même date décalée d'un nombre entier de semaines, de 1 à 52$"
)]
async fn date_is_shifted_by_whole_weeks(world: &mut AnonymizationWorld) {
    let shifted = world.last_pseudonym();
    let delta = day_number(shifted) - day_number(world.input());
    assert_eq!(delta % 7, 0, "{shifted}");
    assert!(
        (MIN_SHIFT_WEEKS..=MAX_SHIFT_WEEKS).contains(&(delta.abs() / 7)),
        "{shifted}"
    );
}

#[then(regex = r"^la valeur rendue est une date ISO entre (\d+) et (\d+)$")]
async fn date_is_plausible(world: &mut AnonymizationWorld, from: i64, to: i64) {
    let date = world.last_pseudonym();
    assert_eq!(
        crate::anonymization_world::layout(date),
        "xxxx-xx-xx",
        "{date}"
    );
    let year: i64 = date[..4].parse().expect("a year");
    assert!((from..=to).contains(&year), "{date}");
}

#[then(regex = r"^la valeur rendue est un nombre$")]
async fn rendered_value_is_a_number(world: &mut AnonymizationWorld) {
    let rendered = rendered_cell(world);
    assert!(rendered.is_number(), "{rendered}");
}
