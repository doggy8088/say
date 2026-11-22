mod audio;
mod cli;
mod format;
mod interactive;
mod rate;
mod text;
mod tts;
mod voice;

use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser;
use msedge_tts::tts::SpeechConfig;

use crate::audio::AudioOutput;
use crate::cli::Cli;
use crate::format::{
    AudioFormatPlan, assemble_output_audio, format_data_formats_list, format_file_formats_list,
    is_playable_format, resolve_format_plan,
};
use crate::interactive::{HighlightStyle, PlaybackDisplay, ProgressBar};
use crate::rate::{parse_pitch, parse_rate, parse_volume};
use crate::text::{DEFAULT_MAX_CHUNK_CHARS, TextInput, chunk_text, resolve_input, strip_bom};
use crate::tts::TtsClient;
use crate::voice::{format_voices_table, list_voices, resolve_voice};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // 1. Handle immediate query modes before reading stdin or validating synthesis options.
    if cli.wants_file_format_list() {
        print!("{}", format_file_formats_list());
        return Ok(());
    }

    if cli.wants_data_format_list() {
        print!("{}", format_data_formats_list());
        return Ok(());
    }

    if cli.wants_device_list() {
        let devices = audio::list_audio_devices()?;
        print!("{}", audio::format_device_list(&devices));
        return Ok(());
    }

    if cli.wants_voice_list() {
        let voices = list_voices(cli.locale.as_deref())?;
        print!("{}", format_voices_table(&voices, cli.locale.as_deref()));
        return Ok(());
    }

    // 2. Parse & validate speech, format, and display parameters upfront.
    let rate = match cli.rate.as_deref() {
        Some(raw) => parse_rate(raw)?,
        None => 0,
    };
    let pitch = parse_pitch(&cli.pitch)?;
    let volume = parse_volume(&cli.volume)?;

    let format_plan = resolve_format_plan(
        cli.data_format.as_deref(),
        cli.file_format.as_deref(),
        cli.output_file.as_deref(),
    )?;

    if cli.output_file.is_none() && !is_playable_format(&format_plan.synthesis_format) {
        bail!(
            "Direct audio playback does not support format '{}'. Use MP3/WAV for playback or specify `-o <FILE>` to save Opus/Ogg/WebM audio.",
            format_plan.requested_format
        );
    }

    let highlight_style = cli
        .interactive
        .as_deref()
        .map(HighlightStyle::parse)
        .transpose()?;

    // 3. Resolve text input source.
    let input = resolve_input(&cli)?;

    // Exit early for empty non-TTY input without making network calls.
    let chunks_opt = match input {
        TextInput::FullText(ref full) => {
            let chunks = chunk_text(full, DEFAULT_MAX_CHUNK_CHARS);
            if chunks.is_empty() {
                return Ok(());
            }
            Some(chunks)
        }
        TextInput::InteractiveTty => None,
    };

    // 4. Resolve voice configuration.
    let config = resolve_voice(
        &cli.voice,
        &format_plan.synthesis_format,
        rate,
        pitch,
        volume,
    )?;
    let progress = ProgressBar::new(cli.progress);
    let mut client = TtsClient::new();

    // 5. Execute either file output mode (-o) or real-time audio playback mode.
    if let Some(ref output_path) = cli.output_file {
        match chunks_opt {
            Some(chunks) => {
                synthesize_chunks_to_file(
                    &mut client,
                    &chunks,
                    &config,
                    &format_plan,
                    output_path,
                    &progress,
                )?;
            }
            None => {
                synthesize_tty_to_file(&mut client, &config, &format_plan, output_path, &progress)?;
            }
        }
        return Ok(());
    }

    let audio_output = AudioOutput::open(cli.audio_device.as_deref())?;
    match chunks_opt {
        Some(chunks) => {
            speak_chunks(
                &mut client,
                &audio_output,
                &chunks,
                &config,
                highlight_style.as_ref(),
                &progress,
            )?;
        }
        None => {
            speak_tty(
                &mut client,
                &audio_output,
                &config,
                highlight_style.as_ref(),
                &progress,
            )?;
        }
    }

    audio_output.drain();
    Ok(())
}

fn empty_audio_error(config: &SpeechConfig) -> anyhow::Error {
    anyhow::anyhow!(
        "Synthesis produced no audio data for voice '{}'. Verify the voice name with `say -v '?'` and ensure the input contains speakable text.",
        config.voice_name
    )
}

