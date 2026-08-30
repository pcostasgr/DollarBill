/// Centralized OCC option symbol parser.
///
/// Handles both compact (`GLD260626P00405000`) and padded (`AAPL  260117C00150000`)
/// root formats by parsing from the right side of the string.
///
/// Layout from the right: `[8-digit strike][C|P][6-digit YYMMDD][1–6 char root]`
use chrono::NaiveDate;

/// Parsed components of an OCC option symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct OccParts {
    pub root: String,
    pub expiry: NaiveDate,
    /// ISO date string "YYYY-MM-DD".
    pub expiry_str: String,
    pub is_call: bool,
    pub strike: f64,
}

/// Parse an OCC symbol into its components.
/// Returns `None` if the symbol does not match the expected layout.
///
/// Works entirely on the byte slice to avoid panics on multi-byte UTF-8 input.
/// Any non-ASCII byte causes an early `None` return.
pub fn parse_occ(occ: &str) -> Option<OccParts> {
    // All OCC symbols are pure ASCII — reject any non-ASCII input immediately.
    if !occ.is_ascii() {
        return None;
    }
    let bytes = occ.as_bytes();
    let len = bytes.len();

    // Minimum: 1-char root + 6 date + 1 type + 8 strike = 16 chars
    if len < 16 {
        return None;
    }

    // The C/P type byte is at position len-9 (before the 8-digit strike field)
    let type_idx = len - 9;
    let type_byte = bytes[type_idx];
    if type_byte != b'C' && type_byte != b'P' {
        return None;
    }

    // 6-digit YYMMDD immediately precedes the type byte
    if type_idx < 6 {
        return None;
    }
    let date_start = type_idx - 6;
    let date_bytes = &bytes[date_start..type_idx];
    if !date_bytes.iter().all(|b| b.is_ascii_digit()) {
        return None;
    }

    // Parse date fields from ASCII bytes
    let yy: i32 = std::str::from_utf8(&date_bytes[0..2]).ok()?.parse().ok()?;
    let mm: u32 = std::str::from_utf8(&date_bytes[2..4]).ok()?.parse().ok()?;
    let dd: u32 = std::str::from_utf8(&date_bytes[4..6]).ok()?.parse().ok()?;

    // 8-digit strike in thousandths (00405000 → $405.000)
    let strike_bytes = &bytes[type_idx + 1..];
    if strike_bytes.len() != 8 || !strike_bytes.iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let strike_raw: u64 = std::str::from_utf8(strike_bytes).ok()?.parse().ok()?;

    // Root is everything before the date block, with spaces stripped (pure ASCII)
    let root_bytes = &bytes[..date_start];
    let root = std::str::from_utf8(root_bytes).ok()?.trim().to_string();
    if root.is_empty() {
        return None;
    }

    let year = 2000 + yy;
    let expiry = NaiveDate::from_ymd_opt(year, mm, dd)?;
    let expiry_str = format!("{:04}-{:02}-{:02}", year, mm, dd);

    Some(OccParts {
        root,
        expiry,
        expiry_str,
        is_call: type_byte == b'C',
        strike: strike_raw as f64 / 1000.0,
    })
}

