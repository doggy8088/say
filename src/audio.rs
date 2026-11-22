use std::io::Cursor;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::format::{decoder_hint_for_format, is_playable_format};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub name: String,
    pub is_default: bool,
}

fn device_display_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_owned())
        .unwrap_or_else(|_| device.to_string())
}

pub fn list_audio_devices() -> Result<Vec<AudioDeviceInfo>> {
    let host = cpal::default_host();
    let default_device = host.default_output_device();
    let default_id = default_device.as_ref().and_then(|d| d.id().ok());
    let default_name = default_device.as_ref().map(device_display_name);

    let devices = host
        .output_devices()
        .context("Failed to enumerate audio output devices")?;

    let mut list = Vec::new();
    for (index, device) in devices.enumerate() {
        let name = device_display_name(&device);
        let is_default = if default_id.is_some() {
            device.id().ok() == default_id
        } else {
            default_name.as_deref() == Some(name.as_str())
        };
        list.push(AudioDeviceInfo {
            index,
            name,
            is_default,
        });
    }

    Ok(list)
}

pub fn format_device_list(devices: &[AudioDeviceInfo]) -> String {
    if devices.is_empty() {
        return "No audio output devices found.\n".to_owned();
    }

    let id_width = devices
        .last()
        .map(|d| d.index.to_string().len())
        .unwrap_or(1)
        .max(2);

    let mut out = String::new();
    for dev in devices {
        let marker = if dev.is_default { " [default]" } else { "" };
        out.push_str(&format!(
            "{:>id_width$}  {}{}\n",
            dev.index, dev.name, marker
        ));
    }
    out
}

pub fn match_audio_device_index(devices: &[(usize, String)], query: &str) -> Result<usize> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        bail!("Audio device query cannot be empty. Run `say -a '?'` to list output devices.");
    }
    if devices.is_empty() {
        bail!("No audio output devices available on this system.");
    }

    if let Ok(id) = trimmed.parse::<usize>() {
        if devices.iter().any(|(idx, _)| *idx == id) {
            return Ok(id);
        }
        bail!(
            "Audio device ID {id} is out of range (valid IDs: 0..={}). Run `say -a '?'` to list output devices.",
            devices.len() - 1
        );
    }

    let query_lower = trimmed.to_ascii_lowercase();

    // 1. Exact case-insensitive match.
    if let Some((idx, _)) = devices
        .iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(trimmed))
    {
        return Ok(*idx);
    }

    // 2. Prefix case-insensitive match.
    let prefix_matches: Vec<(usize, &str)> = devices
        .iter()
        .filter(|(_, name)| name.to_ascii_lowercase().starts_with(&query_lower))
        .map(|(idx, name)| (*idx, name.as_str()))
        .collect();

    if prefix_matches.len() == 1 {
        return Ok(prefix_matches[0].0);
    }
    if prefix_matches.len() > 1 {
        return Err(ambiguous_device_error(trimmed, &prefix_matches));
    }

    // 3. Substring case-insensitive match.
    let sub_matches: Vec<(usize, &str)> = devices
        .iter()
        .filter(|(_, name)| name.to_ascii_lowercase().contains(&query_lower))
        .map(|(idx, name)| (*idx, name.as_str()))
        .collect();

    match sub_matches.len() {
        1 => Ok(sub_matches[0].0),
        n if n > 1 => Err(ambiguous_device_error(trimmed, &sub_matches)),
        _ => bail!(
            "Audio device '{trimmed}' not found. Run `say -a '?'` to list available output devices."
        ),
    }
}

fn ambiguous_device_error(query: &str, candidates: &[(usize, &str)]) -> anyhow::Error {
    let mut msg = format!(
        "Audio device '{query}' matched {} devices. Please specify a numeric ID or fuller name:\n",
        candidates.len()
    );
    for (idx, name) in candidates {
        msg.push_str(&format!("  {idx}: {name}\n"));
    }
    anyhow::anyhow!(msg.trim_end().to_owned())
}

pub fn resolve_audio_device(query: &str) -> Result<cpal::Device> {
    let host = cpal::default_host();
    let devices: Vec<cpal::Device> = host
        .output_devices()
        .context("Failed to enumerate audio output devices")?
        .collect();

    let indexed_names: Vec<(usize, String)> = devices
        .iter()
        .enumerate()
        .map(|(idx, d)| (idx, device_display_name(d)))
        .collect();

    let matched_idx = match_audio_device_index(&indexed_names, query)?;
    Ok(devices.into_iter().nth(matched_idx).expect("valid index"))
}

