use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rodio::{Decoder, Source};

pub const DEFAULT_MP3_FORMAT: &str = "audio-24khz-48kbitrate-mono-mp3";
pub const HQ_MP3_FORMAT: &str = "audio-24khz-96kbitrate-mono-mp3";
pub const WAV_FORMAT: &str = "riff-24khz-16bit-mono-pcm";
pub const OGG_FORMAT: &str = "ogg-24khz-16bit-mono-opus";
pub const WEBM_FORMAT: &str = "webm-24khz-16bit-mono-opus";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Mp3,
    Wav,
    Ogg,
    Webm,
}

impl FileFormat {
    pub fn parse(input: &str) -> Result<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "mp3" | "mpeg" => Ok(Self::Mp3),
            "wav" | "wave" | "riff" | "pcm" => Ok(Self::Wav),
            "ogg" | "opus" => Ok(Self::Ogg),
            "webm" => Ok(Self::Webm),
            other => bail!(
                "Unsupported file format '{other}'. Supported formats: mp3, wav, ogg, webm (run `say --file-format '?'` for details)"
            ),
        }
    }

    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("wav" | "wave") => Self::Wav,
            Some("ogg" | "opus") => Self::Ogg,
            Some("webm") => Self::Webm,
            _ => Self::Mp3,
        }
    }

    pub fn default_edge_format(self) -> &'static str {
        match self {
            Self::Mp3 => DEFAULT_MP3_FORMAT,
            Self::Wav => WAV_FORMAT,
            Self::Ogg => OGG_FORMAT,
            Self::Webm => WEBM_FORMAT,
        }
    }
}

/// Strategy describing both what format to request from Microsoft Edge TTS and
/// how to assemble/encode the final audio payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormatPlan {
    /// Logical format name selected by the user or inferred from output extension.
    pub requested_format: String,
    /// Actual `outputFormat` sent to Microsoft Edge Read Aloud WebSocket.
    pub synthesis_format: String,
    /// Post-processing strategy when producing an output file (`-o`).
    pub output_mode: OutputAssemblyMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputAssemblyMode {
    /// Concatenate raw MP3 frames (stripping subsequent ID3v2 headers).
    Mp3,
    /// Synthesize 96 kbps MP3 and decode in pure Rust to standard RIFF WAVE 16-bit PCM.
    WavFromMp3,
    /// Synthesize native WebM Opus and losslessly remux Opus packets into RFC 7845 Ogg Opus.
    OggOpusFromWebm,
    /// Pass through / assemble native Edge TTS WebM Opus.
    WebmOpus,
    /// Pass through custom `--data-format` payload cleanly (with WAV header merging if RIFF).
    Passthrough,
}

pub struct FileFormatSpec {
    pub id: &'static str,
    pub extensions: &'static str,
    pub edge_format: &'static str,
    pub description: &'static str,
}

pub const SUPPORTED_FILE_FORMATS: &[FileFormatSpec] = &[
    FileFormatSpec {
        id: "mp3",
        extensions: ".mp3",
        edge_format: DEFAULT_MP3_FORMAT,
        description: "MPEG-1 Layer 3 audio (default, native Edge TTS)",
    },
    FileFormatSpec {
        id: "wav",
        extensions: ".wav, .wave",
        edge_format: WAV_FORMAT,
        description: "RIFF WAVE uncompressed 16-bit PCM (24 kHz mono)",
    },
    FileFormatSpec {
        id: "ogg",
        extensions: ".ogg, .opus",
        edge_format: OGG_FORMAT,
        description: "Ogg container with Opus audio (24 kHz mono)",
    },
    FileFormatSpec {
        id: "webm",
        extensions: ".webm",
        edge_format: WEBM_FORMAT,
        description: "WebM container with Opus audio (24 kHz mono, native Edge TTS)",
    },
];

pub struct DataFormatSpec {
    pub name: &'static str,
    pub container: &'static str,
    pub playable: bool,
    pub description: &'static str,
}

