use anyhow::{Context, Result, bail};
use msedge_tts::tts::SpeechConfig;
use msedge_tts::voice::{Voice, get_voices_list};

pub const DEFAULT_VOICE: &str = "zh-TW-HsiaoChenNeural";

/// Fetch voices from Microsoft Edge TTS and optionally filter by locale prefix (case-insensitive).
pub fn list_voices(locale_filter: Option<&str>) -> Result<Vec<Voice>> {
    let voices = get_voices_list()
        .context("Failed to fetch available voices from Microsoft Edge TTS service")?;
    Ok(filter_and_sort_voices(voices, locale_filter))
}

pub fn filter_and_sort_voices(voices: Vec<Voice>, locale_filter: Option<&str>) -> Vec<Voice> {
    let filter_lower = locale_filter
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase);

    let mut filtered: Vec<Voice> = voices
        .into_iter()
        .filter(|voice| match &filter_lower {
            None => true,
            Some(prefix) => voice
                .locale
                .as_deref()
                .or(voice.short_name.as_deref())
                .map(|loc| loc.to_ascii_lowercase().starts_with(prefix))
                .unwrap_or(false),
        })
        .collect();

    filtered.sort_by(|a, b| {
        let a_key = (
            a.locale.as_deref().unwrap_or(""),
            a.short_name.as_deref().unwrap_or(&a.name),
        );
        let b_key = (
            b.locale.as_deref().unwrap_or(""),
            b.short_name.as_deref().unwrap_or(&b.name),
        );
        a_key.cmp(&b_key)
    });

    filtered
}

pub fn format_voices_table(voices: &[Voice], locale_filter: Option<&str>) -> String {
    if voices.is_empty() {
        return match locale_filter {
            Some(loc) => format!("No voices found matching locale '{loc}'.\n"),
            None => "No voices available.\n".to_owned(),
        };
    }

    let short_width = voices
        .iter()
        .map(|v| v.short_name.as_deref().unwrap_or(&v.name).len())
        .max()
        .unwrap_or(9)
        .max("ShortName".len());
    let locale_width = voices
        .iter()
        .map(|v| v.locale.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(6)
        .max("Locale".len());
    let gender_width = voices
        .iter()
        .map(|v| v.gender.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(6)
        .max("Gender".len());

    let mut out = String::new();
    out.push_str(&format!(
        "{:<short_width$}  {:<locale_width$}  {:<gender_width$}  {}\n",
        "ShortName", "Locale", "Gender", "FriendlyName"
    ));
    out.push_str(&format!(
        "{}  {}  {}  {}\n",
        "-".repeat(short_width),
        "-".repeat(locale_width),
        "-".repeat(gender_width),
        "-".repeat(12)
    ));

    for voice in voices {
        let short = voice.short_name.as_deref().unwrap_or(&voice.name);
        let locale = voice.locale.as_deref().unwrap_or("-");
        let gender = voice.gender.as_deref().unwrap_or("-");
        let friendly = voice.friendly_name.as_deref().unwrap_or(&voice.name);
        out.push_str(&format!(
            "{short:<short_width$}  {locale:<locale_width$}  {gender:<gender_width$}  {friendly}\n"
        ));
    }

    out
}

/// Attempt to build the Edge TTS voice `Name` directly without a `get_voices_list()` network call
/// when the caller passes an exact `ShortName` (e.g. `zh-TW-HsiaoChenNeural`) or full Edge `Name`.
pub fn try_build_direct_voice_name(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("Microsoft Server Speech Text to Speech Voice (")
        && trimmed.ends_with(')')
    {
        return Some(trimmed.to_owned());
    }

    if trimmed.chars().any(char::is_whitespace) {
        return None;
    }

    let (locale_part, voice_part) = trimmed.rsplit_once('-')?;
    if !voice_part.ends_with("Neural")
        || !voice_part
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
        || !voice_part.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return None;
    }

    let mut locale_segments = locale_part.split('-');
    let lang = locale_segments.next()?;
    if !(2..=3).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return None;
    }

    let mut has_region = false;
    for seg in locale_segments {
        if seg.is_empty() || !seg.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        has_region = true;
    }
    if !has_region {
        return None;
    }

    Some(format!(
        "Microsoft Server Speech Text to Speech Voice ({locale_part}, {voice_part})"
    ))
}

