//! DateTime picture string formatting and parsing for JSONata.
//!
//! Ported from Stedi's jsonata-rs `datetime.rs` — provides `format_custom_date`,
//! `parse_custom_format`, `parse_timezone_offset` and all helper functions.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use num_format::{Locale, ToFormattedString};

use crate::Error;

pub fn format_custom_date(date: &DateTime<FixedOffset>, picture: &str) -> Result<String, Error> {
    // Pre-scan for unclosed brackets (D3135 takes priority over other errors)
    check_balanced_brackets(picture)?;

    let mut formatted_string = String::new();
    let mut inside_brackets = false;
    let mut current_pattern = String::new();
    let mut i = 0;
    let chars: Vec<char> = picture.chars().collect();

    while i < chars.len() {
        let ch = chars[i];

        if ch == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            formatted_string.push('[');
            i += 2; // Skip both [[
            continue;
        }

        if ch == ']' && i + 1 < chars.len() && chars[i + 1] == ']' {
            formatted_string.push(']');
            i += 2; // Skip both ]]
            continue;
        }

        if ch == '[' {
            inside_brackets = true;
            current_pattern.clear(); // Start new pattern
            i += 1;
            continue;
        }

        if ch == ']' {
            inside_brackets = false;

            let trimmed_pattern = current_pattern.trim().replace(['\n', '\t', ' '], "");

            formatted_string.push_str(&handle_pattern(&trimmed_pattern, date)?);

            current_pattern.clear();
            i += 1;
            continue;
        }

        if inside_brackets {
            current_pattern.push(ch);
        } else {
            formatted_string.push(ch);
        }

        i += 1;
    }

    // If we ended while still inside brackets, that's a D3135 error
    if inside_brackets {
        return Err(Error::D3135PictureStringNoClosingBracket(
            "Invalid datetime picture string".to_string(),
        ));
    }

    Ok(formatted_string)
}

/// Pre-scan a picture string for unbalanced brackets.
/// D3135 (no closing bracket) takes priority over other datetime errors.
fn check_balanced_brackets(picture: &str) -> Result<(), Error> {
    let chars: Vec<char> = picture.chars().collect();
    let mut i = 0;
    let mut inside = false;

    while i < chars.len() {
        let ch = chars[i];
        // Skip escaped brackets [[ and ]]
        if ch == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            i += 2;
            continue;
        }
        if ch == ']' && i + 1 < chars.len() && chars[i + 1] == ']' {
            i += 2;
            continue;
        }
        if ch == '[' {
            inside = true;
        } else if ch == ']' {
            inside = false;
        }
        i += 1;
    }

    if inside {
        return Err(Error::D3135PictureStringNoClosingBracket(
            "Invalid datetime picture string".to_string(),
        ));
    }
    Ok(())
}

fn handle_pattern(pattern: &str, date: &DateTime<FixedOffset>) -> Result<String, Error> {
    match pattern {
        // Year patterns
        "X0001" => Ok(date.iso_week().year().to_string()),
        "Y" | "Y0001" | "Y0001,2" => Ok(date.format("%Y").to_string()),
        "Y,2" => Ok(date.format("%y").to_string()),
        "Y01" => Ok(date.format("%y").to_string()),
        "Y0001,2-2" | "Y##01,2-2" => handle_year_last_two_digits(date),
        "Y9,999,*" => Ok(date.year().to_formatted_string(&Locale::en)),
        "YI" => Ok(to_roman_numerals(date.year())),
        "Yi" => Ok(to_roman_numerals_lower(date.year())),
        "Yw" => Ok(to_year_in_words(date.year())),

        // Month patterns
        "M01" => Ok(date.format("%m").to_string()),
        "m01" => Ok(date.format("%M").to_string()),
        "M1,2" => Ok(format!("{:02}", date.month())),
        "M" | "M#1" => Ok(date.format("%-m").to_string()),
        "MA" => Ok(map_month_to_letter(date.month())),
        "MNn" => Ok(date.format("%B").to_string()),
        "MNn,3-3" => Ok(date.format("%B").to_string()[..3].to_string()),
        "MN" => Ok(date.format("%B").to_string().to_uppercase()),

        // Day patterns
        "D01" | "D#1,2" => Ok(date.format("%d").to_string()),
        "D" | "D#1" | "D1" => Ok(date.format("%-d").to_string()),
        "Da" => Ok(map_day_to_letter(date.day())),
        "Dwo" => Ok(format_day_in_words_with_ordinal(date.day())),
        "dwo" => Ok(format_day_in_words_with_ordinal(date.ordinal())),
        "D1o" => Ok(format_day_with_ordinal(date.day())),
        "d" => Ok(calculate_total_days_in_year(date)),

        // Week patterns
        "W01" => Ok(date.format("%V").to_string()),
        "W" => Ok(format!("{}", date.iso_week().week())),
        "w" => Ok(handle_week_of_month(date)),

        // Time patterns
        "H01" => Ok(date.format("%H").to_string()),
        "h" | "h#1" => Ok(date.format("%-I").to_string()),
        "m" => Ok(date.format("%M").to_string()),
        "s" | "s01" => Ok(date.format("%S").to_string()),
        "f001" => Ok(date.format("%3f").to_string()),

        // Timezone patterns
        "Z01:01t" | "Z01:01" | "Z0101t" => handle_timezone(date, pattern),
        "Z" => Ok(date.format("%:z").to_string()),
        "z" => Ok(format!("GMT{}", date.format("%:z"))),
        "Z0" => Ok(handle_trimmed_timezone(date)),
        s if s.starts_with('Z') && s.chars().filter(|c| c.is_ascii_digit()).count() > 4 => Err(
            Error::D3134TooManyTzDigits("Invalid datetime picture string".to_string()),
        ),

        // Day of the week patterns
        "F0" | "F1" => Ok(date.format("%u").to_string()),
        "FNn" => Ok(date.format("%A").to_string()),
        "FNn,3-3" => Ok(date.format("%A").to_string()[..3].to_string()),
        "F" => Ok(date.format("%A").to_string().to_lowercase()),

        // Period patterns
        "P" | "Pn" => Ok(date.format("%p").to_string().to_lowercase()),
        "PN" => Ok(date.format("%p").to_string()),

        // ISO/Era patterns
        "E" | "C" => Ok("ISO".to_string()),

        // Custom patterns
        "xNn" => Ok(handle_xnn(date)),

        "YN" => Err(Error::D3133PictureStringNameModifier(
            "Invalid datetime picture string".to_string(),
        )),
        // Fallback for unsupported patterns
        s => Err(Error::D3137Error(format!(
            "Unsupported datetime picture string: {s}"
        ))),
    }
}