pub const SUPPORTED_DATA_FORMATS: &[DataFormatSpec] = &[
    DataFormatSpec {
        name: "audio-24khz-48kbitrate-mono-mp3",
        container: "mp3",
        playable: true,
        description: "24 kHz, 48 kbps, mono MP3 (default, native Edge TTS)",
    },
    DataFormatSpec {
        name: "audio-24khz-96kbitrate-mono-mp3",
        container: "mp3",
        playable: true,
        description: "24 kHz, 96 kbps, mono MP3 (high quality, native Edge TTS)",
    },
    DataFormatSpec {
        name: "riff-24khz-16bit-mono-pcm",
        container: "wav",
        playable: true,
        description: "24 kHz, 16-bit, mono RIFF WAV PCM (pure-Rust PCM encoding)",
    },
    DataFormatSpec {
        name: "ogg-24khz-16bit-mono-opus",
        container: "ogg",
        playable: false,
        description: "24 kHz, mono Ogg Opus (lossless Opus remux for -o)",
    },
    DataFormatSpec {
        name: "webm-24khz-16bit-mono-opus",
        container: "webm",
        playable: false,
        description: "24 kHz, mono WebM Opus (native Edge TTS for -o)",
    },
];

/// Resolve the logical Edge TTS format string from CLI flags and optional output path.
pub fn resolve_audio_format(
    data_format: Option<&str>,
    file_format: Option<&str>,
    output_file: Option<&Path>,
) -> Result<String> {
    if let Some(df) = data_format {
        let trimmed = df.trim();
        if trimmed.is_empty() {
            bail!("--data-format cannot be empty");
        }
        return Ok(trimmed.to_owned());
    }

    if let Some(ff) = file_format {
        let parsed = FileFormat::parse(ff)?;
        return Ok(parsed.default_edge_format().to_owned());
    }

    if let Some(path) = output_file {
        return Ok(FileFormat::from_path(path).default_edge_format().to_owned());
    }

    Ok(DEFAULT_MP3_FORMAT.to_owned())
}

/// Build the full synthesis & output assembly plan from CLI flags.
///
/// Note: Microsoft Edge Read Aloud's WebSocket endpoint natively serves MP3
/// (`audio-24khz-48kbitrate-mono-mp3`, `audio-24khz-96kbitrate-mono-mp3`) and
/// WebM Opus (`webm-24khz-16bit-mono-opus`), while silently returning empty audio
/// for `riff-*` and `ogg-*`. For WAV and Ogg Opus outputs, we transparently request
/// high-quality native Edge audio and perform pure-Rust PCM WAV encoding or lossless
/// Ogg Opus remuxing with zero external tools.
pub fn resolve_format_plan(
    data_format: Option<&str>,
    file_format: Option<&str>,
    output_file: Option<&Path>,
) -> Result<AudioFormatPlan> {
    let requested = resolve_audio_format(data_format, file_format, output_file)?;
    let lower = requested.to_ascii_lowercase();

    if lower.starts_with("riff-") {
        Ok(AudioFormatPlan {
            requested_format: requested,
            synthesis_format: HQ_MP3_FORMAT.to_owned(),
            output_mode: OutputAssemblyMode::WavFromMp3,
        })
    } else if lower.starts_with("ogg-") {
        Ok(AudioFormatPlan {
            requested_format: requested,
            synthesis_format: WEBM_FORMAT.to_owned(),
            output_mode: OutputAssemblyMode::OggOpusFromWebm,
        })
    } else if lower.starts_with("webm-") {
        Ok(AudioFormatPlan {
            requested_format: requested.clone(),
            synthesis_format: requested,
            output_mode: OutputAssemblyMode::WebmOpus,
        })
    } else if lower.contains("mp3") {
        Ok(AudioFormatPlan {
            requested_format: requested.clone(),
            synthesis_format: requested,
            output_mode: OutputAssemblyMode::Mp3,
        })
    } else {
        Ok(AudioFormatPlan {
            requested_format: requested.clone(),
            synthesis_format: requested,
            output_mode: OutputAssemblyMode::Passthrough,
        })
    }
}

pub fn format_file_formats_list() -> String {
    let mut out = String::from("Supported writable file formats (--file-format):\n");
    for spec in SUPPORTED_FILE_FORMATS {
        out.push_str(&format!(
            "  {:<6} {:<12} {:<34} {}\n",
            spec.id, spec.extensions, spec.edge_format, spec.description
        ));
    }
    out
}

pub fn format_data_formats_list() -> String {
    let mut out = String::from("Supported native Edge TTS audio data formats (--data-format):\n");
    for spec in SUPPORTED_DATA_FORMATS {
        let mode = if spec.playable {
            "play / -o"
        } else {
            "-o only"
        };
        out.push_str(&format!(
            "  {:<36} {:<6} {:<10} {}\n",
            spec.name, spec.container, mode, spec.description
        ));
    }
    out
}

pub fn decoder_hint_for_format(audio_format: &str) -> Option<&'static str> {
    let lower = audio_format.to_ascii_lowercase();
    if lower.contains("mp3") {
        Some("mp3")
    } else if lower.starts_with("riff-") || lower.contains("wav") {
        Some("wav")
    } else if lower.starts_with("ogg-") {
        Some("ogg")
    } else {
        None
    }
}