/// Match a voice query against a fetched voice list:
/// 1. Exact case-insensitive match on `ShortName` or `Name`
/// 2. Unique case-insensitive substring match on `ShortName`
/// 3. Unique case-insensitive substring match on `FriendlyName` / `Name`
pub fn match_voice<'a>(voices: &'a [Voice], query: &str) -> Result<&'a Voice> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        bail!("Voice name cannot be empty. Run `say -v '?'` to list available voices.");
    }
    let query_lower = trimmed.to_ascii_lowercase();

    // 1. Exact case-insensitive match on ShortName or full Name.
    if let Some(exact) = voices.iter().find(|v| {
        v.short_name
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case(trimmed))
            || v.name.eq_ignore_ascii_case(trimmed)
    }) {
        return Ok(exact);
    }

    // 2. Substring match on ShortName.
    let short_matches: Vec<&Voice> = voices
        .iter()
        .filter(|v| {
            v.short_name
                .as_deref()
                .is_some_and(|s| s.to_ascii_lowercase().contains(&query_lower))
        })
        .collect();

    if short_matches.len() == 1 {
        return Ok(short_matches[0]);
    }
    if short_matches.len() > 1 {
        return Err(ambiguous_voice_error(trimmed, &short_matches));
    }

    // 3. Fallback substring match on FriendlyName or Name.
    let name_matches: Vec<&Voice> = voices
        .iter()
        .filter(|v| {
            v.friendly_name
                .as_deref()
                .is_some_and(|f| f.to_ascii_lowercase().contains(&query_lower))
                || v.name.to_ascii_lowercase().contains(&query_lower)
        })
        .collect();

    match name_matches.len() {
        1 => Ok(name_matches[0]),
        n if n > 1 => Err(ambiguous_voice_error(trimmed, &name_matches)),
        _ => bail!(
            "Voice '{trimmed}' not found. Run `say -v '?'` or `say --list-voices` to list available voices."
        ),
    }
}

fn ambiguous_voice_error(query: &str, candidates: &[&Voice]) -> anyhow::Error {
    let mut msg = format!(
        "Voice '{query}' matched {} voices. Please specify a more specific name:\n",
        candidates.len()
    );
    for voice in candidates.iter().take(20) {
        let short = voice.short_name.as_deref().unwrap_or(&voice.name);
        let locale = voice.locale.as_deref().unwrap_or("-");
        let gender = voice.gender.as_deref().unwrap_or("-");
        msg.push_str(&format!("  - {short} ({locale}, {gender})\n"));
    }
    if candidates.len() > 20 {
        msg.push_str(&format!("  ... and {} more\n", candidates.len() - 20));
    }
    anyhow::anyhow!(msg.trim_end().to_owned())
}

