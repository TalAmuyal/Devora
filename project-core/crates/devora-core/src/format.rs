//! How Devora shows values to the user, the same in every UI.

/// Devora's shorthand for a duration: whole seconds, in its two largest units, the second not padded ("45s", "3m 5s",
/// "1h 4m", "2d 3h"); a smaller unit is dropped, not rounded, so a running timer never shows a time it hasn't reached.
pub fn duration(seconds: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    match seconds {
        s if s < MINUTE => format!("{s}s"),
        s if s < HOUR => format!("{}m {}s", s / MINUTE, s % MINUTE),
        s if s < DAY => format!("{}h {}m", s / HOUR, s % HOUR / MINUTE),
        s => format!("{}d {}h", s / DAY, s % DAY / HOUR),
    }
}

#[cfg(test)]
mod tests {
    use super::duration;

    #[test]
    fn a_duration_shows_its_two_largest_units() {
        for (seconds, shown) in [
            (0, "0s"),
            (59, "59s"),
            (60, "1m 0s"),
            (185, "3m 5s"),
            (3599, "59m 59s"),
            (3600, "1h 0m"),
            (3_840 + 59, "1h 4m"),
            (86_399, "23h 59m"),
            (86_400, "1d 0h"),
            (2 * 86_400 + 3 * 3600 + 59 * 60, "2d 3h"),
        ] {
            assert_eq!(duration(seconds), shown, "{seconds} s");
        }
    }
}
