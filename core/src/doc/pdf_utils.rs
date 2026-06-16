//! PDF-only helpers for the document module.

/// Parse a hex color string like "#RRGGBB" or "#RRGGBBAA" into (r, g, b, a).
pub(super) fn parse_hex_color(s: &str) -> Result<(u8, u8, u8, u8), String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    match s.len() {
        6 => Ok((
            parse_hex_byte(s, 0)?,
            parse_hex_byte(s, 2)?,
            parse_hex_byte(s, 4)?,
            255,
        )),
        8 => Ok((
            parse_hex_byte(s, 0)?,
            parse_hex_byte(s, 2)?,
            parse_hex_byte(s, 4)?,
            parse_hex_byte(s, 6)?,
        )),
        _ => Err(format!(
            "invalid color format '{}' (expected #RRGGBB or #RRGGBBAA)",
            s
        )),
    }
}

fn parse_hex_byte(s: &str, start: usize) -> Result<u8, String> {
    u8::from_str_radix(&s[start..start + 2], 16).map_err(|_| format!("invalid color: #{}", s))
}