pub fn parse_custom_format(timestamp_str: &str, picture: &str) -> Result<Option<i64>, Error> {
    match picture {
        // Handle ISO 8601 dates (including with timezone offsets like "+0000")
        "" => {
            // Handle year-only input (e.g., "2018")
            if let Some(millis) = parse_year_only(timestamp_str) {
                return Ok(Some(millis));
            }
            // Handle date-only input (e.g., "2017-10-30")
            if let Some(millis) = parse_date_only(timestamp_str) {
                return Ok(Some(millis));
            }
            // Handle ISO 8601 formats with timezone offsets (e.g., "2018-02-01T09:42:13.123+0000")
            if let Some(millis) = parse_iso8601_with_timezone(timestamp_str) {
                return Ok(Some(millis));
            }
            // Handle other standard ISO 8601 formats (e.g., "1970-01-01T00:00:00.001Z")
            if let Some(millis) = parse_iso8601_date(timestamp_str) {
                return Ok(Some(millis));
            }
            Ok(None)
        }

        // Handle the simple year format "[Y1]"
        "[Y1]" => {
            if let Ok(year) = timestamp_str.parse::<i32>() {
                let parsed_year = NaiveDate::from_ymd_opt(year, 1, 1);
                let time = NaiveTime::from_hms_opt(0, 0, 0);
                if let (Some(d), Some(t)) = (parsed_year, time) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle Roman numeral year format "[YI]" (e.g., 'MCMLXXXIV')
        "[YI]" => {
            if let Some(year) = roman_to_int(timestamp_str) {
                let parsed_year = NaiveDate::from_ymd_opt(year, 1, 1);
                let time = NaiveTime::from_hms_opt(0, 0, 0);
                if let (Some(d), Some(t)) = (parsed_year, time) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[Yw]' (e.g., 'one thousand, nine hundred and eighty-four')
        "[Yw]" => {
            let year = match words_to_number(&timestamp_str.to_lowercase()) {
                Some(y) => y,
                None => return Ok(None),
            };

            let parsed_date = NaiveDate::from_ymd_opt(year, 1, 1);
            let time = NaiveTime::from_hms_opt(0, 0, 0);
            if let (Some(d), Some(t)) = (parsed_date, time) {
                let datetime = NaiveDateTime::new(d, t);
                Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()))
            } else {
                Ok(None)
            }
        }

        "[Y]-[M]-[D]" => Ok(parse_ymd_date(timestamp_str)),

        // Handle the format '[H]:[m]' (e.g., '13:45')
        "[H]:[m]" => {
            let parts: Vec<&str> = timestamp_str.split(':').collect();
            if parts.len() == 2 {
                let hour: u32 = match parts[0].parse() {
                    Ok(h) => h,
                    Err(_) => return Ok(None),
                };
                let minute: u32 = match parts[1].parse() {
                    Ok(m) => m,
                    Err(_) => return Ok(None),
                };

                let now = Utc::now();
                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(now.year(), now.month(), now.day()),
                    NaiveTime::from_hms_opt(hour, minute, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Custom date format handling with time and AM/PM
        "[D1]/[M1]/[Y0001] [h]:[m] [P]" => {
            if let Some(parsed_datetime) = parse_custom_date(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Ok(Some(utc_datetime.timestamp_millis()));
            }
            Ok(None)
        }

        // Handle the format '[Y0001]-[d001]' (e.g., '2018-094')
        "[Y0001]-[d001]" => {
            if let Some(parsed_datetime) = parse_ordinal_date(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Ok(Some(utc_datetime.timestamp_millis()));
            }
            Ok(None)
        }

        // Handle the format '[FNn], [D1o] [MNn] [Y]' (e.g., 'Wednesday, 14th November 2018')
        "[FNn], [D1o] [MNn] [Y]" => {
            if let Some(parsed_datetime) = parse_custom_date_with_weekday(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Ok(Some(utc_datetime.timestamp_millis()));
            }
            Ok(None)
        }

        // Handle the format '[FNn,*-3], [DWwo] [MNn] [Y]' (e.g., 'Mon, Twelfth November 2018')
        "[FNn,*-3], [DWwo] [MNn] [Y]" => {
            if let Some(parsed_datetime) = parse_custom_date_with_weekday_and_ordinal(timestamp_str)
            {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Ok(Some(utc_datetime.timestamp_millis()));
            }
            Ok(None)
        }

        // Handle the format '[dwo] day of [Y]' (e.g., 'three hundred and sixty-fifth day of 2018')
        "[dwo] day of [Y]" => {
            if let Some(parsed_datetime) = parse_ordinal_day_of_year(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Ok(Some(utc_datetime.timestamp_millis()));
            }
            Ok(None)
        }

        // Handle the format '[Y]--[d]' (e.g., '2018--180')
        "[Y]--[d]" => {
            if let Some(parsed_datetime) = parse_ordinal_date_with_dashes(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Ok(Some(utc_datetime.timestamp_millis()));
            }
            Ok(None)
        }

        // Handle the format '[Dw] [MNn] [Y0001]' (e.g., 'twenty-seven April 2008')
        "[Dw] [MNn] [Y0001]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() != 3 {
                return Ok(None);
            }

            let day_str = remove_day_suffix(parts[0]);
            let day = match words_to_number(&day_str.to_lowercase()) {
                Some(d) => d as u32,
                None => return Ok(None),
            };

            let month = match month_name_to_int(parts[1]) {
                Some(m) => m,
                None => return Ok(None),
            };

            let year_str = parts[2..].join(" ");
            let year = match year_str.parse::<i32>() {
                Ok(num) => num,
                Err(_) => match words_to_number(&year_str) {
                    Some(y) => y,
                    None => return Ok(None),
                },
            };

            if let (Some(d), Some(t)) = (
                NaiveDate::from_ymd_opt(year, month, day),
                NaiveTime::from_hms_opt(0, 0, 0),
            ) {
                let datetime = NaiveDateTime::new(d, t);
                Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()))
            } else {
                Ok(None)
            }
        }

        // Handle the format '[D1] [M01] [YI]' (e.g., '27 03 MMXVIII')
        "[D1] [M01] [YI]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = match parts[0].parse() {
                    Ok(d) => d,
                    Err(_) => return Ok(None),
                };
                let month: u32 = match parts[1].parse() {
                    Ok(m) => m,
                    Err(_) => return Ok(None),
                };
                let year = match roman_to_int(parts[2]) {
                    Some(y) => y,
                    None => return Ok(None),
                };
                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(0, 0, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[D1] [Mi] [YI]' (e.g., '27 iii MMXVIII')
        "[D1] [Mi] [YI]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = match parts[0].parse() {
                    Ok(d) => d,
                    Err(_) => return Ok(None),
                };
                let month = match roman_to_int(parts[1].to_uppercase().as_str()) {
                    Some(m) => m as u32,
                    None => return Ok(None),
                };
                let year = match roman_to_int(parts[2]) {
                    Some(y) => y,
                    None => return Ok(None),
                };

                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(0, 0, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[Da] [MA] [Yi]' (e.g., 'w C mmxviii')
        "[Da] [MA] [Yi]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let month = match roman_month_to_int(parts[1]) {
                    Some(m) => m,
                    None => return Ok(None),
                };
                let year = match roman_to_int(parts[2].to_uppercase().as_str()) {
                    Some(y) => y,
                    None => return Ok(None),
                };
                let day = match alphabetic_to_day(parts[0]) {
                    Some(d) => d,
                    None => return Ok(None),
                };

                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(0, 0, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[D1o] [M#1] [Y0001]' (e.g., '27th 3 1976')
        "[D1o] [M#1] [Y0001]" => {
            let cleaned_timestamp = timestamp_str
                .replace("th", "")
                .replace("st", "")
                .replace("nd", "")
                .replace("rd", "");
            let parts: Vec<&str> = cleaned_timestamp.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = match parts[0].parse() {
                    Ok(d) => d,
                    Err(_) => return Ok(None),
                };
                let month: u32 = match parts[1].parse() {
                    Ok(m) => m,
                    Err(_) => return Ok(None),
                };
                let year: i32 = match parts[2].parse() {
                    Ok(y) => y,
                    Err(_) => return Ok(None),
                };
                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(0, 0, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[D1o] [MNn] [Y0001]' (e.g., '27th April 2008')
        "[D1o] [MNn] [Y0001]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day = match remove_ordinal_suffix(parts[0]) {
                    Some(d) => d,
                    None => return Ok(None),
                };
                let month = match month_name_to_int(parts[1]) {
                    Some(m) => m,
                    None => return Ok(None),
                };
                let year: i32 = match parts[2].parse() {
                    Ok(y) => y,
                    Err(_) => return Ok(None),
                };

                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(0, 0, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[D1] [MNn] [Y0001]' (e.g., '21 August 2017')
        "[D1] [MNn] [Y0001]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = match parts[0].parse() {
                    Ok(d) => d,
                    Err(_) => return Ok(None),
                };
                let month = match month_name_to_int(parts[1]) {
                    Some(m) => m,
                    None => return Ok(None),
                };
                let year: i32 = match parts[2].parse() {
                    Ok(y) => y,
                    Err(_) => return Ok(None),
                };

                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(0, 0, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[D1] [MNn,3-3] [Y0001]' (e.g., '2 Feb 2012')
        "[D1] [MNn,3-3] [Y0001]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = match parts[0].parse() {
                    Ok(d) => d,
                    Err(_) => return Ok(None),
                };
                let month = match abbreviated_month_to_int(parts[1]) {
                    Some(m) => m,
                    None => return Ok(None),
                };
                let year: i32 = match parts[2].parse() {
                    Ok(y) => y,
                    Err(_) => return Ok(None),
                };

                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(0, 0, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[D1o] [M01] [Y0001]' (e.g., '21st 12 1881')
        "[D1o] [M01] [Y0001]" => {
            let cleaned_timestamp = timestamp_str
                .replace("th", "")
                .replace("st", "")
                .replace("nd", "")
                .replace("rd", "");
            let parts: Vec<&str> = cleaned_timestamp.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = match parts[0].parse() {
                    Ok(d) => d,
                    Err(_) => return Ok(None),
                };
                let month: u32 = match parts[1].parse() {
                    Ok(m) => m,
                    Err(_) => return Ok(None),
                };
                let year: i32 = match parts[2].parse() {
                    Ok(y) => y,
                    Err(_) => return Ok(None),
                };
                if let (Some(d), Some(t)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(0, 0, 0),
                ) {
                    let datetime = NaiveDateTime::new(d, t);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle the format '[D01]/[M01]/[Y0001] [H01]:[m01]:[s01]' (e.g., '13/09/2024 13:45:00')
        "[D01]/[M01]/[Y0001] [H01]:[m01]:[s01]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() != 2 {
                return Ok(None);
            }

            let date_part = parts[0];
            let time_part = parts[1];

            let date_elements: Vec<&str> = date_part.split('/').collect();
            if date_elements.len() != 3 {
                return Ok(None);
            }

            let day: u32 = match date_elements[0].parse() {
                Ok(d) => d,
                Err(_) => return Ok(None),
            };
            let month: u32 = match date_elements[1].parse() {
                Ok(m) => m,
                Err(_) => return Ok(None),
            };
            let year: i32 = match date_elements[2].parse() {
                Ok(y) => y,
                Err(_) => return Ok(None),
            };

            let time_elements: Vec<&str> = time_part.split(':').collect();
            if time_elements.len() != 3 {
                return Ok(None);
            }

            let hour: u32 = match time_elements[0].parse() {
                Ok(h) => h,
                Err(_) => return Ok(None),
            };
            let minute: u32 = match time_elements[1].parse() {
                Ok(m) => m,
                Err(_) => return Ok(None),
            };
            let second: u32 = match time_elements[2].parse() {
                Ok(s) => s,
                Err(_) => return Ok(None),
            };

            if let (Some(d), Some(t)) = (
                NaiveDate::from_ymd_opt(year, month, day),
                NaiveTime::from_hms_opt(hour, minute, second),
            ) {
                let datetime = NaiveDateTime::new(d, t);
                let utc_datetime: DateTime<Utc> = Utc.from_utc_datetime(&datetime);
                Ok(Some(utc_datetime.timestamp_millis()))
            } else {
                Ok(None)
            }
        }

        // Handle ISO 8601-like formats with custom pattern handling
        "[Y0001]-[M01]-[D01]" => {
            if let Ok(parsed_date) = NaiveDate::parse_from_str(timestamp_str, "%Y-%m-%d") {
                if let Some(time) = NaiveTime::from_hms_opt(0, 0, 0) {
                    let datetime = NaiveDateTime::new(parsed_date, time);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        // Handle ISO 8601-like formats with custom pattern handling like '[Y1]-[M01]-[D01]'
        "[Y1]-[M01]-[D01]" => {
            if let Ok(parsed_date) = NaiveDate::parse_from_str(timestamp_str, "%Y-%m-%d") {
                if let Some(time) = NaiveTime::from_hms_opt(0, 0, 0) {
                    let datetime = NaiveDateTime::new(parsed_date, time);
                    return Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()));
                }
            }
            Ok(None)
        }

        "[Y0001]-[M01]-[D01]T[H01]:[m01]:[s01].[f001]Z" => {
            if let Ok(parsed_datetime) = DateTime::parse_from_rfc3339(timestamp_str) {
                return Ok(Some(parsed_datetime.timestamp_millis()));
            }
            Ok(None)
        }

        // Handle the format '[Dw] [MNn] [Yw]' (e.g., 'twenty-first August two thousand and seventeen')
        "[Dw] [MNn] [Yw]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() < 5 {
                return Ok(None);
            }

            let day_str = parse_day_str(parts[0]);
            let day = match words_to_number(&day_str.to_lowercase()) {
                Some(d) => d as u32,
                None => return Ok(None),
            };
            let month = match month_name_to_int(parts[1]) {
                Some(m) => m,
                None => return Ok(None),
            };

            let year_str = parts[2..].join(" ");
            let year = match words_to_number(&year_str.to_lowercase()) {
                Some(y) => y,
                None => return Ok(None),
            };

            if let (Some(d), Some(t)) = (
                NaiveDate::from_ymd_opt(year, month, day),
                NaiveTime::from_hms_opt(0, 0, 0),
            ) {
                let datetime = NaiveDateTime::new(d, t);
                Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()))
            } else {
                Ok(None)
            }
        }

        "[DW] [MNn] [Yw]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() < 5 {
                return Ok(None);
            }

            let day_str = parse_day_str(parts[0]);
            let day = match words_to_number(&day_str.to_lowercase()) {
                Some(d) => d as u32,
                None => return Ok(None),
            };
            let month = match month_name_to_int(parts[1]) {
                Some(m) => m,
                None => return Ok(None),
            };

            let year_str = parts[2..].join(" ");
            let year = match words_to_number(&year_str.to_lowercase()) {
                Some(y) => y,
                None => return Ok(None),
            };

            if let (Some(d), Some(t)) = (
                NaiveDate::from_ymd_opt(year, month, day),
                NaiveTime::from_hms_opt(0, 0, 0),
            ) {
                let datetime = NaiveDateTime::new(d, t);
                Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()))
            } else {
                Ok(None)
            }
        }

        // Handle the format '[DW] of [MNn], [Yw]'
        "[DW] of [MNn], [Yw]" => {
            let cleaned_str = timestamp_str.replace("of", "").replace(',', "");
            let parts: Vec<&str> = cleaned_str.split_whitespace().collect();
            if parts.len() < 5 {
                return Ok(None);
            }

            let day_str = parts[0];
            let day = match words_to_number(&day_str.to_lowercase()) {
                Some(d) => d as u32,
                None => return Ok(None),
            };
            let month = match month_name_to_int(parts[1]) {
                Some(m) => m,
                None => return Ok(None),
            };

            let year_str = parts[2..].join(" ");
            let year = match words_to_number(&year_str.to_lowercase()) {
                Some(y) => y,
                None => return Ok(None),
            };

            if let (Some(d), Some(t)) = (
                NaiveDate::from_ymd_opt(year, month, day),
                NaiveTime::from_hms_opt(0, 0, 0),
            ) {
                let datetime = NaiveDateTime::new(d, t);
                Ok(Some(Utc.from_utc_datetime(&datetime).timestamp_millis()))
            } else {
                Ok(None)
            }
        }

        // Check for error-producing patterns before the default fallback
        _ => {
            // Analyze the picture string for known error patterns
            analyze_picture_for_errors(picture)
        }
    }
}

