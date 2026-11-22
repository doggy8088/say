use std::path::PathBuf;

use clap::Parser;

use crate::voice::DEFAULT_VOICE;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "say",
    version,
    about = "Convert text to audible speech using Microsoft Edge TTS (macOS say compatible)",
    after_help = "Examples:\n  \
        say \"你好世界\"\n  \
        say -v \"?\" --locale zh-TW\n  \
        say -v YunJhe -r +20% \"加速朗讀測試\"\n  \
        say -r 200 -o hello.wav \"Hello world\"\n  \
        say --progress -i \"逐字高亮播放\"\n  \
        say --interactive=/green \"Green background highlight\""
)]
pub struct Cli {
    /// Text to speak (words are joined with spaces). Reads from stdin if omitted and no -f is given.
    #[arg(value_name = "MESSAGE")]
    pub message: Vec<String>,

    /// Read text from file ('-' reads from stdin)
    #[arg(short = 'f', long = "input-file", value_name = "FILE")]
    pub input_file: Option<String>,

    /// Voice name (ShortName or partial name; '?' lists available voices)
    #[arg(
        short = 'v',
        long = "voice",
        value_name = "VOICE",
        default_value = DEFAULT_VOICE,
        env = "SAY_VOICE"
    )]
    pub voice: String,

    /// List available voices (same as -v '?')
    #[arg(long = "list-voices")]
    pub list_voices: bool,

    /// Filter voice list by locale prefix (e.g. zh-TW, zh, en-US)
    #[arg(long = "locale", value_name = "LOCALE")]
    pub locale: Option<String>,

    /// Speech rate: percentage delta (e.g. +20%, -15%) or macOS WPM (e.g. 200; 175 WPM = 0%)
    #[arg(
        short = 'r',
        long = "rate",
        value_name = "RATE",
        allow_hyphen_values = true
    )]
    pub rate: Option<String>,

    /// Pitch adjustment in Hz (e.g. +0Hz, +10Hz, -5Hz)
    #[arg(
        long = "pitch",
        value_name = "PITCH",
        default_value = "+0Hz",
        allow_hyphen_values = true
    )]
    pub pitch: String,

    /// Volume adjustment in percent (e.g. +0%, +20%, -30%)
    #[arg(
        long = "volume",
        value_name = "VOLUME",
        default_value = "+0%",
        allow_hyphen_values = true
    )]
    pub volume: String,

    /// Write synthesized audio to file instead of playing it
    #[arg(short = 'o', long = "output-file", value_name = "FILE")]
    pub output_file: Option<PathBuf>,

    /// Output file format (mp3, wav, ogg, webm; '?' lists supported formats)
    #[arg(long = "file-format", value_name = "FORMAT")]
    pub file_format: Option<String>,

    /// Native Edge TTS audio_format string ('?' lists supported data formats)
    #[arg(long = "data-format", value_name = "FORMAT")]
    pub data_format: Option<String>,

    /// Audio output device ID or name prefix ('?' lists output devices)
    #[arg(short = 'a', long = "audio-device", value_name = "DEVICE")]
    pub audio_device: Option<String>,

    /// Display synthesis and playback progress meter on stderr
    #[arg(long = "progress")]
    pub progress: bool,

    /// Highlight words as they are spoken (optional markup via '=': smso, bold, smul, color, fg/bg)
    #[arg(
        short = 'i',
        long = "interactive",
        value_name = "MARKUP",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "smso"
    )]
    pub interactive: Option<String>,
}

impl Cli {
    pub fn wants_voice_list(&self) -> bool {
        self.list_voices || self.voice.trim() == "?"
    }

    pub fn wants_device_list(&self) -> bool {
        self.audio_device.as_deref().map(str::trim) == Some("?")
    }

    pub fn wants_file_format_list(&self) -> bool {
        self.file_format.as_deref().map(str::trim) == Some("?")
    }

    pub fn wants_data_format_list(&self) -> bool {
        self.data_format.as_deref().map(str::trim) == Some("?")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[test]
    fn parses_interactive_flag_without_eating_message() {
        let cli = parse(&["say", "-i", "Hello world"]).unwrap();
        assert_eq!(cli.interactive.as_deref(), Some("smso"));
        assert_eq!(cli.message, vec!["Hello world"]);
    }

    #[test]
    fn parses_long_interactive_without_eating_message() {
        let cli = parse(&["say", "--interactive", "Hello", "world"]).unwrap();
        assert_eq!(cli.interactive.as_deref(), Some("smso"));
        assert_eq!(cli.message, vec!["Hello", "world"]);
    }

    #[test]
    fn parses_interactive_with_equals_markup() {
        let cli = parse(&["say", "--interactive=green", "Hello world"]).unwrap();
        assert_eq!(cli.interactive.as_deref(), Some("green"));
        assert_eq!(cli.message, vec!["Hello world"]);

        let cli = parse(&["say", "--interactive=/green", "Hello world"]).unwrap();
        assert_eq!(cli.interactive.as_deref(), Some("/green"));
        assert_eq!(cli.message, vec!["Hello world"]);

        let cli = parse(&["say", "-i=yellow/blue", "Hello"]).unwrap();
        assert_eq!(cli.interactive.as_deref(), Some("yellow/blue"));
        assert_eq!(cli.message, vec!["Hello"]);
    }

    #[test]
    fn parses_negative_rate_pitch_volume() {
        let cli = parse(&[
            "say", "-r", "-20%", "--pitch", "-10Hz", "--volume", "-30%", "test",
        ])
        .unwrap();
        assert_eq!(cli.rate.as_deref(), Some("-20%"));
        assert_eq!(cli.pitch, "-10Hz");
        assert_eq!(cli.volume, "-30%");
        assert_eq!(cli.message, vec!["test"]);
    }

    #[test]
    fn parses_positive_wpm_and_query_flags() {
        let cli = parse(&["say", "-r", "210", "-v", "YunJhe", "hello"]).unwrap();
        assert_eq!(cli.rate.as_deref(), Some("210"));
        assert_eq!(cli.voice, "YunJhe");

        let cli = parse(&["say", "-v", "?", "--locale", "zh-TW"]).unwrap();
        assert!(cli.wants_voice_list());
        assert_eq!(cli.locale.as_deref(), Some("zh-TW"));

        let cli = parse(&["say", "-a", "?"]).unwrap();
        assert!(cli.wants_device_list());

        let cli = parse(&["say", "--file-format", "?"]).unwrap();
        assert!(cli.wants_file_format_list());

        let cli = parse(&["say", "--data-format", "?"]).unwrap();
        assert!(cli.wants_data_format_list());
    }

    #[test]
    fn parses_input_and_output_files() {
        let cli = parse(&["say", "-f", "notes.txt", "-o", "out.wav", "--progress"]).unwrap();
        assert_eq!(cli.input_file.as_deref(), Some("notes.txt"));
        assert_eq!(cli.output_file, Some(PathBuf::from("out.wav")));
        assert!(cli.progress);
    }
}
