use crate::datetime::{now_string, today_string};
use crate::error::{Result, SqlRockError};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum DateFunctionArg {
    Value(String),
    Interval { value: String, unit: String },
}

#[derive(Debug, Clone, Copy)]
struct DateTime {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
}

pub fn evaluate_date_function(name: &str, args: &[DateFunctionArg]) -> Result<String> {
    let name = name.to_ascii_uppercase();
    match name.as_str() {
        "NOW" | "CURRENT_TIMESTAMP" | "LOCALTIME" | "LOCALTIMESTAMP" | "SYSDATE" => {
            require_arity(args, 0, 1)?;
            Ok(now_string())
        }
        "CURDATE" | "CURRENT_DATE" => {
            require_arity(args, 0, 0)?;
            Ok(today_string())
        }
        "CURTIME" | "CURRENT_TIME" => {
            require_arity(args, 0, 1)?;
            Ok(now_string()[11..].to_string())
        }
        "UTC_TIMESTAMP" => {
            require_arity(args, 0, 1)?;
            Ok(now_string())
        }
        "UTC_DATE" => {
            require_arity(args, 0, 0)?;
            Ok(now_string()[..10].to_string())
        }
        "UTC_TIME" => {
            require_arity(args, 0, 1)?;
            Ok(now_string()[11..].to_string())
        }
        "DATE" => unary_datetime(args, |dt| format_date(dt)),
        "TIME" => unary_datetime(args, |dt| format_time(dt)),
        "YEAR" => unary_datetime(args, |dt| dt.year.to_string()),
        "MONTH" => unary_datetime(args, |dt| dt.month.to_string()),
        "DAY" | "DAYOFMONTH" => unary_datetime(args, |dt| dt.day.to_string()),
        "HOUR" => unary_datetime(args, |dt| dt.hour.to_string()),
        "MINUTE" => unary_datetime(args, |dt| dt.minute.to_string()),
        "SECOND" => unary_datetime(args, |dt| dt.second.to_string()),
        "MICROSECOND" => {
            require_arity(args, 1, 1)?;
            Ok(fractional_microseconds(value(args, 0)?).to_string())
        }
        "QUARTER" => unary_datetime(args, |dt| ((dt.month - 1) / 3 + 1).to_string()),
        "DAYOFWEEK" => unary_datetime(args, |dt| (weekday_sunday_zero(dt) + 1).to_string()),
        "WEEKDAY" => unary_datetime(args, |dt| ((weekday_sunday_zero(dt) + 6) % 7).to_string()),
        "DAYOFYEAR" => unary_datetime(args, |dt| day_of_year(dt).to_string()),
        "DAYNAME" => unary_datetime(args, |dt| {
            [
                "Sunday",
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
            ][weekday_sunday_zero(dt) as usize]
                .to_string()
        }),
        "MONTHNAME" => unary_datetime(args, |dt| {
            [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ][dt.month.saturating_sub(1) as usize]
                .to_string()
        }),
        "LAST_DAY" => unary_datetime(args, |mut dt| {
            dt.day = days_in_month(dt.year, dt.month);
            format_date(dt)
        }),
        "DATEDIFF" => {
            require_arity(args, 2, 2)?;
            Ok((days_from_datetime(parse_datetime(value(args, 0)?)?)
                - days_from_datetime(parse_datetime(value(args, 1)?)?))
            .to_string())
        }
        "TIMEDIFF" | "SUBTIME" => {
            require_arity(args, 2, 2)?;
            let seconds =
                parse_temporal_seconds(value(args, 0)?)? - parse_temporal_seconds(value(args, 1)?)?;
            Ok(format_duration(seconds))
        }
        "ADDTIME" => {
            require_arity(args, 2, 2)?;
            let base = value(args, 0)?;
            let seconds = parse_temporal_seconds(base)? + parse_duration(value(args, 1)?)?;
            if base.contains('-') {
                Ok(format_datetime(from_unix_seconds(seconds)))
            } else {
                Ok(format_duration(seconds))
            }
        }
        "TIME_TO_SEC" => {
            require_arity(args, 1, 1)?;
            Ok(parse_duration(value(args, 0)?)?.to_string())
        }
        "SEC_TO_TIME" => {
            require_arity(args, 1, 1)?;
            Ok(format_duration(parse_i64(value(args, 0)?)?))
        }
        "MAKETIME" => {
            require_arity(args, 3, 3)?;
            Ok(format!(
                "{:02}:{:02}:{:02}",
                parse_i64(value(args, 0)?)?,
                parse_i64(value(args, 1)?)?,
                parse_i64(value(args, 2)?)?
            ))
        }
        "MAKEDATE" => {
            require_arity(args, 2, 2)?;
            let year = parse_i64(value(args, 0)?)?;
            let day = parse_i64(value(args, 1)?)?;
            Ok(format_date(from_days(
                days_from_civil(year, 1, 1) + day - 1,
            )))
        }
        "TO_DAYS" => unary_datetime(args, |dt| (days_from_datetime(dt) + 719_528).to_string()),
        "FROM_DAYS" => {
            require_arity(args, 1, 1)?;
            Ok(format_date(from_days(
                parse_i64(value(args, 0)?)? - 719_528,
            )))
        }
        "TO_SECONDS" => unary_datetime(args, |dt| {
            ((days_from_datetime(dt) + 719_528) * 86_400
                + dt.hour * 3_600
                + dt.minute * 60
                + dt.second)
                .to_string()
        }),
        "UNIX_TIMESTAMP" => {
            require_arity(args, 0, 1)?;
            if args.is_empty() {
                Ok(SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string())
            } else {
                Ok(to_unix_seconds(parse_datetime(value(args, 0)?)?).to_string())
            }
        }
        "FROM_UNIXTIME" => {
            require_arity(args, 1, 2)?;
            let dt = from_unix_seconds(parse_i64(value(args, 0)?)?);
            if args.len() == 2 {
                format_datetime_value(dt, value(args, 1)?)
            } else {
                Ok(format_datetime(dt))
            }
        }
        "DATE_FORMAT" => {
            require_arity(args, 2, 2)?;
            format_datetime_value(parse_datetime(value(args, 0)?)?, value(args, 1)?)
        }
        "TIME_FORMAT" => {
            require_arity(args, 2, 2)?;
            format_datetime_value(parse_datetime(value(args, 0)?)?, value(args, 1)?)
        }
        "STR_TO_DATE" => {
            require_arity(args, 2, 2)?;
            str_to_date(value(args, 0)?, value(args, 1)?)
        }
        "PERIOD_ADD" => {
            require_arity(args, 2, 2)?;
            let (year, month) = parse_period(value(args, 0)?)?;
            let total = year * 12 + month - 1 + parse_i64(value(args, 1)?)?;
            Ok(format!(
                "{:04}{:02}",
                total.div_euclid(12),
                total.rem_euclid(12) + 1
            ))
        }
        "PERIOD_DIFF" => {
            require_arity(args, 2, 2)?;
            let (year1, month1) = parse_period(value(args, 0)?)?;
            let (year2, month2) = parse_period(value(args, 1)?)?;
            Ok(((year1 * 12 + month1) - (year2 * 12 + month2)).to_string())
        }
        "TIMESTAMP" => {
            require_arity(args, 1, 2)?;
            let dt = parse_datetime(value(args, 0)?)?;
            if args.len() == 1 {
                Ok(format_datetime(dt))
            } else {
                Ok(format_datetime(from_unix_seconds(
                    to_unix_seconds(dt) + parse_duration(value(args, 1)?)?,
                )))
            }
        }
        "DATE_ADD" | "ADDDATE" | "DATE_SUB" | "SUBDATE" => {
            require_arity(args, 2, 2)?;
            let mut dt = parse_datetime(value(args, 0)?)?;
            let subtract = matches!(name.as_str(), "DATE_SUB" | "SUBDATE");
            match &args[1] {
                DateFunctionArg::Interval { value, unit } => {
                    dt = add_interval(dt, value, unit, subtract)?;
                }
                DateFunctionArg::Value(days) if name == "ADDDATE" || name == "SUBDATE" => {
                    dt = add_interval(dt, days, "DAY", subtract)?;
                }
                _ => return Err(SqlRockError::new("expected INTERVAL argument")),
            }
            Ok(format_like_input(dt, value(args, 0)?))
        }
        "TIMESTAMPADD" => {
            require_arity(args, 3, 3)?;
            let dt = parse_datetime(value(args, 2)?)?;
            Ok(format_like_input(
                add_interval(dt, value(args, 1)?, value(args, 0)?, false)?,
                value(args, 2)?,
            ))
        }
        "TIMESTAMPDIFF" => {
            require_arity(args, 3, 3)?;
            let start = parse_datetime(value(args, 1)?)?;
            let end = parse_datetime(value(args, 2)?)?;
            Ok(timestamp_diff(value(args, 0)?, start, end)?.to_string())
        }
        "EXTRACT" => {
            require_arity(args, 2, 2)?;
            extract(value(args, 0)?, parse_datetime(value(args, 1)?)?)
        }
        "WEEK" | "WEEKOFYEAR" => {
            require_arity(args, 1, 2)?;
            Ok(iso_week(parse_datetime(value(args, 0)?)?).1.to_string())
        }
        "YEARWEEK" => {
            require_arity(args, 1, 2)?;
            let dt = parse_datetime(value(args, 0)?)?;
            let (year, week) = iso_week(dt);
            Ok(format!("{year}{week:02}"))
        }
        "CONVERT_TZ" => {
            require_arity(args, 3, 3)?;
            let dt = parse_datetime(value(args, 0)?)?;
            let from = parse_timezone(value(args, 1)?)?;
            let to = parse_timezone(value(args, 2)?)?;
            Ok(format_datetime(from_unix_seconds(
                to_unix_seconds(dt) - from + to,
            )))
        }
        "GET_FORMAT" => {
            require_arity(args, 2, 2)?;
            get_format(value(args, 0)?, value(args, 1)?)
        }
        _ => Err(SqlRockError::new(format!(
            "unsupported date function `{name}`"
        ))),
    }
}

