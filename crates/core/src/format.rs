//! Human-readable sizes.
//!
//! Uses 1024-based units with the labels Windows Explorer shows (KB, MB, GB, TB),
//! so a number in the app matches the number the user sees in Explorer.

const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

/// Formats a byte count, e.g. `1536` -> `"1.5 KB"`, `0` -> `"0 B"`.
///
/// One decimal under 100 of a unit, none above (`"12.4 GB"`, `"238 GB"`).
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    // Rounding can push 1023.96 KB to "1024.0 KB"; bump to the next unit instead.
    if value >= 1023.95 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value < 100.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn bytes_below_one_kb() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1), "1 B");
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn scales_units() {
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_bytes(12_884_901_888), "12.0 GB");
    }

    #[test]
    fn drops_decimal_above_100() {
        // 238 GiB, the size of the C: drive
        assert_eq!(format_bytes(238 * 1024 * 1024 * 1024), "238 GB");
    }

    #[test]
    fn rounding_rolls_over_to_next_unit() {
        assert_eq!(format_bytes(1024 * 1024 - 1), "1.0 MB");
    }

    #[test]
    fn caps_at_terabytes() {
        assert_eq!(format_bytes(2048 * 1024u64.pow(4)), "2048 TB");
    }
}