pub fn is_playable_format(audio_format: &str) -> bool {
    let lower = audio_format.to_ascii_lowercase();
    lower.contains("mp3") || lower.starts_with("riff-")
}

pub fn is_wav_format(audio_format: &str) -> bool {
    audio_format.to_ascii_lowercase().starts_with("riff-")
}

/// Assemble synthesized audio chunks according to the selected `AudioFormatPlan`.
pub fn assemble_output_audio(chunks: &[Vec<u8>], plan: &AudioFormatPlan) -> Result<Vec<u8>> {
    let non_empty: Vec<&[u8]> = chunks
        .iter()
        .map(Vec::as_slice)
        .filter(|c| !c.is_empty())
        .collect();

    if non_empty.is_empty() {
        return Ok(Vec::new());
    }

    match plan.output_mode {
        OutputAssemblyMode::Mp3 => merge_audio_chunks(chunks, &plan.synthesis_format),
        OutputAssemblyMode::WavFromMp3 => encode_mp3_chunks_to_wav(&non_empty),
        OutputAssemblyMode::OggOpusFromWebm => remux_webm_chunks_to_ogg_opus(&non_empty),
        OutputAssemblyMode::WebmOpus => {
            if non_empty.len() == 1 {
                Ok(non_empty[0].to_vec())
            } else {
                let total: usize = non_empty.iter().map(|c| c.len()).sum();
                let mut out = Vec::with_capacity(total);
                for chunk in non_empty {
                    out.extend_from_slice(chunk);
                }
                Ok(out)
            }
        }
        OutputAssemblyMode::Passthrough => merge_audio_chunks(chunks, &plan.requested_format),
    }
}

/// Decode one or more MP3 chunks in pure Rust (via Symphonia/rodio) and encode
/// a standard RIFF WAVE 16-bit PCM file with exact header sizes.
pub fn encode_mp3_chunks_to_wav(mp3_chunks: &[&[u8]]) -> Result<Vec<u8>> {
    let mut pcm_bytes: Vec<u8> = Vec::new();
    let mut target_channels: Option<u16> = None;
    let mut target_sample_rate: Option<u32> = None;

    for chunk in mp3_chunks {
        if chunk.is_empty() {
            continue;
        }
        let decoder = Decoder::builder()
            .with_data(Cursor::new(chunk.to_vec()))
            .with_byte_len(chunk.len() as u64)
            .with_seekable(true)
            .with_hint("mp3")
            .build()
            .context("Failed to decode synthesized MP3 audio for WAV output")?;

        let channels = decoder.channels().get();
        let sample_rate = decoder.sample_rate().get();
        target_channels.get_or_insert(channels);
        target_sample_rate.get_or_insert(sample_rate);

        for sample in decoder {
            let clamped = sample.clamp(-1.0, 1.0);
            let quantized = (clamped * i16::MAX as f32).round() as i16;
            pcm_bytes.extend_from_slice(&quantized.to_le_bytes());
        }
    }

    if pcm_bytes.is_empty() {
        return Ok(Vec::new());
    }

    let channels = target_channels.unwrap_or(1);
    let sample_rate = target_sample_rate.unwrap_or(24_000);
    build_pcm_wav(channels, sample_rate, &pcm_bytes)
}