/// Analyze a picture string for known error patterns and return appropriate errors.
/// If no specific error pattern is found, return Ok(None) to indicate the format
/// is simply not recognized (which upstream code may treat as undefined).
fn analyze_picture_for_errors(picture: &str) -> Result<Option<i64>, Error> {
    // Extract all component specifiers from the picture string
    let chars: Vec<char> = picture.chars().collect();
    let mut i = 0;
    let mut components: Vec<String> = Vec::new();
    let mut has_year = false;
    let mut has_month = false;
    let mut has_day = false;
    let mut has_day_of_year = false;
    let mut has_hour = false;
    let mut has_minute = false;
    let mut _has_second = false;
    let mut has_week_year = false;
    let mut has_week = false;
    let mut has_day_of_week = false;
    let mut has_week_of_month = false;

    while i < chars.len() {
        if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            i += 2;
            continue;
        }
        if chars[i] == '[' {
            let start = i + 1;
            i += 1;
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
            if i < chars.len() {
                let component: String = chars[start..i].iter().collect();
                let trimmed = component.trim().replace(' ', "");
                components.push(trimmed);
            }
            i += 1;
            continue;
        }
        i += 1;
    }

    for comp in &components {
        if comp.is_empty() {
            continue;
        }
        let first_char = comp.chars().next().unwrap();
        match first_char {
            'Y' => {
                // Check for named year (YN) which is an error
                if comp.starts_with("YN") {
                    return Err(Error::D3133PictureStringNameModifier(
                        "Invalid datetime picture string".to_string(),
                    ));
                }
                has_year = true;
            }
            'M' => has_month = true,
            'D' => has_day = true,
            'd' => has_day_of_year = true,
            'H' | 'h' => has_hour = true,
            'm' => has_minute = true,
            's' => _has_second = true,
            'X' => has_week_year = true,
            'W' => has_week = true,
            'w' => has_week_of_month = true,
            'F' => has_day_of_week = true,
            'f' | 'Z' | 'z' | 'P' | 'E' | 'C' | 'x' => {}
            _ => {
                // Unknown component
                return Err(Error::D3132UnknownComponent(format!(
                    "Unknown component: {}",
                    first_char
                )));
            }
        }
    }

    // Check for underspecified date/time (D3136)
    // If we have year + day but no month and no day-of-year
    if has_year && has_day && !has_month && !has_day_of_year {
        return Err(Error::D3136DatetimeComponentsMissing(
            "The datetime components are underspecified".to_string(),
        ));
    }
    // If we have month + day + minute + second but no hour
    if has_month && has_day && has_minute && !has_hour {
        return Err(Error::D3136DatetimeComponentsMissing(
            "The datetime components are underspecified".to_string(),
        ));
    }
    // week-based date: X/x/w/F combinations without proper support
    if has_week_year && has_week_of_month && has_day_of_week {
        return Err(Error::D3136DatetimeComponentsMissing(
            "The datetime components are underspecified".to_string(),
        ));
    }
    if has_week_year && has_week && has_day_of_week {
        return Err(Error::D3136DatetimeComponentsMissing(
            "The datetime components are underspecified".to_string(),
        ));
    }

    Ok(None)
}