fn require_arity(args: &[DateFunctionArg], min: usize, max: usize) -> Result<()> {
    if (min..=max).contains(&args.len()) {
        Ok(())
    } else {
        Err(SqlRockError::new(format!(
            "invalid date function argument count: expected {min}..={max}, got {}",
            args.len()
        )))
    }
}

fn value(args: &[DateFunctionArg], index: usize) -> Result<&str> {
    match args.get(index) {
        Some(DateFunctionArg::Value(value)) => Ok(value),
        _ => Err(SqlRockError::new("expected value argument")),
    }
}

fn unary_datetime(
    args: &[DateFunctionArg],
    function: impl FnOnce(DateTime) -> String,
) -> Result<String> {
    require_arity(args, 1, 1)?;
    Ok(function(parse_datetime(value(args, 0)?)?))
}

fn parse_datetime(value: &str) -> Result<DateTime> {
    let value = value.trim();
    if value.contains('-') {
        let (date, time) = value.split_once(' ').unwrap_or((value, "00:00:00"));
        let date = date.split('-').collect::<Vec<_>>();
        if date.len() != 3 {
            return invalid_datetime(value);
        }
        let (hour, minute, second) = parse_time_parts(time)?;
        Ok(DateTime {
            year: parse_i64(date[0])?,
            month: parse_i64(date[1])?,
            day: parse_i64(date[2])?,
            hour,
            minute,
            second,
        })
    } else if value.contains(':') {
        let (hour, minute, second) = parse_time_parts(value)?;
        Ok(DateTime {
            year: 1970,
            month: 1,
            day: 1,
            hour,
            minute,
            second,
        })
    } else {
        invalid_datetime(value)
    }
}