pub fn build_pcm_wav(channels: u16, sample_rate: u32, pcm_16le: &[u8]) -> Result<Vec<u8>> {
    let data_size = u32::try_from(pcm_16le.len())
        .map_err(|_| anyhow::anyhow!("WAV output exceeds 4 GiB RIFF format limit"))?;
    let riff_size = 36u32
        .checked_add(data_size)
        .ok_or_else(|| anyhow::anyhow!("WAV output exceeds 4 GiB RIFF format limit"))?;
    let bits_per_sample: u16 = 16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * block_align as u32;

    let mut out = Vec::with_capacity(44 + pcm_16le.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(pcm_16le);
    Ok(out)
}

/// Concatenate multiple synthesized audio chunks into a single valid audio payload.
/// For RIFF/WAV streams, strips subsequent headers and rewrites RIFF & data chunk sizes cleanly.
pub fn merge_audio_chunks(chunks: &[Vec<u8>], audio_format: &str) -> Result<Vec<u8>> {
    let non_empty: Vec<&[u8]> = chunks
        .iter()
        .map(Vec::as_slice)
        .filter(|c| !c.is_empty())
        .collect();

    if non_empty.is_empty() {
        return Ok(Vec::new());
    }

    let treat_as_wav = is_wav_format(audio_format)
        || (non_empty[0].len() >= 12
            && &non_empty[0][0..4] == b"RIFF"
            && &non_empty[0][8..12] == b"WAVE");

    if treat_as_wav {
        return merge_wav_chunks(&non_empty);
    }

    if non_empty.len() == 1 {
        return Ok(non_empty[0].to_vec());
    }

    let is_mp3 = audio_format.to_ascii_lowercase().contains("mp3");
    let total_len: usize = non_empty.iter().map(|c| c.len()).sum();
    let mut merged = Vec::with_capacity(total_len);

    for (idx, chunk) in non_empty.iter().enumerate() {
        if is_mp3 && idx > 0 {
            merged.extend_from_slice(strip_leading_id3v2(chunk));
        } else {
            merged.extend_from_slice(chunk);
        }
    }

    Ok(merged)
}

fn strip_leading_id3v2(data: &[u8]) -> &[u8] {
    if data.len() >= 10 && &data[0..3] == b"ID3" {
        let size = (((data[6] & 0x7f) as usize) << 21)
            | (((data[7] & 0x7f) as usize) << 14)
            | (((data[8] & 0x7f) as usize) << 7)
            | ((data[9] & 0x7f) as usize);
        let header_end = 10 + size;
        if header_end <= data.len() {
            return &data[header_end..];
        }
    }
    data
}

struct ParsedWav<'a> {
    fmt_payload: &'a [u8],
    pcm_data: &'a [u8],
}

fn parse_wav(bytes: &[u8]) -> Result<ParsedWav<'_>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("Invalid WAV audio chunk: missing RIFF/WAVE header");
    }

    let mut offset = 12usize;
    let mut fmt_payload: Option<&[u8]> = None;
    let mut pcm_data: Option<&[u8]> = None;

    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let raw_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let payload_start = offset + 8;

        if chunk_id == b"data" {
            let payload_end = if raw_size == 0
                || raw_size == u32::MAX as usize
                || payload_start.saturating_add(raw_size) > bytes.len()
            {
                bytes.len()
            } else {
                payload_start + raw_size
            };
            pcm_data = Some(&bytes[payload_start..payload_end]);
            break;
        }

        let payload_end = payload_start.saturating_add(raw_size).min(bytes.len());
        if chunk_id == b"fmt " {
            fmt_payload = Some(&bytes[payload_start..payload_end]);
        }

        let padded_size = raw_size + (raw_size % 2);
        offset = match payload_start.checked_add(padded_size) {
            Some(next) if next > offset => next,
            _ => break,
        };
    }

    let fmt_payload =
        fmt_payload.ok_or_else(|| anyhow::anyhow!("WAV chunk missing 'fmt ' header"))?;
    let pcm_data = pcm_data.ok_or_else(|| anyhow::anyhow!("WAV chunk missing 'data' payload"))?;

    Ok(ParsedWav {
        fmt_payload,
        pcm_data,
    })
}

fn merge_wav_chunks(chunks: &[&[u8]]) -> Result<Vec<u8>> {
    let first = parse_wav(chunks[0])?;
    let fmt = first.fmt_payload;
    let fmt_pad = fmt.len() % 2;

    let mut total_pcm_len = first.pcm_data.len();
    let mut pcm_slices = Vec::with_capacity(chunks.len());
    pcm_slices.push(first.pcm_data);

    for chunk in &chunks[1..] {
        let parsed = parse_wav(chunk)?;
        total_pcm_len = total_pcm_len
            .checked_add(parsed.pcm_data.len())
            .ok_or_else(|| anyhow::anyhow!("Merged WAV output exceeds addressable size"))?;
        pcm_slices.push(parsed.pcm_data);
    }

    let data_size_u32 = u32::try_from(total_pcm_len)
        .map_err(|_| anyhow::anyhow!("Merged WAV audio exceeds 4 GiB RIFF format limit"))?;
    let fmt_size_u32 = u32::try_from(fmt.len())
        .map_err(|_| anyhow::anyhow!("WAV 'fmt ' chunk exceeds RIFF size limit"))?;

    let riff_size = 4u32
        .checked_add(8)
        .and_then(|n| n.checked_add(fmt_size_u32))
        .and_then(|n| n.checked_add(fmt_pad as u32))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(data_size_u32))
        .ok_or_else(|| anyhow::anyhow!("Merged WAV header exceeds 4 GiB RIFF format limit"))?;

    let mut out = Vec::with_capacity(8 + riff_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&fmt_size_u32.to_le_bytes());
    out.extend_from_slice(fmt);
    if fmt_pad == 1 {
        out.push(0);
    }

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size_u32.to_le_bytes());
    for slice in pcm_slices {
        out.extend_from_slice(slice);
    }

    Ok(out)
}

