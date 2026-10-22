//! UTC civil time over integer microseconds, shared by the rail, the swimlane graph
//! and the query layer: the calendar math and every stamp the graphs print.
//!
//! No clock and no time zone. Every stamp is the instant's own UTC reading.

use super::model::Micros;

const MICROS_PER_MS: i64 = 1_000;
const MICROS_PER_SEC: i64 = 1_000_000;
const SECS_PER_DAY: i64 = 86_400;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Whole milliseconds at or before `at`: the unit of the warped axis.
pub fn to_ms(at: Micros) -> i64 {
    at.div_euclid(MICROS_PER_MS)
}

/// Days since the epoch of the UTC date containing `at`.
pub fn day_index(at: Micros) -> i64 {
    at.div_euclid(MICROS_PER_SEC * SECS_PER_DAY)
}

/// `HH:MM:SS` of `at`.
pub fn format_clock(at: Micros) -> String {
    let c = civil(at);
    let mut out = String::with_capacity(8);
    push_hm(&mut out, &c);
    out.push(':');
    push_2(&mut out, c.second);
    out
}

/// `HH:MM` of `at`: the rail stamp when every row falls on one UTC date.
pub fn format_time(at: Micros) -> String {
    let mut out = String::with_capacity(5);
    push_hm(&mut out, &civil(at));
    out
}

/// `MM-DD HH:MM` of `at`: the rail stamp when rows span several UTC dates, and
/// the stamp of a collaboration thread step.
pub fn format_day_time(at: Micros) -> String {
    let c = civil(at);
    let mut out = String::with_capacity(11);
    push_2(&mut out, c.month);
    out.push('-');
    push_2(&mut out, c.day);
    out.push(' ');
    push_hm(&mut out, &c);
    out
}

/// `%b %-d %H:%M` of `at`, e.g. `Oct 1 00:10`: the tooltip stamp of a rail row.
pub fn format_tooltip(at: Micros) -> String {
    format_title_stamp(at)
}

/// Epoch days of a civil date (proleptic Gregorian, Hinnant).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((month + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil `(year, month, day)` of an epoch day (Hinnant).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// A UTC instant broken into calendar fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    /// Calendar year.
    pub year: i64,
    /// Month, 1 to 12.
    pub month: u32,
    /// Day of month, 1 to 31.
    pub day: u32,
    /// Hour, 0 to 23.
    pub hour: u32,
    /// Minute, 0 to 59.
    pub minute: u32,
    /// Second, 0 to 59.
    pub second: u32,
    /// Microsecond within the second.
    pub micro: u32,
    /// Day of week, 0 for Sunday.
    pub weekday: u32,
}

/// Breaks `at` into UTC calendar fields.
pub fn civil(at: Micros) -> Civil {
    let secs = at.div_euclid(MICROS_PER_SEC);
    let micro = at.rem_euclid(MICROS_PER_SEC) as u32;
    let days = secs.div_euclid(SECS_PER_DAY);
    let of_day = secs.rem_euclid(SECS_PER_DAY) as u32;
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hour: of_day / 3_600,
        minute: of_day % 3_600 / 60,
        second: of_day % 60,
        micro,
        // 1970-01-01 was a Thursday.
        weekday: (days + 4).rem_euclid(7) as u32,
    }
}

fn push_2(out: &mut String, n: u32) {
    out.push(char::from(b'0' + (n / 10 % 10) as u8));
    out.push(char::from(b'0' + (n % 10) as u8));
}

fn push_day(out: &mut String, day: u32) {
    if day >= 10 {
        out.push(char::from(b'0' + (day / 10) as u8));
    }
    out.push(char::from(b'0' + (day % 10) as u8));
}

fn push_hm(out: &mut String, c: &Civil) {
    push_2(out, c.hour);
    out.push(':');
    push_2(out, c.minute);
}

fn month_name(c: &Civil) -> &'static str {
    MONTHS.get(c.month as usize - 1).copied().unwrap_or("")
}

/// Appends `%b %-d %H:%M` of `at`, e.g. `Oct 1 00:10`.
pub fn push_title_stamp(out: &mut String, at: Micros) {
    let c = civil(at);
    out.push_str(month_name(&c));
    out.push(' ');
    push_day(out, c.day);
    out.push(' ');
    push_hm(out, &c);
}