fn parse_day_str(day_str: &str) -> String {
    day_str
        .split('-')
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

fn handle_year_last_two_digits(date: &DateTime<FixedOffset>) -> Result<String, Error> {
    let year = date.year();
    let last_two_digits = year % 100;
    Ok(format!("{:02}", last_two_digits))
}

fn map_day_to_letter(day: u32) -> String {
    match day {
        1..=26 => (b'a' + (day - 1) as u8) as char,
        27..=31 => (b'a' + (day - 27) as u8) as char,
        _ => ' ',
    }
    .to_string()
}

fn map_month_to_letter(month: u32) -> String {
    match month {
        1..=12 => (b'a' + (month - 1) as u8) as char,
        _ => ' ',
    }
    .to_uppercase()
    .to_string()
}

fn calculate_total_days_in_year(date: &DateTime<FixedOffset>) -> String {
    let total_days = if date.date_naive().leap_year() {
        366
    } else {
        365
    };
    total_days.to_string()
}

fn handle_week_of_month(date: &DateTime<FixedOffset>) -> String {
    let iso_week = date.iso_week().week();
    let month = date.month();
    let day_of_month = date.day();
    let first_day_of_month = date.with_day(1).unwrap();
    let first_weekday_of_month = first_day_of_month.weekday().num_days_from_sunday();
    let week_of_month = ((day_of_month + first_weekday_of_month - 1) / 7) + 1;

    if (month == 12 && iso_week == 1)
        || (week_of_month == 5 && month == 1 && iso_week == 5)
        || (week_of_month == 1 && first_weekday_of_month == 5 && iso_week == 5)
    {
        format!("{}", iso_week)
    } else if week_of_month == 5 && first_weekday_of_month == 0 {
        format!("{}", 1)
    } else if month == 1 && iso_week >= 52 && first_weekday_of_month == 0 {
        format!("{}", 5)
    } else {
        format!("{}", week_of_month)
    }
}

fn handle_trimmed_timezone(date: &DateTime<FixedOffset>) -> String {
    let tz_offset = date.format("%z").to_string();

    if tz_offset == "+0000" || tz_offset == "-0000" {
        "0".to_string()
    } else if tz_offset[3..] == *"00" {
        format!(
            "{}{}",
            &tz_offset[..1],
            tz_offset[1..3].trim_start_matches('0')
        )
    } else {
        format!(
            "{}{}:{}",
            &tz_offset[..1],
            tz_offset[1..3].trim_start_matches('0'),
            &tz_offset[3..]
        )
    }
}

fn handle_xnn(date: &DateTime<FixedOffset>) -> String {
    let days_from_monday = date.weekday().num_days_from_monday() as i64;
    let first_day_of_week = *date - chrono::Duration::days(days_from_monday);
    let last_day_of_week = first_day_of_week + chrono::Duration::days(6);
    let first_day_month = first_day_of_week.month();
    let last_day_month = last_day_of_week.month();

    let week_month = if first_day_month != last_day_month {
        if last_day_of_week.day() >= 4 {
            last_day_month
        } else {
            first_day_month
        }
    } else {
        first_day_month
    };

    chrono::NaiveDate::from_ymd_opt(date.year(), week_month, 1)
        .expect("Invalid month or day")
        .format("%B")
        .to_string()
}

fn handle_timezone(date: &DateTime<FixedOffset>, pattern: &str) -> Result<String, Error> {
    match pattern {
        "Z01:01t" => {
            if date.offset().local_minus_utc() == 0 {
                Ok("Z".to_string())
            } else {
                Ok(date.format("%:z").to_string())
            }
        }
        "Z01:01" => {
            if date.offset().local_minus_utc() == 0 {
                Ok("+00:00".to_string())
            } else {
                let offset_minutes = date.offset().local_minus_utc() / 60;
                let hours = offset_minutes / 60;
                let minutes = offset_minutes % 60;
                Ok(format!("{:+03}:{:02}", hours, minutes))
            }
        }
        "Z0101t" => {
            if date.offset().local_minus_utc() == 0 {
                Ok("Z".to_string())
            } else {
                let offset_minutes = date.offset().local_minus_utc() / 60;
                let hours = offset_minutes / 60;
                let minutes = offset_minutes % 60;
                Ok(format!("{:+03}{:02}", hours, minutes))
            }
        }
        _ => Err(Error::D3134TooManyTzDigits(
            "Invalid timezone format".to_string(),
        )),
    }
}

pub fn format_day_with_ordinal(day: u32) -> String {
    match day {
        1 | 21 | 31 => format!("{}st", day),
        2 | 22 => format!("{}nd", day),
        3 | 23 => format!("{}rd", day),
        _ => format!("{}th", day),
    }
}

fn to_year_in_words(year: i32) -> String {
    if year < 0 {
        return format!("minus {}", to_year_in_words(-year));
    }

    let below_20 = [
        "",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    let tens = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];

    let mut result = String::new();
    let mut y = year;

    if y >= 1000 {
        let thousands = y / 1000;
        result.push_str(below_20[thousands as usize]);
        result.push_str(" thousand");
        y %= 1000;

        if y > 0 && y < 100 {
            result.push_str(" and ");
        } else if y > 0 {
            result.push(' ');
        }
    }

    if y >= 100 {
        let hundreds = y / 100;
        result.push_str(below_20[hundreds as usize]);
        result.push_str(" hundred");
        y %= 100;
        if y > 0 {
            result.push_str(" and ");
        }
    }

    if y >= 20 {
        let t = y / 10;
        result.push_str(tens[t as usize]);
        y %= 10;

        if y > 0 {
            result.push('-');
        }
    }

    if y > 0 {
        result.push_str(below_20[y as usize]);
    }

    result.trim().to_string()
}

pub fn to_roman_numerals(year: i32) -> String {
    let mut year = year;
    let mut roman = String::new();
    let numerals = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    for &(value, symbol) in &numerals {
        while year >= value {
            roman.push_str(symbol);
            year -= value;
        }
    }
    roman
}

pub fn to_roman_numerals_lower(year: i32) -> String {
    to_roman_numerals(year).to_lowercase()
}

// Helper function to parse timezone strings like "+HHMM"
pub fn parse_timezone_offset(timezone: &str) -> Option<FixedOffset> {
    if timezone == "0000" {
        return FixedOffset::east_opt(0); // UTC
    }
    if timezone.len() != 5 {
        return None;
    }

    let (hours, minutes) = (
        timezone[1..3].parse::<i32>().ok()?,
        timezone[3..5].parse::<i32>().ok()?,
    );
    let total_offset_seconds = (hours * 3600) + (minutes * 60);

    match &timezone[0..1] {
        "+" => FixedOffset::east_opt(total_offset_seconds),
        "-" => FixedOffset::west_opt(total_offset_seconds),
        _ => None,
    }
}

fn format_day_in_words_with_ordinal(day: u32) -> String {
    let word = to_words(day);

    // Special cases for 11th, 12th, and 13th
    if (11..=13).contains(&(day % 100)) {
        return format!("{}th", word);
    }

    if word.ends_with("first") || word.ends_with("second") || word.ends_with("third") {
        return word;
    }

    let suffix = match day % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };

    format!("{}{}", word, suffix)
}