/// Losslessly remux one or more Edge TTS WebM Opus streams into a standard RFC 7845 Ogg Opus file.
pub fn remux_webm_chunks_to_ogg_opus(webm_chunks: &[&[u8]]) -> Result<Vec<u8>> {
    let mut opus_head: Option<Vec<u8>> = None;
    let mut packets: Vec<Vec<u8>> = Vec::new();

    for chunk in webm_chunks {
        let extracted = extract_webm_opus(chunk)?;
        if opus_head.is_none() {
            opus_head = extracted.opus_head;
        }
        packets.extend(extracted.packets);
    }

    if packets.is_empty() {
        bail!("Failed to extract Opus audio packets from WebM stream");
    }

    // Default RFC 7845 OpusHead (24 kHz mono, pre-skip 312) if CodecPrivate was omitted.
    let head_packet = opus_head.unwrap_or_else(|| {
        let mut h = Vec::with_capacity(19);
        h.extend_from_slice(b"OpusHead");
        h.push(1); // version
        h.push(1); // channels
        h.extend_from_slice(&312u16.to_le_bytes()); // pre-skip
        h.extend_from_slice(&24_000u32.to_le_bytes()); // original input sample rate
        h.extend_from_slice(&0i16.to_le_bytes()); // output gain
        h.push(0); // channel mapping family
        h
    });

    let mut tags_packet = Vec::with_capacity(20);
    tags_packet.extend_from_slice(b"OpusTags");
    tags_packet.extend_from_slice(&3u32.to_le_bytes());
    tags_packet.extend_from_slice(b"say");
    tags_packet.extend_from_slice(&0u32.to_le_bytes());

    let serial: u32 = 0x5341_5901;
    let mut out = Vec::new();
    let mut seq: u32 = 0;

    // Page 0: BOS with OpusHead.
    write_ogg_page(&mut out, 0x02, 0, serial, seq, &[head_packet.as_slice()]);
    seq += 1;

    // Page 1: OpusTags comment header.
    write_ogg_page(&mut out, 0x00, 0, serial, seq, &[tags_packet.as_slice()]);
    seq += 1;

    // Audio pages: batch packets into Ogg pages (max 255 lacing segments per page).
    let mut granule: u64 = 0;
    let mut idx = 0usize;
    while idx < packets.len() {
        let mut page_packets: Vec<&[u8]> = Vec::new();
        let mut seg_count = 0usize;

        while idx < packets.len() && page_packets.len() < 40 {
            let pkt = packets[idx].as_slice();
            let needed_segs = (pkt.len() / 255) + 1;
            if seg_count + needed_segs > 255 && !page_packets.is_empty() {
                break;
            }
            seg_count += needed_segs;
            granule = granule.saturating_add(opus_packet_samples_48k(pkt));
            page_packets.push(pkt);
            idx += 1;
        }

        let header_type = if idx >= packets.len() { 0x04 } else { 0x00 };
        write_ogg_page(&mut out, header_type, granule, serial, seq, &page_packets);
        seq += 1;
    }

    Ok(out)
}

struct ExtractedWebmOpus {
    opus_head: Option<Vec<u8>>,
    packets: Vec<Vec<u8>>,
}

fn extract_webm_opus(data: &[u8]) -> Result<ExtractedWebmOpus> {
    let mut opus_head = None;
    let mut packets = Vec::new();
    walk_ebml(data, 0, data.len(), false, &mut opus_head, &mut packets)?;
    Ok(ExtractedWebmOpus { opus_head, packets })
}

fn is_segment_level_id(id: u32) -> bool {
    matches!(
        id,
        0x1A45_DFA3 // EBML
            | 0x1853_8067 // Segment
            | 0x114D_9B74 // SeekHead
            | 0x1549_A966 // Info
            | 0x1654_AE6B // Tracks
            | 0x1F43_B675 // Cluster
            | 0x1C53_BB6B // Cues
            | 0x1254_C367 // Tags
    )
}