fn parse_time_parts(value: &str) -> Result<(i64, i64, i64)> {
    let value = value.split('.').next().unwrap_or(value);
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(SqlRockError::new(format!("invalid time `{value}`")));
    }
    Ok((
        parse_i64(parts[0])?,
        parse_i64(parts[1])?,
        parse_i64(parts[2])?,
    ))
}

fn invalid_datetime<T>(value: &str) -> Result<T> {
    Err(SqlRockError::new(format!("invalid datetime `{value}`")))
}

fn parse_i64(value: &str) -> Result<i64> {
    value
        .trim_matches('\'')
        .parse()
        .map_err(|_| SqlRockError::new(format!("invalid integer `{value}`")))
}

fn days_from_datetime(dt: DateTime) -> i64 {
    days_from_civil(dt.year, dt.month, dt.day)
}

fn to_unix_seconds(dt: DateTime) -> i64 {
    days_from_datetime(dt) * 86_400 + dt.hour * 3_600 + dt.minute * 60 + dt.second
}

fn from_unix_seconds(seconds: i64) -> DateTime {
    let mut dt = from_days(seconds.div_euclid(86_400));
    let rest = seconds.rem_euclid(86_400);
    dt.hour = rest / 3_600;
    dt.minute = rest % 3_600 / 60;
    dt.second = rest % 60;
    dt
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn from_days(days: i64) -> DateTime {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    DateTime {
        year: year + i64::from(month <= 2),
        month,
        day,
        hour: 0,
        minute: 0,
        second: 0,
    }
}

fn format_date(dt: DateTime) -> String {
    format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day)
}