fn to_words(num: u32) -> String {
    let below_20 = [
        "",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
    ];
    let tens = [
        "",
        "",
        "twentieth",
        "thirtieth",
        "fortieth",
        "fiftieth",
        "sixtieth",
        "seventieth",
        "eightieth",
        "ninetieth",
    ];
    let tens_with_units = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];

    if num < 20 {
        below_20[num as usize].to_string()
    } else if num < 100 {
        if num % 10 == 0 {
            return tens[(num / 10) as usize].to_string();
        }
        let ten = tens_with_units[(num / 10) as usize];
        let unit = below_20[(num % 10) as usize];
        format!("{}-{}", ten, unit)
    } else if num < 1000 {
        let hundreds = num / 100;
        let remainder = num % 100;
        let below_20_cardinal = [
            "",
            "one",
            "two",
            "three",
            "four",
            "five",
            "six",
            "seven",
            "eight",
            "nine",
            "ten",
            "eleven",
            "twelve",
            "thirteen",
            "fourteen",
            "fifteen",
            "sixteen",
            "seventeen",
            "eighteen",
            "nineteen",
        ];
        let hundreds_word = below_20_cardinal[hundreds as usize];
        if remainder == 0 {
            format!("{} hundredth", hundreds_word)
        } else {
            format!("{} hundred and {}", hundreds_word, to_words(remainder))
        }
    } else {
        num.to_string()
    }
}