fn synthesize_chunks_to_file(
    client: &mut TtsClient,
    chunks: &[String],
    config: &SpeechConfig,
    format_plan: &AudioFormatPlan,
    output_path: &Path,
    progress: &ProgressBar,
) -> Result<()> {
    let total = chunks.len();
    let mut raw_audio_chunks = Vec::with_capacity(total);

    for (idx, chunk) in chunks.iter().enumerate() {
        progress.update_synthesizing(idx, total);
        let synthesized = client.synthesize(chunk, config)?;
        if !synthesized.audio_bytes.is_empty() {
            raw_audio_chunks.push(synthesized.audio_bytes);
        }
        progress.update_synthesizing(idx + 1, total);
    }

    let merged = assemble_output_audio(&raw_audio_chunks, format_plan)?;
    if merged.is_empty() {
        progress.clear_line();
        return Err(empty_audio_error(config));
    }

    fs::write(output_path, &merged)
        .with_context(|| format!("Failed to write audio file '{}'", output_path.display()))?;

    progress.finish_message(&format!(
        "Saved {} ({} bytes)",
        output_path.display(),
        merged.len()
    ));
    Ok(())
}

fn synthesize_tty_to_file(
    client: &mut TtsClient,
    config: &SpeechConfig,
    format_plan: &AudioFormatPlan,
    output_path: &Path,
    progress: &ProgressBar,
) -> Result<()> {
    let stdin = io::stdin();
    let mut raw_audio_chunks = Vec::new();
    let mut line = String::new();
    let mut line_count = 0usize;

    loop {
        line.clear();
        let bytes_read = stdin
            .lock()
            .read_line(&mut line)
            .context("Failed to read line from standard input")?;
        if bytes_read == 0 {
            break;
        }
        let cleaned = strip_bom(&line).trim();
        if cleaned.is_empty() {
            continue;
        }
        for chunk in chunk_text(cleaned, DEFAULT_MAX_CHUNK_CHARS) {
            line_count += 1;
            progress.update_synthesizing(line_count.saturating_sub(1), line_count);
            let synthesized = client.synthesize(&chunk, config)?;
            if !synthesized.audio_bytes.is_empty() {
                raw_audio_chunks.push(synthesized.audio_bytes);
            }
            progress.update_synthesizing(line_count, line_count);
        }
    }

    if line_count == 0 {
        return Ok(());
    }

    let merged = assemble_output_audio(&raw_audio_chunks, format_plan)?;
    if merged.is_empty() {
        progress.clear_line();
        return Err(empty_audio_error(config));
    }

    fs::write(output_path, &merged)
        .with_context(|| format!("Failed to write audio file '{}'", output_path.display()))?;
    progress.finish_message(&format!(
        "Saved {} ({} bytes)",
        output_path.display(),
        merged.len()
    ));
    Ok(())
}

fn speak_chunks(
    client: &mut TtsClient,
    audio_output: &AudioOutput,
    chunks: &[String],
    config: &SpeechConfig,
    highlight_style: Option<&HighlightStyle>,
    progress: &ProgressBar,
) -> Result<()> {
    let total = chunks.len();
    let mut produced_any_audio = false;

    for (idx, chunk) in chunks.iter().enumerate() {
        progress.update_synthesizing(idx, total);
        let synthesized = client.synthesize(chunk, config)?;
        if synthesized.audio_bytes.is_empty() {
            continue;
        }
        produced_any_audio = true;

        let fallback_duration = synthesized.metadata_duration();
        let mut display = PlaybackDisplay::new(
            &synthesized.text,
            &synthesized.audio_metadata,
            highlight_style,
            progress,
            idx + 1,
            total,
        );

        let mut observed_total = fallback_duration;
        audio_output.play_bytes(
            synthesized.audio_bytes,
            &synthesized.audio_format,
            fallback_duration,
            |pos, total_dur| {
                if total_dur.is_some() {
                    observed_total = total_dur;
                }
                display.update(pos, total_dur);
            },
        )?;
        display.finish(observed_total);
    }

    if !chunks.is_empty() && !produced_any_audio {
        progress.clear_line();
        return Err(empty_audio_error(config));
    }

    Ok(())
}

fn speak_tty(
    client: &mut TtsClient,
    audio_output: &AudioOutput,
    config: &SpeechConfig,
    highlight_style: Option<&HighlightStyle>,
    progress: &ProgressBar,
) -> Result<()> {
    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = stdin
            .lock()
            .read_line(&mut line)
            .context("Failed to read line from standard input")?;
        if bytes_read == 0 {
            break;
        }
        let cleaned = strip_bom(&line).trim();
        if cleaned.is_empty() {
            continue;
        }
        let chunks = chunk_text(cleaned, DEFAULT_MAX_CHUNK_CHARS);
        if !chunks.is_empty() {
            speak_chunks(
                client,
                audio_output,
                &chunks,
                config,
                highlight_style,
                progress,
            )?;
        }
    }

    Ok(())
}