fn format_time(dt: DateTime) -> String {
    format!("{:02}:{:02}:{:02}", dt.hour, dt.minute, dt.second)
}

fn format_datetime(dt: DateTime) -> String {
    format!("{} {}", format_date(dt), format_time(dt))
}

fn format_like_input(dt: DateTime, input: &str) -> String {
    if input.contains(' ') {
        format_datetime(dt)
    } else {
        format_date(dt)
    }
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn day_of_year(dt: DateTime) -> i64 {
    (1..dt.month)
        .map(|month| days_in_month(dt.year, month))
        .sum::<i64>()
        + dt.day
}

fn weekday_sunday_zero(dt: DateTime) -> i64 {
    (days_from_datetime(dt) + 4).rem_euclid(7)
}

fn iso_week(dt: DateTime) -> (i64, i64) {
    let weekday = (weekday_sunday_zero(dt) + 6) % 7;
    let thursday = from_days(days_from_datetime(dt) + 3 - weekday);
    let week1 = days_from_civil(thursday.year, 1, 4);
    let week1_weekday = (weekday_sunday_zero(from_days(week1)) + 6) % 7;
    let week = (days_from_datetime(dt) - (week1 - week1_weekday)).div_euclid(7) + 1;
    (thursday.year, week)
}

fn parse_duration(value: &str) -> Result<i64> {
    let value = value.trim_matches('\'');
    let (days, time) = value.split_once(' ').unwrap_or(("0", value));
    let sign = if value.starts_with('-') { -1 } else { 1 };
    let days = days.trim_start_matches('-').parse::<i64>().unwrap_or(0);
    let (hour, minute, second) = parse_time_parts(time.trim_start_matches('-'))?;
    Ok(sign * (days * 86_400 + hour * 3_600 + minute * 60 + second))
}

fn parse_temporal_seconds(value: &str) -> Result<i64> {
    if value.contains('-') {
        Ok(to_unix_seconds(parse_datetime(value)?))
    } else {
        parse_duration(value)
    }
}

fn format_duration(seconds: i64) -> String {
    let sign = if seconds < 0 { "-" } else { "" };
    let seconds = seconds.abs();
    format!(
        "{sign}{:02}:{:02}:{:02}",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    )
}

fn add_interval(mut dt: DateTime, value: &str, unit: &str, subtract: bool) -> Result<DateTime> {
    let mut amount = parse_i64(value)?;
    if subtract {
        amount = -amount;
    }
    match unit.to_ascii_uppercase().as_str() {
        "YEAR" => dt.year += amount,
        "QUARTER" => return add_interval(dt, &(amount * 3).to_string(), "MONTH", false),
        "MONTH" => {
            let total = dt.year * 12 + dt.month - 1 + amount;
            dt.year = total.div_euclid(12);
            dt.month = total.rem_euclid(12) + 1;
            dt.day = dt.day.min(days_in_month(dt.year, dt.month));
        }
        "WEEK" => return Ok(from_unix_seconds(to_unix_seconds(dt) + amount * 7 * 86_400)),
        "DAY" => return Ok(from_unix_seconds(to_unix_seconds(dt) + amount * 86_400)),
        "HOUR" => return Ok(from_unix_seconds(to_unix_seconds(dt) + amount * 3_600)),
        "MINUTE" => return Ok(from_unix_seconds(to_unix_seconds(dt) + amount * 60)),
        "SECOND" => return Ok(from_unix_seconds(to_unix_seconds(dt) + amount)),
        _ => {
            return Err(SqlRockError::new(format!(
                "unsupported interval unit `{unit}`"
            )));
        }
    }
    Ok(dt)
}

fn timestamp_diff(unit: &str, start: DateTime, end: DateTime) -> Result<i64> {
    let seconds = to_unix_seconds(end) - to_unix_seconds(start);
    match unit.to_ascii_uppercase().as_str() {
        "SECOND" => Ok(seconds),
        "MINUTE" => Ok(seconds / 60),
        "HOUR" => Ok(seconds / 3_600),
        "DAY" => Ok(seconds / 86_400),
        "WEEK" => Ok(seconds / (7 * 86_400)),
        "MONTH" => Ok((end.year - start.year) * 12 + end.month - start.month),
        "QUARTER" => Ok(((end.year - start.year) * 12 + end.month - start.month) / 3),
        "YEAR" => Ok(end.year - start.year),
        _ => Err(SqlRockError::new(format!(
            "unsupported interval unit `{unit}`"
        ))),
    }
}

fn extract(unit: &str, dt: DateTime) -> Result<String> {
    match unit.to_ascii_uppercase().as_str() {
        "YEAR" => Ok(dt.year.to_string()),
        "MONTH" => Ok(dt.month.to_string()),
        "DAY" => Ok(dt.day.to_string()),
        "HOUR" => Ok(dt.hour.to_string()),
        "MINUTE" => Ok(dt.minute.to_string()),
        "SECOND" => Ok(dt.second.to_string()),
        "YEAR_MONTH" => Ok(format!("{:04}{:02}", dt.year, dt.month)),
        "DAY_HOUR" => Ok(format!("{}{:02}", dt.day, dt.hour)),
        "HOUR_MINUTE" => Ok(format!("{:02}{:02}", dt.hour, dt.minute)),
        "MINUTE_SECOND" => Ok(format!("{:02}{:02}", dt.minute, dt.second)),
        _ => Err(SqlRockError::new(format!(
            "unsupported EXTRACT unit `{unit}`"
        ))),
    }
}

fn format_datetime_value(dt: DateTime, format: &str) -> Result<String> {
    let mut output = String::new();
    let mut chars = format.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }
        let specifier = chars
            .next()
            .ok_or_else(|| SqlRockError::new("incomplete date format"))?;
        output.push_str(&match specifier {
            'Y' => format!("{:04}", dt.year),
            'y' => format!("{:02}", dt.year.rem_euclid(100)),
            'm' => format!("{:02}", dt.month),
            'c' => dt.month.to_string(),
            'd' => format!("{:02}", dt.day),
            'e' => dt.day.to_string(),
            'H' => format!("{:02}", dt.hour),
            'k' => dt.hour.to_string(),
            'h' | 'I' => format!("{:02}", ((dt.hour + 11) % 12) + 1),
            'i' => format!("{:02}", dt.minute),
            's' | 'S' => format!("{:02}", dt.second),
            'p' => if dt.hour < 12 { "AM" } else { "PM" }.to_string(),
            'W' => {
                evaluate_date_function("DAYNAME", &[DateFunctionArg::Value(format_datetime(dt))])?
            }
            'M' => {
                evaluate_date_function("MONTHNAME", &[DateFunctionArg::Value(format_datetime(dt))])?
            }
            'j' => format!("{:03}", day_of_year(dt)),
            '%' => "%".to_string(),
            other => other.to_string(),
        });
    }
    Ok(output)
}