fn roman_to_int(s: &str) -> Option<i32> {
    let mut total = 0;
    let mut prev_value = 0;

    for c in s.chars().rev() {
        let value = match c {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        };

        if value < prev_value {
            total -= value;
        } else {
            total += value;
        }

        prev_value = value;
    }

    Some(total)
}

pub fn roman_month_to_int(month_str: &str) -> Option<u32> {
    match month_str.to_uppercase().as_str() {
        "I" | "A" => Some(1),
        "II" | "B" => Some(2),
        "III" | "C" => Some(3),
        "IV" | "D" => Some(4),
        "V" | "E" => Some(5),
        "VI" | "F" => Some(6),
        "VII" | "G" => Some(7),
        "VIII" | "H" => Some(8),
        "IX" => Some(9),
        "X" | "J" => Some(10),
        "XI" | "K" => Some(11),
        "XII" | "L" => Some(12),
        _ => None,
    }
}

fn alphabetic_to_day(s: &str) -> Option<u32> {
    let chars: Vec<char> = s.chars().collect();

    if chars.len() == 1 {
        let day = chars[0].to_ascii_lowercase() as u32 - 'a' as u32 + 1;
        return if day <= 31 { Some(day) } else { None };
    } else if chars.len() == 2 {
        let first = chars[0].to_ascii_lowercase() as u32 - 'a' as u32 + 1;
        let second = chars[1].to_ascii_lowercase() as u32 - 'a' as u32 + 1;

        let day = first * 26 + second;
        return if day <= 31 { Some(day) } else { None };
    }

    None
}