fn walk_ebml(
    data: &[u8],
    start: usize,
    end: usize,
    inside_unknown_cluster: bool,
    opus_head: &mut Option<Vec<u8>>,
    packets: &mut Vec<Vec<u8>>,
) -> Result<usize> {
    let mut pos = start;
    while pos < end {
        let Some((id, id_len)) = read_ebml_id(&data[pos..end]) else {
            break;
        };
        if inside_unknown_cluster && is_segment_level_id(id) {
            // Reached the next sibling element at Segment level; let parent consume it.
            return Ok(pos);
        }
        pos += id_len;
        let Some((size_opt, size_len)) = read_ebml_size(&data[pos..end]) else {
            break;
        };
        pos += size_len;

        let elem_end = match size_opt {
            Some(sz) => pos.saturating_add(sz).min(end),
            None => end,
        };

        // Matroska/WebM container elements we descend into:
        // Segment (0x18538067), Tracks (0x1654AE6B), TrackEntry (0xAE),
        // Cluster (0x1F43B675), BlockGroup (0xA0).
        match id {
            0x1F43_B675 => {
                let next_pos =
                    walk_ebml(data, pos, elem_end, size_opt.is_none(), opus_head, packets)?;
                if size_opt.is_none() {
                    pos = next_pos;
                    continue;
                }
            }
            0x1853_8067 | 0x1654_AE6B | 0xAE | 0xA0 => {
                let next_pos = walk_ebml(data, pos, elem_end, false, opus_head, packets)?;
                if size_opt.is_none() {
                    pos = next_pos;
                    continue;
                }
            }
            // CodecPrivate (0x63A2): contains OpusHead for A_OPUS.
            0x63A2 => {
                let payload = &data[pos..elem_end];
                if payload.starts_with(b"OpusHead") && opus_head.is_none() {
                    *opus_head = Some(payload.to_vec());
                }
            }
            // SimpleBlock (0xA3) / Block (0xA1).
            0xA3 | 0xA1 => {
                let payload = &data[pos..elem_end];
                if let Some((_, track_vint_len)) = read_ebml_size(payload)
                    && track_vint_len + 3 < payload.len()
                {
                    let flags = payload[track_vint_len + 2];
                    let lacing = (flags >> 1) & 0x03;
                    if lacing == 0 {
                        packets.push(payload[track_vint_len + 3..].to_vec());
                    }
                }
            }
            _ => {}
        }

        if size_opt.is_none() {
            return Ok(end);
        }
        pos = elem_end;
    }
    Ok(pos)
}

fn read_ebml_id(buf: &[u8]) -> Option<(u32, usize)> {
    let first = *buf.first()?;
    if first == 0 {
        return None;
    }
    let width = first.leading_zeros() as usize + 1;
    if width > 4 || width > buf.len() {
        return None;
    }
    let mut id = 0u32;
    for &b in &buf[..width] {
        id = (id << 8) | b as u32;
    }
    Some((id, width))
}

fn read_ebml_size(buf: &[u8]) -> Option<(Option<usize>, usize)> {
    let first = *buf.first()?;
    if first == 0 {
        return None;
    }
    let width = first.leading_zeros() as usize + 1;
    if width > 8 || width > buf.len() {
        return None;
    }
    let mask = if width == 8 { 0 } else { 0xFFu8 >> width };
    let mut val = (first & mask) as u64;
    for &b in &buf[1..width] {
        val = (val << 8) | b as u64;
    }
    let unknown_sentinel = (1u64 << (7 * width)) - 1;
    if val == unknown_sentinel {
        Some((None, width))
    } else {
        Some((usize::try_from(val).ok(), width))
    }
}

/// Compute 48 kHz sample count of an Opus packet per RFC 6716 Section 3.1.
fn opus_packet_samples_48k(packet: &[u8]) -> u64 {
    let Some(&toc) = packet.first() else {
        return 0;
    };
    let config = toc >> 3;
    let frame_samples: u64 = match config {
        0 | 4 | 8 | 12 | 14 => 480, // 10 ms
        1 | 5 | 9 | 13 | 15 => 960, // 20 ms
        2 | 6 | 10 => 1920,         // 40 ms
        3 | 7 | 11 => 2880,         // 60 ms
        16 | 20 | 24 | 28 => 120,   // 2.5 ms
        17 | 21 | 25 | 29 => 240,   // 5 ms
        18 | 22 | 26 | 30 => 480,   // 10 ms
        19 | 23 | 27 | 31 => 960,   // 20 ms
        _ => 960,
    };
    let frame_count: u64 = match toc & 0x03 {
        0 => 1,
        1 | 2 => 2,
        3 => packet.get(1).map(|b| (b & 0x3F) as u64).unwrap_or(1),
        _ => 1,
    };
    frame_samples * frame_count
}