fn str_to_date(value: &str, format: &str) -> Result<String> {
    let mut year = 1970;
    let mut month = 1;
    let mut day = 1;
    let mut hour = 0;
    let mut minute = 0;
    let mut second = 0;
    let mut input = value.chars().peekable();
    let mut format_chars = format.chars().peekable();
    while let Some(ch) = format_chars.next() {
        if ch != '%' {
            if input.next() != Some(ch) {
                return invalid_datetime(value);
            }
            continue;
        }
        let spec = format_chars
            .next()
            .ok_or_else(|| SqlRockError::new("incomplete date format"))?;
        let width = match spec {
            'Y' => 4,
            'y' | 'm' | 'd' | 'H' | 'h' | 'I' | 'i' | 's' | 'S' => 2,
            _ => {
                return Err(SqlRockError::new(format!(
                    "unsupported date format `%{spec}`"
                )));
            }
        };
        let digits = (0..width)
            .map(|_| {
                input
                    .next()
                    .ok_or_else(|| SqlRockError::new("date input is too short"))
            })
            .collect::<Result<String>>()?;
        let number = parse_i64(&digits)?;
        match spec {
            'Y' => year = number,
            'y' => year = 2000 + number,
            'm' => month = number,
            'd' => day = number,
            'H' | 'h' | 'I' => hour = number,
            'i' => minute = number,
            's' | 'S' => second = number,
            _ => {}
        }
    }
    let dt = DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    };
    if format.contains("%H") || format.contains("%i") || format.contains("%s") {
        Ok(format_datetime(dt))
    } else {
        Ok(format_date(dt))
    }
}