fn remove_day_suffix(day_str: &str) -> String {
    if day_str.ends_with("st") {
        day_str.trim_end_matches("st").to_string()
    } else if day_str.ends_with("nd") {
        day_str.trim_end_matches("nd").to_string()
    } else if day_str.ends_with("rd") {
        day_str.trim_end_matches("rd").to_string()
    } else if day_str.ends_with("th") {
        day_str.trim_end_matches("th").to_string()
    } else {
        day_str.to_string()
    }
}

fn remove_ordinal_suffix(day_str: &str) -> Option<u32> {
    let cleaned_day = day_str.trim_end_matches(|c: char| c.is_alphabetic());
    cleaned_day.parse::<u32>().ok()
}

fn month_name_to_int(month_str: &str) -> Option<u32> {
    match month_str.to_lowercase().as_str() {
        "january" => Some(1),
        "february" => Some(2),
        "march" => Some(3),
        "april" => Some(4),
        "may" => Some(5),
        "june" => Some(6),
        "july" => Some(7),
        "august" => Some(8),
        "september" => Some(9),
        "october" => Some(10),
        "november" => Some(11),
        "december" => Some(12),
        _ => None,
    }
}

fn abbreviated_month_to_int(month_str: &str) -> Option<u32> {
    match month_str.to_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

fn words_to_number(word_str: &str) -> Option<i32> {
    let units = [
        ("zero", 0),
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
        ("thirteen", 13),
        ("fourteen", 14),
        ("fifteen", 15),
        ("sixteen", 16),
        ("seventeen", 17),
        ("eighteen", 18),
        ("nineteen", 19),
        // Ordinal units
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("sixth", 6),
        ("seventh", 7),
        ("eighth", 8),
        ("ninth", 9),
        ("tenth", 10),
        ("eleventh", 11),
        ("twelfth", 12),
        ("thirteenth", 13),
        ("fourteenth", 14),
        ("fifteenth", 15),
        ("sixteenth", 16),
        ("seventeenth", 17),
        ("eighteenth", 18),
        ("nineteenth", 19),
        ("twentieth", 20),
        ("twenty-first", 21),
        ("twenty-second", 22),
        ("twenty-third", 23),
        ("twenty-fourth", 24),
        ("twenty-fifth", 25),
        ("twenty-sixth", 26),
        ("twenty-seventh", 27),
        ("twenty-eighth", 28),
        ("twenty-ninth", 29),
        ("thirtieth", 30),
        ("thirty-first", 31),
    ];

    let tens = [
        ("twenty", 20),
        ("thirty", 30),
        ("forty", 40),
        ("fifty", 50),
        ("sixty", 60),
        ("seventy", 70),
        ("eighty", 80),
        ("ninety", 90),
    ];

    let scales = [("hundred", 100), ("thousand", 1000)];

    let mut result = 0;
    let mut current = 0;
    let mut last_ten = None;

    for word in word_str
        .replace(',', "")
        .to_lowercase()
        .split_whitespace()
        .flat_map(|w| w.split('-'))
        .collect::<Vec<_>>()
    {
        if word == "and" {
            continue;
        }

        if let Some(unit) = units.iter().find(|&&(w, _)| w == word).map(|(_, n)| n) {
            if let Some(ten) = last_ten {
                current += ten + unit;
                last_ten = None;
            } else {
                current += unit;
            }
        } else if let Some(ten) = tens.iter().find(|&&(w, _)| w == word).map(|(_, n)| n) {
            if let Some(ten_value) = last_ten {
                current += ten_value + ten;
            } else {
                last_ten = Some(ten);
            }
        } else if let Some(scale) = scales.iter().find(|&&(w, _)| w == word).map(|(_, n)| n) {
            if *scale == 100 {
                current *= scale;
            } else if *scale == 1000 {
                result += current * scale;
                current = 0;
            }
        }
    }

    result += current;
    Some(result)
}

fn parse_custom_date(date_str: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = date_str.split_whitespace().collect();

    if parts.len() != 3 {
        return None;
    }

    let date_parts: Vec<&str> = parts[0].split('/').collect();
    if date_parts.len() != 3 {
        return None;
    }

    let day: u32 = date_parts[0].parse().ok()?;
    let month: u32 = date_parts[1].parse().ok()?;
    let year: i32 = date_parts[2].parse().ok()?;

    let time_parts: Vec<&str> = parts[1].split(':').collect();
    if time_parts.len() != 2 {
        return None;
    }

    let mut hour: u32 = time_parts[0].parse().ok()?;
    let minute: u32 = time_parts[1].parse().ok()?;

    let am_pm = parts[2].to_lowercase();
    if am_pm == "am" {
        if hour == 12 {
            hour = 0;
        }
    } else if am_pm == "pm" {
        if hour != 12 {
            hour += 12;
        }
    } else {
        return None;
    }

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    Some(NaiveDateTime::new(date, time))
}

fn parse_ordinal_date(date_str: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = date_str.split('-').collect();

    if parts.len() != 2 {
        return None;
    }

    let year: i32 = parts[0].parse().ok()?;
    let ordinal_day: u32 = parts[1].parse().ok()?;

    let date = NaiveDate::from_yo_opt(year, ordinal_day)?;
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;

    Some(NaiveDateTime::new(date, time))
}

fn parse_custom_date_with_weekday(date_str: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = date_str
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|&x| !x.is_empty())
        .collect();

    if parts.len() != 4 {
        return None;
    }

    let day_str = parts[1].trim_end_matches(|c: char| c.is_alphabetic());
    let day: u32 = day_str.parse().ok()?;

    let month = match parts[2].to_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        _ => return None,
    };

    let year: i32 = parts[3].parse().ok()?;

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;
    Some(NaiveDateTime::new(date, time))
}

