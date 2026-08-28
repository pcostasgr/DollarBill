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
pub fn parse_occ(occ: &str) -> Option<OccParts> {
    // Minimum: 1-char root + 6 date + 1 type + 8 strike = 16 chars
    if occ.len() < 16 {
        return None;
    }
    let len = occ.len();

    // The C/P type byte is at position len-9 (before the 8-digit strike field)
    let type_idx = len - 9;
    let type_char = occ.as_bytes()[type_idx] as char;
    if type_char != 'C' && type_char != 'P' {
        return None;
    }

    // 6-digit YYMMDD immediately precedes the type byte
    if type_idx < 6 {
        return None;
    }
    let date_start = type_idx - 6;
    let date_bytes = &occ[date_start..type_idx];
    if !date_bytes.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let yy: i32 = date_bytes[0..2].parse().ok()?;
    let mm: u32 = date_bytes[2..4].parse().ok()?;
    let dd: u32 = date_bytes[4..6].parse().ok()?;

    // 8-digit strike in thousandths (00405000 → $405.000)
    let strike_str = &occ[type_idx + 1..];
    if strike_str.len() != 8 || !strike_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let strike_raw: u64 = strike_str.parse().ok()?;

    // Root is everything before the date block, with spaces stripped
    let root = occ[..date_start].trim().to_string();
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
        is_call: type_char == 'C',
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