pub struct AudioOutput {
    sink: MixerDeviceSink,
}

impl AudioOutput {
    pub fn open(device_query: Option<&str>) -> Result<Self> {
        let mut sink = match device_query {
            Some(query) => {
                let device = resolve_audio_device(query)?;
                let name = device_display_name(&device);
                DeviceSinkBuilder::from_device(device)
                    .with_context(|| format!("Failed to configure audio device '{name}'"))?
                    .open_sink_or_fallback()
                    .with_context(|| format!("Failed to open audio stream on device '{name}'"))?
            }
            None => DeviceSinkBuilder::open_default_sink()
                .context("Failed to open default audio output device")?,
        };
        // Prevent rodio from printing a debug notice to stderr on normal program exit.
        sink.log_on_drop(false);
        Ok(Self { sink })
    }

    pub fn play_bytes<F>(
        &self,
        audio_bytes: Vec<u8>,
        audio_format: &str,
        fallback_duration: Option<Duration>,
        mut on_tick: F,
    ) -> Result<()>
    where
        F: FnMut(Duration, Option<Duration>),
    {
        if audio_bytes.is_empty() {
            return Ok(());
        }

        if !is_playable_format(audio_format) {
            bail!(
                "Direct audio playback does not support format '{audio_format}'. Use an MP3 or WAV format for playback, or pass `-o <file>` to write Opus/Ogg/WebM audio to disk."
            );
        }

        let byte_len = audio_bytes.len() as u64;
        let mut builder = Decoder::builder()
            .with_data(Cursor::new(audio_bytes))
            .with_byte_len(byte_len)
            .with_seekable(true);

        if let Some(hint) = decoder_hint_for_format(audio_format) {
            builder = builder.with_hint(hint);
        }

        let decoder = builder
            .build()
            .with_context(|| format!("Failed to decode audio stream ({audio_format})"))?;

        let total_duration = decoder.total_duration().or(fallback_duration);
        let player = Player::connect_new(self.sink.mixer());
        player.append(decoder);

        on_tick(Duration::ZERO, total_duration);
        while !player.empty() {
            let pos = player.get_pos();
            on_tick(pos, total_duration);
            thread::sleep(Duration::from_millis(25));
        }

        let final_pos = total_duration.unwrap_or_else(|| player.get_pos());
        on_tick(final_pos, total_duration);
        Ok(())
    }

    /// Brief pause before closing the OS output sink so the trailing hardware audio buffer finishes cleanly.
    pub fn drain(self) {
        thread::sleep(Duration::from_millis(80));
        drop(self.sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_device_by_id_prefix_and_substring() {
        let devices = vec![
            (0, "Speakers (Realtek High Definition Audio)".to_owned()),
            (1, "Headphones (USB Audio Device)".to_owned()),
            (2, "HDMI Output (Display Audio)".to_owned()),
        ];

        assert_eq!(match_audio_device_index(&devices, "1").unwrap(), 1);
        assert_eq!(match_audio_device_index(&devices, "speakers").unwrap(), 0);
        assert_eq!(match_audio_device_index(&devices, "USB Audio").unwrap(), 1);
        assert_eq!(match_audio_device_index(&devices, "hdmi").unwrap(), 2);
    }

    #[test]
    fn errors_on_out_of_range_ambiguous_or_missing_device() {
        let devices = vec![
            (0, "Speakers (Realtek)".to_owned()),
            (1, "Speakers (USB)".to_owned()),
        ];

        assert!(match_audio_device_index(&devices, "5").is_err());
        let err = match_audio_device_index(&devices, "Speakers")
            .unwrap_err()
            .to_string();
        assert!(err.contains("0: Speakers (Realtek)"));
        assert!(err.contains("1: Speakers (USB)"));
        assert!(match_audio_device_index(&devices, "Bluetooth").is_err());
    }

    #[test]
    fn formats_device_list_with_default_marker() {
        let list = vec![
            AudioDeviceInfo {
                index: 0,
                name: "Speakers".to_owned(),
                is_default: true,
            },
            AudioDeviceInfo {
                index: 1,
                name: "Headphones".to_owned(),
                is_default: false,
            },
        ];
        let rendered = format_device_list(&list);
        assert!(rendered.contains(" 0  Speakers [default]\n"));
        assert!(rendered.contains(" 1  Headphones\n"));
    }
}