fn parse_custom_date_with_weekday_and_ordinal(date_str: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = date_str
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|&x| !x.is_empty())
        .collect();

    if parts.len() != 4 {
        return None;
    }

    let day = words_to_number(parts[1])? as u32;

    let month = match parts[2].to_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        _ => return None,
    };

    let year: i32 = parts[3].parse().ok()?;

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;
    Some(NaiveDateTime::new(date, time))
}

fn parse_ordinal_day_of_year(date_str: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = date_str.split_whitespace().collect();

    if parts.len() < 5 {
        return None;
    }

    let ordinal_day_words = parts[..(parts.len() - 3)].join(" ");
    let day_of_year = words_to_number(&ordinal_day_words)? as u32;

    let year: i32 = parts.last()?.parse().ok()?;

    let parsed_date = NaiveDate::from_yo_opt(year, day_of_year)?;
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;
    Some(NaiveDateTime::new(parsed_date, time))
}

fn parse_ordinal_date_with_dashes(date_str: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = date_str.split("--").collect();

    if parts.len() != 2 {
        return None;
    }

    let year: i32 = parts[0].parse().ok()?;
    let ordinal_day: u32 = parts[1].parse().ok()?;

    let date = NaiveDate::from_yo_opt(year, ordinal_day)?;
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;

    Some(NaiveDateTime::new(date, time))
}

fn parse_iso8601_date(date_str: &str) -> Option<i64> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(date_str) {
        Some(datetime.timestamp_millis())
    } else {
        None
    }
}

fn parse_iso8601_with_timezone(date_str: &str) -> Option<i64> {
    let normalized_str = if date_str.ends_with("+0000") {
        date_str.replace("+0000", "+00:00")
    } else {
        date_str.to_string()
    };

    if let Ok(datetime) = DateTime::parse_from_rfc3339(&normalized_str) {
        Some(datetime.timestamp_millis())
    } else {
        None
    }
}

fn parse_date_only(date_str: &str) -> Option<i64> {
    if let Ok(naive_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        if let Some(naive_datetime) = naive_date.and_hms_opt(0, 0, 0) {
            return Some(Utc.from_utc_datetime(&naive_datetime).timestamp_millis());
        }
    }
    None
}

fn parse_year_only(date_str: &str) -> Option<i64> {
    if let Ok(year) = date_str.parse::<i32>() {
        if let Some(naive_date) = NaiveDate::from_ymd_opt(year, 1, 1) {
            if let Some(naive_datetime) = naive_date.and_hms_opt(0, 0, 0) {
                return Some(Utc.from_utc_datetime(&naive_datetime).timestamp_millis());
            }
        }
    }
    None
}

fn parse_ymd_date(timestamp_str: &str) -> Option<i64> {
    let parts: Vec<&str> = timestamp_str.split('-').collect();
    if parts.len() != 3 {
        return None;
    }

    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;

    if let Some(naive_date) = NaiveDate::from_ymd_opt(year, month, day) {
        if let Some(naive_datetime) = naive_date.and_hms_opt(0, 0, 0) {
            return Some(Utc.from_utc_datetime(&naive_datetime).timestamp_millis());
        }
    }
    None
}
