use std::time::{SystemTime, UNIX_EPOCH};

const JST_OFFSET_SECONDS: i64 = 9 * 3_600;

pub fn now_string() -> String {
    format_timestamp(unix_seconds() + JST_OFFSET_SECONDS)
}

pub fn utc_now_string() -> String {
    format_timestamp(unix_seconds())
}

pub fn today_string() -> String {
    let (year, month, day) =
        civil_from_days((unix_seconds() + JST_OFFSET_SECONDS).div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn format_timestamp(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day % 3_600 / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };

    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::{JST_OFFSET_SECONDS, format_timestamp};

    #[test]
    fn jst_timestamp_is_nine_hours_ahead_of_utc() {
        let utc = 1_718_452_800; // 2024-06-15 12:00:00 UTC

        assert_eq!(format_timestamp(utc), "2024-06-15 12:00:00");
        assert_eq!(
            format_timestamp(utc + JST_OFFSET_SECONDS),
            "2024-06-15 21:00:00"
        );
    }

    #[test]
    fn jst_timestamp_rolls_over_to_the_next_day() {
        let utc = 1_718_492_400; // 2024-06-15 23:00:00 UTC

        assert_eq!(
            format_timestamp(utc + JST_OFFSET_SECONDS),
            "2024-06-16 08:00:00"
        );
    }
}
