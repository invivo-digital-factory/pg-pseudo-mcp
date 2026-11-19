const MICROS_PER_SECOND: i64 = 1_000_000;
const MICROS_PER_MINUTE: i64 = 60 * MICROS_PER_SECOND;
const MICROS_PER_HOUR: i64 = 60 * MICROS_PER_MINUTE;
const MICROS_PER_DAY: i64 = 24 * MICROS_PER_HOUR;
const UNIX_DAYS_AT_POSTGRES_EPOCH: i64 = 10_957;

pub(crate) fn date_to_string(days: i32) -> String {
    match days {
        i32::MAX => "infinity".into(),
        i32::MIN => "-infinity".into(),
        _ => civil_date(i64::from(days)),
    }
}

pub(crate) fn time_to_string(micros: i64) -> String {
    clock(micros)
}

pub(crate) fn timetz_to_string(micros: i64, zone_seconds_west: i32) -> String {
    let east = -i64::from(zone_seconds_west);
    let sign = if east < 0 { '-' } else { '+' };
    let east = east.abs();
    let hours = east / 3600;
    let minutes = (east % 3600) / 60;
    let seconds = east % 60;
    let mut out = format!("{}{sign}{hours:02}:{minutes:02}", clock(micros));
    if seconds != 0 {
        out.push_str(&format!(":{seconds:02}"));
    }
    out
}

pub(crate) fn timestamp_to_string(micros: i64, utc: bool) -> String {
    match micros {
        i64::MAX => "infinity".into(),
        i64::MIN => "-infinity".into(),
        _ => {
            let days = micros.div_euclid(MICROS_PER_DAY);
            let within_day = micros.rem_euclid(MICROS_PER_DAY);
            let suffix = if utc { "Z" } else { "" };
            format!("{}T{}{suffix}", civil_date(days), clock(within_day))
        }
    }
}

pub(crate) fn interval_to_string(micros: i64, days: i32, months: i32) -> String {
    if micros == 0 && days == 0 && months == 0 {
        return "PT0S".into();
    }
    let mut out = String::from("P");
    let years = months / 12;
    let months = months % 12;
    if years != 0 {
        out.push_str(&format!("{years}Y"));
    }
    if months != 0 {
        out.push_str(&format!("{months}M"));
    }
    if days != 0 {
        out.push_str(&format!("{days}D"));
    }
    if micros != 0 {
        out.push('T');
        let hours = micros / MICROS_PER_HOUR;
        let minutes = (micros % MICROS_PER_HOUR) / MICROS_PER_MINUTE;
        let seconds = micros % MICROS_PER_MINUTE;
        if hours != 0 {
            out.push_str(&format!("{hours}H"));
        }
        if minutes != 0 {
            out.push_str(&format!("{minutes}M"));
        }
        if seconds != 0 {
            let sign = if seconds < 0 { "-" } else { "" };
            let seconds = seconds.abs();
            out.push_str(&format!(
                "{sign}{}{}S",
                seconds / MICROS_PER_SECOND,
                fraction(seconds % MICROS_PER_SECOND)
            ));
        }
    }
    out
}

fn clock(micros: i64) -> String {
    let hours = micros / MICROS_PER_HOUR;
    let minutes = (micros % MICROS_PER_HOUR) / MICROS_PER_MINUTE;
    let seconds = (micros % MICROS_PER_MINUTE) / MICROS_PER_SECOND;
    format!(
        "{hours:02}:{minutes:02}:{seconds:02}{}",
        fraction(micros % MICROS_PER_SECOND)
    )
}

fn fraction(micros: i64) -> String {
    if micros == 0 {
        return String::new();
    }
    let digits = format!("{micros:06}");
    format!(".{}", digits.trim_end_matches('0'))
}

fn civil_date(days_since_postgres_epoch: i64) -> String {
    let z = days_since_postgres_epoch + UNIX_DAYS_AT_POSTGRES_EPOCH + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{}-{month:02}-{day:02}", year_label(year))
}

fn year_label(year: i64) -> String {
    if (0..=9999).contains(&year) {
        format!("{year:04}")
    } else if year < 0 {
        format!("-{:04}", -year)
    } else {
        format!("+{year}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_count_from_the_postgres_epoch() {
        assert_eq!(date_to_string(0), "2000-01-01");
        assert_eq!(date_to_string(-1), "1999-12-31");
        assert_eq!(date_to_string(60), "2000-03-01");
        assert_eq!(date_to_string(i32::MAX), "infinity");
    }

    #[test]
    fn timestamps_render_iso_8601_with_trimmed_fractions() {
        assert_eq!(timestamp_to_string(0, true), "2000-01-01T00:00:00Z");
        assert_eq!(timestamp_to_string(-1, false), "1999-12-31T23:59:59.999999");
        assert_eq!(
            timestamp_to_string(1_500_000, false),
            "2000-01-01T00:00:01.5"
        );
        assert_eq!(timestamp_to_string(i64::MIN, true), "-infinity");
    }

    #[test]
    fn times_with_zone_render_the_eastward_offset() {
        assert_eq!(timetz_to_string(MICROS_PER_HOUR, -7200), "01:00:00+02:00");
        assert_eq!(timetz_to_string(0, 19_800), "00:00:00-05:30");
    }

    #[test]
    fn intervals_follow_the_postgres_iso_8601_style() {
        assert_eq!(interval_to_string(0, 0, 0), "PT0S");
        assert_eq!(interval_to_string(0, 0, 14), "P1Y2M");
        assert_eq!(interval_to_string(-1_500_000, 0, 0), "PT-1.5S");
        assert_eq!(
            interval_to_string(-(MICROS_PER_HOUR + MICROS_PER_MINUTE), -2, -13),
            "P-1Y-1M-2DT-1H-1M"
        );
    }
}