/// `%b %-d %H:%M`, e.g. `Oct 1 00:10`: the tooltip stamp of a dot.
pub fn format_title_stamp(at: Micros) -> String {
    let mut out = String::with_capacity(12);
    push_title_stamp(&mut out, at);
    out
}

/// `%a %-d %b` of an instant in epoch milliseconds, e.g. `Fri 2 Oct`: a day tick.
pub fn format_day_label(ms: i64) -> String {
    let c = civil(ms.saturating_mul(MICROS_PER_MS));
    let mut out = String::with_capacity(10);
    out.push_str(WEEKDAYS.get(c.weekday as usize).copied().unwrap_or(""));
    out.push(' ');
    push_day(&mut out, c.day);
    out.push(' ');
    out.push_str(month_name(&c));
    out
}

/// `%H:%M` of an instant in epoch milliseconds, e.g. `08:01`: a time tick.
pub fn format_clock_label(ms: i64) -> String {
    let c = civil(ms.saturating_mul(MICROS_PER_MS));
    let mut out = String::with_capacity(5);
    push_hm(&mut out, &c);
    out
}

/// `%m-%d %H:%M`, e.g. `10-02 06:00`: a collaboration thread step.
pub fn format_step_time(at: Micros) -> String {
    format_day_time(at)
}

/// ISO 8601 UTC, `2026-10-01T00:00:00Z`. A six-digit fraction appears only
/// when the microseconds are non-zero.
pub fn format_iso8601(at: Micros) -> String {
    let c = civil(at);
    let mut out = String::with_capacity(27);
    out.push_str(&format!("{:04}", c.year));
    out.push('-');
    push_2(&mut out, c.month);
    out.push('-');
    push_2(&mut out, c.day);
    out.push('T');
    push_hm(&mut out, &c);
    out.push(':');
    push_2(&mut out, c.second);
    if c.micro != 0 {
        out.push_str(&format!(".{:06}", c.micro));
    }
    out.push('Z');
    out
}