fn parse_period(value: &str) -> Result<(i64, i64)> {
    let value = parse_i64(value)?;
    let mut year = value / 100;
    let month = value % 100;
    if year < 70 {
        year += 2000;
    } else if year < 100 {
        year += 1900;
    }
    Ok((year, month))
}

fn parse_timezone(value: &str) -> Result<i64> {
    match value.to_ascii_uppercase().as_str() {
        "UTC" | "GMT" => Ok(0),
        _ => {
            let sign = if value.starts_with('-') { -1 } else { 1 };
            let parts = value
                .trim_start_matches(['+', '-'])
                .split(':')
                .collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(SqlRockError::new(format!("unsupported timezone `{value}`")));
            }
            Ok(sign * (parse_i64(parts[0])? * 3_600 + parse_i64(parts[1])? * 60))
        }
    }
}

fn get_format(kind: &str, style: &str) -> Result<String> {
    let style = style.to_ascii_uppercase();
    let format = match (kind.to_ascii_uppercase().as_str(), style.as_str()) {
        ("DATE", "USA") => "%m.%d.%Y",
        ("DATE", "JIS") | ("DATE", "ISO") => "%Y-%m-%d",
        ("DATE", "EUR") => "%d.%m.%Y",
        ("DATE", "INTERNAL") => "%Y%m%d",
        ("DATETIME", "USA") => "%Y-%m-%d %H.%i.%s",
        ("DATETIME", "JIS") | ("DATETIME", "ISO") => "%Y-%m-%d %H:%i:%s",
        ("DATETIME", "EUR") => "%Y-%m-%d %H.%i.%s",
        ("DATETIME", "INTERNAL") => "%Y%m%d%H%i%s",
        ("TIME", "USA") => "%h:%i:%s %p",
        ("TIME", "JIS") | ("TIME", "ISO") => "%H:%i:%s",
        ("TIME", "EUR") => "%H.%i.%s",
        ("TIME", "INTERNAL") => "%H%i%s",
        _ => return Err(SqlRockError::new("unsupported GET_FORMAT arguments")),
    };
    Ok(format.to_string())
}

fn fractional_microseconds(value: &str) -> i64 {
    value
        .split_once('.')
        .map(|(_, fraction)| {
            format!("{fraction:0<6}")
                .chars()
                .take(6)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}
