use anyhow::{Result, bail};

pub const DEFAULT_WPM: f64 = 175.0;
pub const MIN_RATE_PERCENT: i32 = -100;
pub const MAX_RATE_PERCENT: i32 = 200;
pub const MIN_PITCH_HZ: i32 = -100;
pub const MAX_PITCH_HZ: i32 = 100;
pub const MIN_VOLUME_PERCENT: i32 = -100;
pub const MAX_VOLUME_PERCENT: i32 = 100;

/// Parse speech rate supporting both Edge TTS percentage deltas (`+20%`, `-15%`, `+20`, `-10`)
/// and macOS `say` WPM values (`200`, `120`, where 175 WPM = 0%).
pub fn parse_rate(input: &str) -> Result<i32> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("Invalid rate: value cannot be empty");
    }

    let is_percent_mode =
        trimmed.ends_with('%') || trimmed.starts_with('+') || trimmed.starts_with('-');

    if is_percent_mode {
        let num_str = trimmed.strip_suffix('%').unwrap_or(trimmed);
        if num_str.is_empty() || num_str.contains(char::is_whitespace) {
            bail!(
                "Invalid rate percentage '{input}': expected integer between {MIN_RATE_PERCENT}% and +{MAX_RATE_PERCENT}% (e.g. +20%, -15%)"
            );
        }
        let value: i32 = num_str.parse().map_err(|_| {
            anyhow::anyhow!(
                "Invalid rate percentage '{input}': expected integer between {MIN_RATE_PERCENT}% and +{MAX_RATE_PERCENT}% (e.g. +20%, -15%)"
            )
        })?;
        if !(MIN_RATE_PERCENT..=MAX_RATE_PERCENT).contains(&value) {
            bail!(
                "Rate percentage '{input}' out of range: must be between {MIN_RATE_PERCENT}% and +{MAX_RATE_PERCENT}%"
            );
        }
        Ok(value)
    } else {
        if trimmed.contains(char::is_whitespace) {
            bail!("Invalid WPM rate '{input}': expected a positive number (e.g. 175, 200)");
        }
        let wpm: f64 = trimmed.parse().map_err(|_| {
            anyhow::anyhow!(
                "Invalid rate '{input}': expected WPM (e.g. 200) or percentage delta (e.g. +20%, -15%)"
            )
        })?;
        if !wpm.is_finite() || wpm <= 0.0 {
            bail!("Invalid WPM rate '{input}': words per minute must be a positive number");
        }
        let pct = ((wpm - DEFAULT_WPM) / DEFAULT_WPM * 100.0).round();
        let clamped = pct.clamp(MIN_RATE_PERCENT as f64, MAX_RATE_PERCENT as f64) as i32;
        Ok(clamped)
    }
}

/// Parse pitch adjustment in Hz (e.g. `+0Hz`, `+10Hz`, `-5Hz`, `+10`, `-5`).
pub fn parse_pitch(input: &str) -> Result<i32> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("Invalid pitch: value cannot be empty");
    }

    let num_str = if trimmed.len() >= 2 && trimmed[trimmed.len() - 2..].eq_ignore_ascii_case("hz") {
        &trimmed[..trimmed.len() - 2]
    } else {
        trimmed
    };

    if num_str.is_empty() || num_str.contains(char::is_whitespace) {
        bail!(
            "Invalid pitch '{input}': expected Hz value between {MIN_PITCH_HZ}Hz and +{MAX_PITCH_HZ}Hz (e.g. +10Hz, -5Hz)"
        );
    }

    let value: i32 = num_str.parse().map_err(|_| {
        anyhow::anyhow!(
            "Invalid pitch '{input}': expected Hz value between {MIN_PITCH_HZ}Hz and +{MAX_PITCH_HZ}Hz (e.g. +10Hz, -5Hz)"
        )
    })?;

    if !(MIN_PITCH_HZ..=MAX_PITCH_HZ).contains(&value) {
        bail!(
            "Pitch '{input}' out of range: must be between {MIN_PITCH_HZ}Hz and +{MAX_PITCH_HZ}Hz"
        );
    }

    Ok(value)
}