/// UTC instant of calendar fields; fields are taken as given.
pub fn from_utc(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Micros {
    let days = days_from_civil(year, month, day);
    let secs =
        days * SECS_PER_DAY + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second);
    secs * MICROS_PER_SEC
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0, (1970, 1, 1))]
    #[case(-1, (1969, 12, 31))]
    #[case(11_017, (2000, 3, 1))]
    #[case(20_362, (2025, 10, 1))]
    #[case(20_727, (2026, 10, 1))]
    #[case(19_782, (2024, 2, 29))]
    fn civil_from_days_known_dates(#[case] days: i64, #[case] want: (i64, u32, u32)) {
        assert_eq!(civil_from_days(days), want);
        assert_eq!(days_from_civil(want.0, want.1, want.2), days);
    }

    #[rstest]
    #[case(from_utc(2026, 10, 1, 0, 0, 0), "Oct 1 00:00")]
    #[case(from_utc(2026, 10, 1, 0, 10, 0), "Oct 1 00:10")]
    #[case(from_utc(2026, 9, 28, 14, 5, 59), "Sep 28 14:05")]
    #[case(from_utc(2026, 12, 31, 23, 59, 59), "Dec 31 23:59")]
    fn title_stamp_has_no_day_padding(#[case] at: Micros, #[case] want: &str) {
        assert_eq!(format_title_stamp(at), want);
    }

    #[rstest]
    #[case(from_utc(2026, 10, 2, 0, 0, 0), "Fri 2 Oct")]
    #[case(from_utc(2026, 10, 1, 0, 0, 0), "Thu 1 Oct")]
    #[case(from_utc(2026, 9, 8, 8, 0, 0), "Tue 8 Sep")]
    #[case(from_utc(1970, 1, 1, 0, 0, 0), "Thu 1 Jan")]
    #[case(from_utc(2026, 10, 4, 9, 0, 0), "Sun 4 Oct")]
    fn day_label_names_weekday_day_month(#[case] at: Micros, #[case] want: &str) {
        assert_eq!(format_day_label(to_ms(at)), want);
    }

    #[rstest]
    #[case(from_utc(2026, 10, 1, 8, 1, 0), "08:01")]
    #[case(from_utc(2026, 10, 1, 0, 0, 0), "00:00")]
    #[case(from_utc(2026, 10, 1, 23, 59, 0), "23:59")]
    fn clock_label_is_zero_padded(#[case] at: Micros, #[case] want: &str) {
        assert_eq!(format_clock_label(to_ms(at)), want);
    }

    #[test]
    fn step_time_pads_month_and_day() {
        assert_eq!(
            format_step_time(from_utc(2026, 10, 2, 6, 0, 0)),
            "10-02 06:00"
        );
        assert_eq!(
            format_step_time(from_utc(2026, 1, 9, 23, 5, 0)),
            "01-09 23:05"
        );
    }

    #[test]
    fn iso8601_shows_fraction_only_when_present() {
        let at = from_utc(2026, 10, 1, 0, 0, 0);
        assert_eq!(format_iso8601(at), "2026-10-01T00:00:00Z");
        assert_eq!(format_iso8601(at + 5), "2026-10-01T00:00:00.000005Z");
        assert_eq!(format_iso8601(at + 123_456), "2026-10-01T00:00:00.123456Z");
    }

    #[test]
    fn to_ms_floors_toward_negative_infinity() {
        assert_eq!(to_ms(1_999), 1);
        assert_eq!(to_ms(-1), -1);
        assert_eq!(to_ms(-1_000), -1);
        assert_eq!(to_ms(-1_001), -2);
    }

    proptest! {
        #[test]
        fn civil_round_trips(days in -800_000i64..800_000) {
            let (y, m, d) = civil_from_days(days);
            prop_assert_eq!(days_from_civil(y, m, d), days);
            prop_assert!((1..=12).contains(&m) && (1..=31).contains(&d));
        }

        #[test]
        fn civil_fields_in_range(at in -4_000_000_000_000_000i64..4_000_000_000_000_000) {
            let c = civil(at);
            prop_assert!(c.hour < 24 && c.minute < 60 && c.second < 60 && c.micro < 1_000_000);
            prop_assert!(c.weekday < 7);
            prop_assert_eq!(from_utc(c.year, c.month, c.day, c.hour, c.minute, c.second) + i64::from(c.micro), at);
        }
    }

    #[rstest]
    #[case(0, "1970-01-01T00:00:00Z")]
    #[case(from_utc(2024, 2, 29, 23, 59, 59) + 5, "2024-02-29T23:59:59.000005Z")]
    #[case(-1_000_000, "1969-12-31T23:59:59Z")]
    #[case(from_utc(2000, 3, 1, 0, 0, 0), "2000-03-01T00:00:00Z")]
    fn iso_formats_edge_instants(#[case] at: Micros, #[case] want: &str) {
        assert_eq!(format_iso8601(at), want);
    }

    #[test]
    fn rail_stamp_formats() {
        let at = from_utc(2026, 10, 2, 6, 0, 0);
        assert_eq!(format_time(at), "06:00");
        assert_eq!(format_day_time(at), "10-02 06:00");
        assert_eq!(format_clock(at + 7_000_000), "06:00:07");
        assert_eq!(
            format_tooltip(from_utc(2026, 10, 1, 0, 10, 0)),
            "Oct 1 00:10"
        );
        assert_eq!(
            format_tooltip(from_utc(2026, 12, 25, 13, 5, 0)),
            "Dec 25 13:05"
        );
    }

    #[test]
    fn civil_round_trips_across_leap_and_century_years() {
        for (y, m, d) in [
            (1999, 12, 31),
            (2000, 2, 29),
            (2100, 3, 1),
            (1969, 7, 20),
            (2026, 10, 20),
        ] {
            let c = civil(from_utc(y, m, d, 12, 34, 56));
            assert_eq!(
                (c.year, c.month, c.day, c.hour, c.minute, c.second),
                (y, m, d, 12, 34, 56)
            );
        }
    }

    #[test]
    fn day_index_floors_negative_instants() {
        let day = MICROS_PER_SEC * SECS_PER_DAY;
        assert_eq!(day_index(-1), -1);
        assert_eq!(day_index(0), 0);
        assert_eq!(day_index(day - 1), 0);
        assert_eq!(day_index(day), 1);
    }
}
