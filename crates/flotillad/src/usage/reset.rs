//! When a runtime's rate limit resets, read from its words: an epoch after
//! a bar ("…limit reached|1760000000"), or "resets 3pm", "resets 3:30pm
//! (Europe/Rome)", "resets Oct 24, 3pm".

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};

/// The reset time `text` gives, read relative to `at`, when the message
/// was written.
pub fn parse(text: &str, at: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if let Some(epoch) = after_bar(text) {
        return DateTime::from_timestamp(epoch, 0);
    }
    let lower = text.to_ascii_lowercase();
    let start = lower.find("resets")? + "resets".len();
    let rest = lower[start..].trim_start();
    let rest = rest.strip_prefix("at ").unwrap_or(rest);
    let zone = zone(rest);
    let (date, rest) = month_day(rest);
    let time = clock(rest)?;
    let local = |date: NaiveDate| -> Option<DateTime<Utc>> {
        let naive = date.and_time(time);
        match zone {
            Some(tz) => tz
                .from_local_datetime(&naive)
                .earliest()
                .map(|t| t.with_timezone(&Utc)),
            None => Local
                .from_local_datetime(&naive)
                .earliest()
                .map(|t| t.with_timezone(&Utc)),
        }
    };
    let today = match zone {
        Some(tz) => at.with_timezone(&tz).date_naive(),
        None => at.with_timezone(&Local).date_naive(),
    };
    match date {
        Some((month, day)) => {
            let this_year = NaiveDate::from_ymd_opt(today.year(), month, day)?;
            let when = local(this_year)?;
            if when + Duration::days(1) < at {
                local(NaiveDate::from_ymd_opt(today.year() + 1, month, day)?)
            } else {
                Some(when)
            }
        }
        None => {
            let when = local(today)?;
            if when <= at {
                local(today.succ_opt()?)
            } else {
                Some(when)
            }
        }
    }
}

fn after_bar(text: &str) -> Option<i64> {
    let (_, tail) = text.rsplit_once('|')?;
    let digits: String = tail
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (digits.len() >= 9).then(|| digits.parse().ok()).flatten()
}

/// A zone named in parentheses, "(Europe/Rome)".
fn zone(rest: &str) -> Option<chrono_tz::Tz> {
    let open = rest.find('(')?;
    let close = rest[open..].find(')')? + open;
    // Zone names are case-sensitive; `rest` is lowercased, so match by hand.
    let name = &rest[open + 1..close];
    chrono_tz::TZ_VARIANTS
        .iter()
        .find(|tz| tz.name().eq_ignore_ascii_case(name))
        .copied()
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// "oct 24, 3pm" → ((10, 24), "3pm").
fn month_day(rest: &str) -> (Option<(u32, u32)>, &str) {
    let Some(month) = MONTHS.iter().position(|m| rest.starts_with(m)) else {
        return (None, rest);
    };
    let after = rest
        .trim_start_matches(|c: char| c.is_ascii_alphabetic())
        .trim_start();
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    let Ok(day) = digits.parse::<u32>() else {
        return (None, rest);
    };
    let tail = after[digits.len()..].trim_start_matches([',', ' ']);
    let tail = tail.strip_prefix("at ").unwrap_or(tail);
    (Some((month as u32 + 1, day)), tail)
}

/// "3pm", "3:30pm", "15:00", "3 pm".
fn clock(rest: &str) -> Option<NaiveTime> {
    let digits: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':')
        .collect();
    if digits.is_empty() {
        return None;
    }
    let (h, m) = match digits.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (digits.parse::<u32>().ok()?, 0),
    };
    let suffix = rest[digits.len()..].trim_start();
    let h = if suffix.starts_with("pm") && h < 12 {
        h + 12
    } else if suffix.starts_with("am") && h == 12 {
        0
    } else {
        h
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn reads_an_epoch_after_a_bar() {
        let reset = parse("Claude AI usage limit reached|1760000000", Utc::now());
        assert_eq!(reset.map(|t| t.timestamp()), Some(1_760_000_000));
    }

    #[test]
    fn reads_a_time_in_a_named_zone() {
        let now = at("2026-10-21T10:00:00Z");
        let reset = parse("You've hit your limit · resets 6pm (Europe/Rome)", now).unwrap();
        // 18:00 in Rome on 21 October (CEST, +2) is 16:00 UTC.
        assert_eq!(reset, at("2026-10-21T16:00:00Z"));
        let reset = parse("5-hour limit reached ∙ resets 3:30am (UTC)", now).unwrap();
        assert_eq!(reset, at("2026-10-22T03:30:00Z"), "the next 3:30am");
    }

    #[test]
    fn reads_a_date() {
        let now = at("2026-10-21T10:00:00Z");
        let reset = parse("Weekly limit reached ∙ resets Oct 24, 3pm (UTC)", now).unwrap();
        assert_eq!(reset, at("2026-10-24T15:00:00Z"));
    }

    #[test]
    fn no_time_no_reset() {
        assert_eq!(parse("API Error: 500", Utc::now()), None);
        assert_eq!(parse("limit reached", Utc::now()), None);
    }
}