/// Parse volume adjustment in percent (e.g. `+0%`, `+20%`, `-30%`, `+20`, `-30`).
pub fn parse_volume(input: &str) -> Result<i32> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("Invalid volume: value cannot be empty");
    }

    let num_str = trimmed.strip_suffix('%').unwrap_or(trimmed);
    if num_str.is_empty() || num_str.contains(char::is_whitespace) {
        bail!(
            "Invalid volume '{input}': expected percentage between {MIN_VOLUME_PERCENT}% and +{MAX_VOLUME_PERCENT}% (e.g. +20%, -30%)"
        );
    }

    let value: i32 = num_str.parse().map_err(|_| {
        anyhow::anyhow!(
            "Invalid volume '{input}': expected percentage between {MIN_VOLUME_PERCENT}% and +{MAX_VOLUME_PERCENT}% (e.g. +20%, -30%)"
        )
    })?;

    if !(MIN_VOLUME_PERCENT..=MAX_VOLUME_PERCENT).contains(&value) {
        bail!(
            "Volume '{input}' out of range: must be between {MIN_VOLUME_PERCENT}% and +{MAX_VOLUME_PERCENT}%"
        );
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_percentage_mode() {
        assert_eq!(parse_rate("+0%").unwrap(), 0);
        assert_eq!(parse_rate("0%").unwrap(), 0);
        assert_eq!(parse_rate("+20%").unwrap(), 20);
        assert_eq!(parse_rate("-15%").unwrap(), -15);
        assert_eq!(parse_rate("+20").unwrap(), 20);
        assert_eq!(parse_rate("-10").unwrap(), -10);
        assert_eq!(parse_rate("-100%").unwrap(), -100);
        assert_eq!(parse_rate("+200%").unwrap(), 200);
        assert_eq!(parse_rate("  +25% ").unwrap(), 25);
    }

    #[test]
    fn rate_wpm_mode() {
        assert_eq!(parse_rate("175").unwrap(), 0);
        assert_eq!(parse_rate("210").unwrap(), 20);
        assert_eq!(parse_rate("200").unwrap(), 14);
        assert_eq!(parse_rate("120").unwrap(), -31);
        assert_eq!(parse_rate("350").unwrap(), 100);
        assert_eq!(parse_rate("525").unwrap(), 200);
        assert_eq!(parse_rate("999").unwrap(), 200); // clamped
        assert_eq!(parse_rate("87.5").unwrap(), -50);
    }

    #[test]
    fn rate_invalid_inputs() {
        assert!(parse_rate("").is_err());
        assert!(parse_rate("0").is_err());
        assert!(parse_rate("-101%").is_err());
        assert!(parse_rate("+201%").is_err());
        assert!(parse_rate("abc").is_err());
        assert!(parse_rate("20%%").is_err());
        assert!(parse_rate("NaN").is_err());
        assert!(parse_rate("inf").is_err());
    }

    #[test]
    fn pitch_parsing() {
        assert_eq!(parse_pitch("+0Hz").unwrap(), 0);
        assert_eq!(parse_pitch("+10Hz").unwrap(), 10);
        assert_eq!(parse_pitch("-5hz").unwrap(), -5);
        assert_eq!(parse_pitch("15HZ").unwrap(), 15);
        assert_eq!(parse_pitch("-100Hz").unwrap(), -100);
        assert_eq!(parse_pitch("+100Hz").unwrap(), 100);
        assert_eq!(parse_pitch("0").unwrap(), 0);

        assert!(parse_pitch("").is_err());
        assert!(parse_pitch("+10%").is_err());
        assert!(parse_pitch("+150Hz").is_err());
        assert!(parse_pitch("-150Hz").is_err());
        assert!(parse_pitch("fast").is_err());
    }

    #[test]
    fn volume_parsing() {
        assert_eq!(parse_volume("+0%").unwrap(), 0);
        assert_eq!(parse_volume("+20%").unwrap(), 20);
        assert_eq!(parse_volume("-30%").unwrap(), -30);
        assert_eq!(parse_volume("-100%").unwrap(), -100);
        assert_eq!(parse_volume("+100%").unwrap(), 100);
        assert_eq!(parse_volume("50").unwrap(), 50);

        assert!(parse_volume("").is_err());
        assert!(parse_volume("+10Hz").is_err());
        assert!(parse_volume("+120%").is_err());
        assert!(parse_volume("-120%").is_err());
        assert!(parse_volume("loud").is_err());
    }
}