/// Resolve a voice query into an `msedge_tts::tts::SpeechConfig`.
pub fn resolve_voice(
    query: &str,
    audio_format: &str,
    rate: i32,
    pitch: i32,
    volume: i32,
) -> Result<SpeechConfig> {
    if let Some(direct_name) = try_build_direct_voice_name(query) {
        return Ok(SpeechConfig {
            voice_name: direct_name,
            audio_format: audio_format.to_owned(),
            pitch,
            rate,
            volume,
        });
    }

    let voices = get_voices_list()
        .context("Failed to fetch available voices from Microsoft Edge TTS service")?;
    let matched = match_voice(&voices, query)?;
    let mut config = SpeechConfig::from(matched);
    config.audio_format = audio_format.to_owned();
    config.rate = rate;
    config.pitch = pitch;
    config.volume = volume;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_voice(short: &str, locale: &str, gender: &str, friendly: &str) -> Voice {
        let voice_part = short.rsplit_once('-').map(|(_, v)| v).unwrap_or(short);
        Voice {
            name: format!("Microsoft Server Speech Text to Speech Voice ({locale}, {voice_part})"),
            short_name: Some(short.to_owned()),
            gender: Some(gender.to_owned()),
            locale: Some(locale.to_owned()),
            suggested_codec: Some("audio-24khz-48kbitrate-mono-mp3".to_owned()),
            friendly_name: Some(friendly.to_owned()),
            status: Some("GA".to_owned()),
            voice_tag: None,
        }
    }

    fn sample_voices() -> Vec<Voice> {
        vec![
            sample_voice(
                "zh-TW-HsiaoChenNeural",
                "zh-TW",
                "Female",
                "Microsoft HsiaoChen Online (Natural) - Chinese (Taiwan)",
            ),
            sample_voice(
                "zh-TW-YunJheNeural",
                "zh-TW",
                "Male",
                "Microsoft YunJhe Online (Natural) - Chinese (Taiwan)",
            ),
            sample_voice(
                "zh-TW-HsiaoYuNeural",
                "zh-TW",
                "Female",
                "Microsoft HsiaoYu Online (Natural) - Chinese (Taiwan)",
            ),
            sample_voice(
                "en-US-AriaNeural",
                "en-US",
                "Female",
                "Microsoft Aria Online (Natural) - English (United States)",
            ),
        ]
    }

    #[test]
    fn direct_voice_name_fast_path() {
        assert_eq!(
            try_build_direct_voice_name("zh-TW-HsiaoChenNeural").as_deref(),
            Some("Microsoft Server Speech Text to Speech Voice (zh-TW, HsiaoChenNeural)")
        );
        assert_eq!(
            try_build_direct_voice_name("zh-CN-liaoning-XiaobeiNeural").as_deref(),
            Some("Microsoft Server Speech Text to Speech Voice (zh-CN-liaoning, XiaobeiNeural)")
        );
        assert_eq!(
            try_build_direct_voice_name("sr-Latn-RS-NicholasNeural").as_deref(),
            Some("Microsoft Server Speech Text to Speech Voice (sr-Latn-RS, NicholasNeural)")
        );
        assert_eq!(try_build_direct_voice_name("YunJhe"), None);
        assert_eq!(try_build_direct_voice_name("zh-tw-hsiaochenneural"), None);
    }

    #[test]
    fn matches_exact_and_unique_substring_case_insensitively() {
        let voices = sample_voices();
        let m1 = match_voice(&voices, "zh-tw-hsiaochenneural").unwrap();
        assert_eq!(m1.short_name.as_deref(), Some("zh-TW-HsiaoChenNeural"));

        let m2 = match_voice(&voices, "YunJhe").unwrap();
        assert_eq!(m2.short_name.as_deref(), Some("zh-TW-YunJheNeural"));

        let m3 = match_voice(&voices, "aria").unwrap();
        assert_eq!(m3.short_name.as_deref(), Some("en-US-AriaNeural"));
    }

    #[test]
    fn reports_ambiguous_and_missing_voices_with_candidates() {
        let voices = sample_voices();
        let err = match_voice(&voices, "Hsiao").unwrap_err().to_string();
        assert!(err.contains("zh-TW-HsiaoChenNeural"));
        assert!(err.contains("zh-TW-HsiaoYuNeural"));

        let err = match_voice(&voices, "NoSuchVoice").unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn filters_and_sorts_voices_by_locale_prefix() {
        let voices = sample_voices();
        let filtered = filter_and_sort_voices(voices, Some("zh-tw"));
        assert_eq!(filtered.len(), 3);
        assert_eq!(
            filtered[0].short_name.as_deref(),
            Some("zh-TW-HsiaoChenNeural")
        );
        assert_eq!(
            filtered[1].short_name.as_deref(),
            Some("zh-TW-HsiaoYuNeural")
        );
        assert_eq!(
            filtered[2].short_name.as_deref(),
            Some("zh-TW-YunJheNeural")
        );
    }
}