fn write_ogg_page(
    out: &mut Vec<u8>,
    header_type: u8,
    granule_pos: u64,
    serial: u32,
    seq: u32,
    packets: &[&[u8]],
) {
    let mut segments = Vec::new();
    for pkt in packets {
        let full = pkt.len() / 255;
        let rem = (pkt.len() % 255) as u8;
        segments.resize(segments.len() + full, 255);
        segments.push(rem);
    }

    let page_start = out.len();
    out.extend_from_slice(b"OggS");
    out.push(0); // stream structure version
    out.push(header_type);
    out.extend_from_slice(&granule_pos.to_le_bytes());
    out.extend_from_slice(&serial.to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    let crc_offset = out.len();
    out.extend_from_slice(&0u32.to_le_bytes()); // CRC placeholder
    out.push(segments.len() as u8);
    out.extend_from_slice(&segments);
    for pkt in packets {
        out.extend_from_slice(pkt);
    }

    let crc = ogg_crc32(&out[page_start..]);
    out[crc_offset..crc_offset + 4].copy_from_slice(&crc.to_le_bytes());
}

fn ogg_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            if (crc & 0x8000_0000) != 0 {
                crc = (crc << 1) ^ 0x04C1_1DB7;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_wav(pcm: &[u8], streaming_placeholder: bool) -> Vec<u8> {
        let fmt: [u8; 16] = [
            1, 0, // PCM
            1, 0, // mono
            0xc0, 0x5d, 0, 0, // 24000 Hz
            0x80, 0xbb, 0, 0, // byte rate 48000
            2, 0, // block align
            16, 0, // bits per sample
        ];
        let data_size = if streaming_placeholder {
            u32::MAX
        } else {
            pcm.len() as u32
        };
        let riff_size = if streaming_placeholder {
            u32::MAX
        } else {
            36 + pcm.len() as u32
        };

        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&riff_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        buf.extend_from_slice(&fmt);
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        buf.extend_from_slice(pcm);
        buf
    }

    #[test]
    fn infers_format_from_extension_and_overrides() {
        assert_eq!(
            resolve_audio_format(None, None, None).unwrap(),
            DEFAULT_MP3_FORMAT
        );
        assert_eq!(
            resolve_audio_format(None, None, Some(Path::new("out.mp3"))).unwrap(),
            DEFAULT_MP3_FORMAT
        );
        assert_eq!(
            resolve_audio_format(None, None, Some(Path::new("out.WAV"))).unwrap(),
            WAV_FORMAT
        );
        assert_eq!(
            resolve_audio_format(None, None, Some(Path::new("out.ogg"))).unwrap(),
            OGG_FORMAT
        );
        assert_eq!(
            resolve_audio_format(None, None, Some(Path::new("out.webm"))).unwrap(),
            WEBM_FORMAT
        );
        assert_eq!(
            resolve_audio_format(None, Some("WAVE"), Some(Path::new("out.mp3"))).unwrap(),
            WAV_FORMAT
        );
        assert_eq!(
            resolve_audio_format(
                Some("audio-24khz-96kbitrate-mono-mp3"),
                Some("wav"),
                Some(Path::new("out.ogg"))
            )
            .unwrap(),
            "audio-24khz-96kbitrate-mono-mp3"
        );
        assert!(resolve_audio_format(None, Some("flac"), None).is_err());
    }

    #[test]
    fn resolves_format_plan_with_pure_rust_transcode_routes() {
        let wav_plan = resolve_format_plan(None, None, Some(Path::new("a.wav"))).unwrap();
        assert_eq!(wav_plan.synthesis_format, HQ_MP3_FORMAT);
        assert_eq!(wav_plan.output_mode, OutputAssemblyMode::WavFromMp3);

        let ogg_plan = resolve_format_plan(None, None, Some(Path::new("a.ogg"))).unwrap();
        assert_eq!(ogg_plan.synthesis_format, WEBM_FORMAT);
        assert_eq!(ogg_plan.output_mode, OutputAssemblyMode::OggOpusFromWebm);

        let webm_plan = resolve_format_plan(None, None, Some(Path::new("a.webm"))).unwrap();
        assert_eq!(webm_plan.synthesis_format, WEBM_FORMAT);
        assert_eq!(webm_plan.output_mode, OutputAssemblyMode::WebmOpus);
    }

    #[test]
    fn merges_multiple_wav_chunks_and_rewrites_sizes() {
        let wav1 = make_wav(&[1, 2, 3, 4], false);
        let wav2 = make_wav(&[5, 6, 7, 8, 9, 10], true);
        let merged = merge_audio_chunks(&[wav1, wav2], WAV_FORMAT).unwrap();

        assert_eq!(&merged[0..4], b"RIFF");
        assert_eq!(&merged[8..12], b"WAVE");
        let riff_size = u32::from_le_bytes(merged[4..8].try_into().unwrap());
        assert_eq!(riff_size as usize, merged.len() - 8);

        assert_eq!(&merged[36..40], b"data");
        let data_size = u32::from_le_bytes(merged[40..44].try_into().unwrap());
        assert_eq!(data_size, 10);
        assert_eq!(&merged[44..], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn normalizes_single_streaming_wav_header() {
        let wav = make_wav(&[11, 22, 33, 44], true);
        let merged = merge_audio_chunks(&[wav], WAV_FORMAT).unwrap();
        let riff_size = u32::from_le_bytes(merged[4..8].try_into().unwrap());
        let data_size = u32::from_le_bytes(merged[40..44].try_into().unwrap());
        assert_eq!(riff_size, 40);
        assert_eq!(data_size, 4);
        assert_eq!(&merged[44..], &[11, 22, 33, 44]);
    }

    #[test]
    fn builds_valid_pcm_wav_header() {
        let wav = build_pcm_wav(1, 24_000, &[0, 0, 1, 0, 2, 0]).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 42);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 24_000);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 6);
    }

    #[test]
    fn merges_mp3_chunks_and_strips_subsequent_id3v2() {
        let chunk1 = vec![0xff, 0xfb, 0x90, 0x00];
        let mut chunk2 = vec![b'I', b'D', b'3', 3, 0, 0, 0, 0, 0, 2, 0xaa, 0xbb];
        chunk2.extend_from_slice(&[0xff, 0xfb, 0x90, 0x01]);
        let merged = merge_audio_chunks(&[chunk1, chunk2], DEFAULT_MP3_FORMAT).unwrap();
        assert_eq!(merged, vec![0xff, 0xfb, 0x90, 0x00, 0xff, 0xfb, 0x90, 0x01]);
    }

    #[test]
    fn remuxes_synthetic_webm_opus_to_valid_ogg_opus() {
        // Minimal WebM: Segment(unknown) -> Tracks -> TrackEntry -> CodecPrivate(OpusHead) + Cluster -> SimpleBlock.
        let mut opus_head = Vec::new();
        opus_head.extend_from_slice(b"OpusHead");
        opus_head.extend_from_slice(&[1, 1, 0x38, 0x01, 0xC0, 0x5D, 0, 0, 0, 0, 0]);
        assert_eq!(opus_head.len(), 19);

        let opus_frame = vec![0xD8, 0xFF, 0xFE, 0x01, 0x02]; // config 27 = 20ms (960 samples @ 48kHz)
        let mut simple_block_payload = vec![0x81, 0x00, 0x00, 0x00]; // track 1, ts 0, flags 0
        simple_block_payload.extend_from_slice(&opus_frame);

        let mut webm = Vec::new();
        // Segment (unknown size)
        webm.extend_from_slice(&[
            0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);
        // Tracks (size = TrackEntry(2 + 22) = 24 -> 0x80 | 24 = 0x98)
        webm.extend_from_slice(&[0x16, 0x54, 0xAE, 0x6B, 0x98]);
        // TrackEntry (size = CodecPrivate(2 + 1 + 19) = 22 -> 0x80 | 22 = 0x96)
        webm.extend_from_slice(&[0xAE, 0x96]);
        // CodecPrivate (0x63A2, size 19 -> 0x93)
        webm.extend_from_slice(&[0x63, 0xA2, 0x93]);
        webm.extend_from_slice(&opus_head);
        // Cluster (unknown size)
        webm.extend_from_slice(&[
            0x1F, 0x43, 0xB6, 0x75, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);
        // SimpleBlock (0xA3, size 9 -> 0x89)
        webm.extend_from_slice(&[0xA3, 0x80 | (simple_block_payload.len() as u8)]);
        webm.extend_from_slice(&simple_block_payload);

        let ogg = remux_webm_chunks_to_ogg_opus(&[webm.as_slice()]).unwrap();
        assert_eq!(&ogg[0..4], b"OggS");
        assert_eq!(ogg[5], 0x02); // BOS
        assert!(ogg.windows(8).any(|w| w == b"OpusHead"));
        assert!(ogg.windows(8).any(|w| w == b"OpusTags"));
        // Verify 3 OggS pages (OpusHead, OpusTags, audio EOS).
        let page_count = ogg.windows(4).filter(|w| *w == b"OggS").count();
        assert_eq!(page_count, 3);
        assert_eq!(opus_packet_samples_48k(&opus_frame), 960);
    }
}