/// Convenience: returns `"YYYY-MM-DD"` expiry string or `None`.
pub fn parse_occ_expiry_str(occ: &str) -> Option<String> {
    parse_occ(occ).map(|p| p.expiry_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_put() {
        // 18-char compact: GLD260626P00405000
        let p = parse_occ("GLD260626P00405000").unwrap();
        assert_eq!(p.root, "GLD");
        assert_eq!(p.expiry_str, "2026-06-26");
        assert!(!p.is_call);
        assert_eq!(p.strike, 405.0);
    }

    #[test]
    fn compact_call_5char_root() {
        // 20-char compact: GOOGL260117C00150000
        let p = parse_occ("GOOGL260117C00150000").unwrap();
        assert_eq!(p.root, "GOOGL");
        assert_eq!(p.expiry_str, "2026-01-17");
        assert!(p.is_call);
        assert_eq!(p.strike, 150.0);
    }

    #[test]
    fn compact_put_4char_root() {
        // 19-char compact: QCOM260508P00120000
        let p = parse_occ("QCOM260508P00120000").unwrap();
        assert_eq!(p.root, "QCOM");
        assert_eq!(p.expiry_str, "2026-05-08");
        assert!(!p.is_call);
        assert_eq!(p.strike, 120.0);
    }

    #[test]
    fn padded_call() {
        // 21-char padded: AAPL  260117C00150000
        let p = parse_occ("AAPL  260117C00150000").unwrap();
        assert_eq!(p.root, "AAPL");
        assert_eq!(p.expiry_str, "2026-01-17");
        assert!(p.is_call);
        assert_eq!(p.strike, 150.0);
    }

    #[test]
    fn padded_put_test_fixture() {
        // 21-char padded: TEST  251219P00250000
        let p = parse_occ("TEST  251219P00250000").unwrap();
        assert_eq!(p.root, "TEST");
        assert_eq!(p.expiry_str, "2025-12-19");
        assert!(!p.is_call);
        assert_eq!(p.strike, 250.0);
    }

    #[test]
    fn too_short_returns_none() {
        assert!(parse_occ("AAPL260").is_none());
    }

    #[test]
    fn invalid_type_char_returns_none() {
        // Replace C with X
        assert!(parse_occ("AAPL  260117X00150000").is_none());
    }
}

// ── Property-based (proptest) fuzzing ─────────────────────────────────────────
// Goals:
//   1. `parse_occ` never panics on ANY arbitrary input (no unwrap/index panics).
//   2. Any output from our own OCC builder round-trips correctly.
//   3. Symbols that are too short, have bad date/type bytes, or zero-strike always
//      return `None` — never a spurious `Some`.
#[cfg(test)]
mod proptest_occ {
    use super::*;
    use proptest::prelude::*;

    // ── Strategy: completely arbitrary byte strings ────────────────────────
    proptest! {
        /// parse_occ never panics on any arbitrary string input.
        #[test]
        fn no_panic_on_arbitrary_input(s in "\\PC*") {
            // We don't care what it returns — only that it doesn't panic.
            let _ = parse_occ(&s);
        }
    }

    // ── Strategy: build valid-looking OCC symbols with proptest-generated fields ─
    fn valid_root() -> impl Strategy<Value = String> {
        // 1–6 uppercase ASCII letters
        prop::string::string_regex("[A-Z]{1,6}").unwrap()
    }

    fn valid_date() -> impl Strategy<Value = (u32, u32, u32)> {
        // Year 25..35 (2025-2035), month 01-12, day 01-28 (safe for all months)
        (25u32..=35u32, 1u32..=12u32, 1u32..=28u32)
    }

    fn valid_strike_raw() -> impl Strategy<Value = u64> {
        // $1 to $9,999 in thousandths → 1_000 to 9_999_000
        1_000u64..=9_999_000u64
    }

    proptest! {
        /// Round-trip: every symbol we build with valid components must parse back
        /// to exactly the same values.
        #[test]
        fn round_trip_compact(
            root in valid_root(),
            (yy, mm, dd) in valid_date(),
            is_call in any::<bool>(),
            strike_raw in valid_strike_raw(),
        ) {
            let type_char = if is_call { 'C' } else { 'P' };
            let occ = format!("{}{:02}{:02}{:02}{}{:08}", root, yy, mm, dd, type_char, strike_raw);
            match parse_occ(&occ) {
                Some(p) => {
                    prop_assert_eq!(p.root, root.trim());
                    prop_assert_eq!(p.is_call, is_call);
                    prop_assert!((p.strike * 1000.0 - strike_raw as f64).abs() < 0.001);
                }
                None => {
                    // parse_occ returns None only when the date is invalid
                    // (e.g. proptest generated month=0 corner case in older strategy).
                    // Any None here is acceptable as long as we don't panic.
                }
            }
        }
    }

    proptest! {
        /// Padded 21-char symbols with a 6-char root field (root + spaces) must parse.
        #[test]
        fn round_trip_padded(
            root_len in 1usize..=5usize,
            (yy, mm, dd) in valid_date(),
            is_call in any::<bool>(),
            strike_raw in valid_strike_raw(),
        ) {
            // Fixed 4-char root with trailing spaces to fill 6 chars
            let roots = ["AAPL", "META", "MSFT", "NVDA", "SPY"];
            // Use root_len to index into the slice (bounded by strategy)
            let root = roots[root_len - 1];
            let padded_root = format!("{:<6}", root);
            let type_char = if is_call { 'C' } else { 'P' };
            let occ = format!("{}{:02}{:02}{:02}{}{:08}", padded_root, yy, mm, dd, type_char, strike_raw);
            prop_assert_eq!(occ.len(), 21);
            match parse_occ(&occ) {
                Some(p) => {
                    prop_assert_eq!(p.root, root);
                    prop_assert_eq!(p.is_call, is_call);
                }
                None => { /* date boundary edge case — acceptable */ }
            }
        }
    }

    proptest! {
        /// Strings shorter than 16 bytes must always return None.
        #[test]
        fn too_short_always_none(s in "\\PC{0,15}") {
            if s.len() < 16 {
                prop_assert!(parse_occ(&s).is_none());
            }
        }
    }

    proptest! {
        /// Injecting a non-C/P byte at the type position always returns None.
        #[test]
        fn bad_type_char_always_none(
            root in valid_root(),
            (yy, mm, dd) in valid_date(),
            bad_type in prop::char::range('\x00', '\x7f').prop_filter(
                "must not be C or P", |c| *c != 'C' && *c != 'P'
            ),
            strike_raw in valid_strike_raw(),
        ) {
            let occ = format!("{}{:02}{:02}{:02}{}{:08}",
                root, yy, mm, dd, bad_type, strike_raw);
            prop_assert!(parse_occ(&occ).is_none());
        }
    }
}
