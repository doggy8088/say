use std::net::TcpStream;
use std::time::Duration;

use anyhow::{Context, Result};
use msedge_tts::tts::client::{MSEdgeTTSClient, connect};
use msedge_tts::tts::{AudioMetadata, SpeechConfig};

use crate::text::escape_ssml_text;

#[derive(Debug)]
pub struct SynthesizedChunk {
    pub text: String,
    pub audio_bytes: Vec<u8>,
    pub audio_metadata: Vec<AudioMetadata>,
    pub audio_format: String,
}

impl SynthesizedChunk {
    /// Estimate total duration from WordBoundary metadata (100ns ticks) as a fallback
    /// when a stream decoder cannot compute exact duration from container headers.
    pub fn metadata_duration(&self) -> Option<Duration> {
        let max_ticks = self
            .audio_metadata
            .iter()
            .map(|m| m.offset.saturating_add(m.duration))
            .max()?;
        if max_ticks == 0 {
            None
        } else {
            Some(Duration::from_nanos(max_ticks.saturating_mul(100)))
        }
    }
}

pub struct TtsClient {
    inner: Option<MSEdgeTTSClient<TcpStream>>,
}

impl TtsClient {
    pub fn new() -> Self {
        Self { inner: None }
    }

    fn ensure_connected(&mut self) -> Result<&mut MSEdgeTTSClient<TcpStream>> {
        if self.inner.is_none() {
            let client =
                connect().context("Failed to connect to Microsoft Edge TTS WebSocket service")?;
            self.inner = Some(client);
        }
        Ok(self.inner.as_mut().expect("client just initialized"))
    }

    /// Synthesize a text chunk, reconnecting once automatically if an idle WebSocket closed.
    pub fn synthesize(&mut self, text: &str, config: &SpeechConfig) -> Result<SynthesizedChunk> {
        let escaped = escape_ssml_text(text);
        let attempt = self
            .ensure_connected()
            .and_then(|client| client.synthesize(&escaped, config).map_err(Into::into));

        let result = match attempt {
            Ok(audio) => audio,
            Err(_) => {
                // Drop stale socket and retry once with a fresh connection.
                self.inner = None;
                let client = self.ensure_connected()?;
                client.synthesize(&escaped, config).with_context(|| {
                    format!(
                        "Failed to synthesize speech with voice '{}'",
                        config.voice_name
                    )
                })?
            }
        };

        Ok(SynthesizedChunk {
            text: text.to_owned(),
            audio_bytes: result.audio_bytes,
            audio_metadata: result.audio_metadata,
            audio_format: result.audio_format,
        })
    }
}

impl Default for TtsClient {
    fn default() -> Self {
        Self::new()
    }
}
