use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use anyhow::{Result, bail};
use crossterm::cursor::MoveUp;
use crossterm::execute;
use crossterm::style::{Attribute, Attributes, Color, ContentStyle};
use crossterm::terminal::{self, Clear, ClearType};
use msedge_tts::tts::AudioMetadata;

/// Terminal styling specification for `-i / --interactive[=MARKUP]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightStyle {
    pub style: ContentStyle,
}

impl Default for HighlightStyle {
    fn default() -> Self {
        Self {
            style: ContentStyle {
                foreground_color: None,
                background_color: None,
                underline_color: None,
                attributes: Attributes::from(Attribute::Reverse),
            },
        }
    }
}

impl HighlightStyle {
    /// Parse macOS `say -i` compatible markup:
    /// - empty or `smso` -> reverse video
    /// - `bold` -> bold
    /// - `smul` / `underline` -> underlined
    /// - `<color>` -> foreground color (`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`)
    /// - `<fg>/<bg>` or `/<bg>` or `<fg>/` -> foreground and/or background ANSI color
    pub fn parse(spec: &str) -> Result<Self> {
        let trimmed = spec.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("smso") {
            return Ok(Self::default());
        }

        if trimmed.eq_ignore_ascii_case("bold") {
            return Ok(Self {
                style: ContentStyle {
                    attributes: Attributes::from(Attribute::Bold),
                    ..ContentStyle::default()
                },
            });
        }

        if trimmed.eq_ignore_ascii_case("smul") || trimmed.eq_ignore_ascii_case("underline") {
            return Ok(Self {
                style: ContentStyle {
                    attributes: Attributes::from(Attribute::Underlined),
                    ..ContentStyle::default()
                },
            });
        }

        if let Some((fg_raw, bg_raw)) = trimmed.split_once('/') {
            if bg_raw.contains('/') {
                bail!("Invalid interactive markup '{spec}': expected 'fg/bg' with a single '/'");
            }
            let fg_str = fg_raw.trim();
            let bg_str = bg_raw.trim();
            if fg_str.is_empty() && bg_str.is_empty() {
                bail!(
                    "Invalid interactive markup '{spec}': specify at least a foreground or background color"
                );
            }
            let fg = if fg_str.is_empty() {
                None
            } else {
                Some(parse_ansi_color(fg_str, spec)?)
            };
            let bg = if bg_str.is_empty() {
                None
            } else {
                Some(parse_ansi_color(bg_str, spec)?)
            };
            return Ok(Self {
                style: ContentStyle {
                    foreground_color: fg,
                    background_color: bg,
                    ..ContentStyle::default()
                },
            });
        }

        let fg = parse_ansi_color(trimmed, spec)?;
        Ok(Self {
            style: ContentStyle {
                foreground_color: Some(fg),
                ..ContentStyle::default()
            },
        })
    }

    pub fn apply(&self, text: &str) -> String {
        self.style.apply(text).to_string()
    }
}

fn parse_ansi_color(token: &str, full_spec: &str) -> Result<Color> {
    match token.to_ascii_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "white" => Ok(Color::White),
        _ => bail!(
            "Invalid interactive markup '{full_spec}': unknown style or color '{token}'. Valid values: smso, bold, smul, black, red, green, yellow, blue, magenta, cyan, white, or fg/bg (e.g. /green, yellow/blue)"
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub offset: Duration,
    pub duration: Duration,
}

/// Align `msedge-tts` WordBoundary metadata entries to byte ranges inside `text`.
pub fn align_word_boundaries(text: &str, metadata: &[AudioMetadata]) -> Vec<WordSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    let text_lower = text.to_ascii_lowercase();

    for entry in metadata {
        if let Some(ref kind) = entry.metadata_type
            && !kind.eq_ignore_ascii_case("WordBoundary")
        {
            continue;
        }
        let Some(ref raw_word) = entry.text else {
            continue;
        };
        let word = raw_word.trim();
        if word.is_empty() {
            continue;
        }

        let offset = Duration::from_nanos(entry.offset.saturating_mul(100));
        let duration = Duration::from_nanos(entry.duration.saturating_mul(100));

        let remaining = &text[cursor..];
        let matched = if let Some(rel) = remaining.find(word) {
            Some((cursor + rel, cursor + rel + word.len()))
        } else if word.is_ascii() && text.is_char_boundary(cursor) {
            let word_lower = word.to_ascii_lowercase();
            text_lower[cursor..]
                .find(&word_lower)
                .map(|rel| (cursor + rel, cursor + rel + word.len()))
        } else {
            None
        };

        if let Some((start, end)) = matched {
            spans.push(WordSpan {
                byte_start: start,
                byte_end: end,
                offset,
                duration,
            });
            cursor = end;
        }
    }

    spans
}

/// Return the currently active word span index for `elapsed` playback time.
pub fn active_span_at(spans: &[WordSpan], elapsed: Duration) -> Option<usize> {
    if spans.is_empty() || elapsed < spans[0].offset {
        return None;
    }

    let mut active = 0usize;
    for (idx, span) in spans.iter().enumerate() {
        if elapsed >= span.offset {
            active = idx;
        } else {
            break;
        }
    }
    Some(active)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayLine {
    pub byte_start: usize,
    pub byte_end: usize,
}

/// Split text into display lines fitting within `max_cols` terminal columns while preserving
/// exact byte ranges into `text`.
pub fn split_display_lines(text: &str, max_cols: usize) -> Vec<DisplayLine> {
    let max_cols = max_cols.max(10);
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut col = 0usize;

    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            if idx > line_start {
                lines.push(DisplayLine {
                    byte_start: line_start,
                    byte_end: idx,
                });
            }
            line_start = idx + 1;
            col = 0;
            continue;
        }
        if ch == '\r' {
            continue;
        }

        let ch_width = char_display_width(ch);
        if col + ch_width > max_cols && idx > line_start {
            lines.push(DisplayLine {
                byte_start: line_start,
                byte_end: idx,
            });
            line_start = idx;
            col = ch_width;
        } else {
            col += ch_width;
        }
    }

    if line_start < text.len() {
        lines.push(DisplayLine {
            byte_start: line_start,
            byte_end: text.len(),
        });
    }

    lines
}

fn char_display_width(ch: char) -> usize {
    if ch.is_ascii() {
        1
    } else if is_wide_char(ch) {
        2
    } else {
        1
    }
}

fn is_wide_char(ch: char) -> bool {
    let c = ch as u32;
    matches!(
        c,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
            | 0x20000..=0x2FFFD
    )
}

/// Render a display line slice with an optional highlighted byte range `[hl_start, hl_end)`.
pub fn render_highlighted_line(
    full_text: &str,
    line: &DisplayLine,
    highlight_range: Option<(usize, usize)>,
    style: &HighlightStyle,
) -> String {
    let line_slice = &full_text[line.byte_start..line.byte_end];
    let Some((hl_start, hl_end)) = highlight_range else {
        return line_slice.to_owned();
    };

    let rel_start = hl_start
        .max(line.byte_start)
        .saturating_sub(line.byte_start);
    let rel_end = hl_end.min(line.byte_end).saturating_sub(line.byte_start);

    if rel_start >= rel_end || rel_end > line_slice.len() {
        return line_slice.to_owned();
    }

    let before = &line_slice[..rel_start];
    let target = &line_slice[rel_start..rel_end];
    let after = &line_slice[rel_end..];
    format!("{before}{}{after}", style.apply(target))
}

/// Progress bar renderer that writes strictly to `stderr`.
pub struct ProgressBar {
    enabled: bool,
    is_tty: bool,
}

impl ProgressBar {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            is_tty: io::stderr().is_terminal(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn update_synthesizing(&self, chunk_idx: usize, total_chunks: usize) {
        if !self.enabled {
            return;
        }
        let ratio = if total_chunks == 0 {
            1.0
        } else {
            chunk_idx as f64 / total_chunks as f64
        };
        let line = format!(
            "Synthesizing [{}/{}] {}",
            chunk_idx,
            total_chunks,
            format_bar(ratio, 24)
        );
        self.write_stderr_line(&line, false);
    }

    pub fn clear_line(&self) {
        if !self.enabled || !self.is_tty {
            return;
        }
        let mut stderr = io::stderr();
        let _ = write!(stderr, "\r");
        let _ = execute!(stderr, Clear(ClearType::UntilNewLine));
        let _ = stderr.flush();
    }

    pub fn finish_message(&self, msg: &str) {
        if !self.enabled {
            return;
        }
        self.write_stderr_line(msg, true);
    }

    pub fn format_playback_line(
        chunk_idx: usize,
        total_chunks: usize,
        pos: Duration,
        total: Option<Duration>,
    ) -> String {
        let ratio = match total {
            Some(dur) if !dur.is_zero() => (pos.as_secs_f64() / dur.as_secs_f64()).clamp(0.0, 1.0),
            _ => 0.0,
        };
        let timing = match total {
            Some(dur) => format!(
                "{:.1}s / {:.1}s",
                pos.as_secs_f64().min(dur.as_secs_f64()),
                dur.as_secs_f64()
            ),
            None => format!("{:.1}s", pos.as_secs_f64()),
        };
        if total_chunks > 1 {
            format!(
                "Playing [{}/{}] {} ({timing})",
                chunk_idx,
                total_chunks,
                format_bar(ratio, 24)
            )
        } else {
            format!("Playing {} ({timing})", format_bar(ratio, 24))
        }
    }

    fn write_stderr_line(&self, text: &str, newline: bool) {
        let mut stderr = io::stderr();
        if self.is_tty {
            let _ = write!(stderr, "\r");
            let _ = execute!(stderr, Clear(ClearType::UntilNewLine));
            if newline {
                let _ = writeln!(stderr, "{text}");
            } else {
                let _ = write!(stderr, "{text}");
            }
        } else if newline {
            let _ = writeln!(stderr, "{text}");
        }
        let _ = stderr.flush();
    }
}

pub fn format_bar(ratio: f64, width: usize) -> String {
    let clamped = ratio.clamp(0.0, 1.0);
    let filled = (clamped * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    let pct = (clamped * 100.0).round() as u32;
    format!("[{}{}] {:>3}%", "#".repeat(filled), "-".repeat(empty), pct)
}

pub fn should_redraw_text(
    has_rendered: bool,
    last_rendered_span: Option<usize>,
    active_idx: Option<usize>,
) -> bool {
    !has_rendered || active_idx != last_rendered_span
}

/// Coordinates real-time `-i` highlighting on `stdout` and `--progress` on `stderr`
/// during chunk playback without polluting `stdout` or clobbering terminal lines.
pub struct PlaybackDisplay<'a> {
    text: &'a str,
    lines: Vec<DisplayLine>,
    spans: Vec<WordSpan>,
    style: Option<&'a HighlightStyle>,
    progress: &'a ProgressBar,
    chunk_idx: usize,
    total_chunks: usize,
    stdout_tty: bool,
    stderr_tty: bool,
    current_line_idx: usize,
    has_rendered: bool,
    last_rendered_span: Option<usize>,
    two_line_active: bool,
    fallback_highlighted: bool,
}

impl<'a> PlaybackDisplay<'a> {
    pub fn new(
        text: &'a str,
        metadata: &[AudioMetadata],
        style: Option<&'a HighlightStyle>,
        progress: &'a ProgressBar,
        chunk_idx: usize,
        total_chunks: usize,
    ) -> Self {
        let cols = terminal::size()
            .map(|(w, _)| (w as usize).saturating_sub(2))
            .unwrap_or(78)
            .max(20);
        let lines = split_display_lines(text, cols);
        let spans = align_word_boundaries(text, metadata);
        let stdout_tty = io::stdout().is_terminal();
        let stderr_tty = io::stderr().is_terminal();

        if style.is_some() && progress.is_enabled() {
            progress.clear_line();
        }

        Self {
            text,
            lines,
            spans,
            style,
            progress,
            chunk_idx,
            total_chunks,
            stdout_tty,
            stderr_tty,
            current_line_idx: 0,
            has_rendered: false,
            last_rendered_span: None,
            two_line_active: false,
            fallback_highlighted: false,
        }
    }

    pub fn update(&mut self, elapsed: Duration, total: Option<Duration>) {
        let progress_line = if self.progress.is_enabled() {
            Some(ProgressBar::format_playback_line(
                self.chunk_idx,
                self.total_chunks,
                elapsed,
                total,
            ))
        } else {
            None
        };

        let Some(style) = self.style else {
            if let Some(ref line) = progress_line {
                self.progress.write_stderr_line(line, false);
            }
            return;
        };

        if self.lines.is_empty() {
            if let Some(ref line) = progress_line {
                self.progress.write_stderr_line(line, false);
            }
            return;
        }

        // If no WordBoundary metadata is available, highlight the whole active chunk/line as fallback.
        if self.spans.is_empty() {
            if !self.fallback_highlighted {
                self.fallback_highlighted = true;
                let line_count = self.lines.len();
                for i in 0..line_count {
                    let line = self.lines[i];
                    let is_last = i + 1 == line_count;
                    let rendered = render_highlighted_line(
                        self.text,
                        &line,
                        Some((line.byte_start, line.byte_end)),
                        style,
                    );
                    if is_last {
                        self.draw_active_line(&rendered, progress_line.as_deref());
                        self.has_rendered = true;
                    } else {
                        self.commit_line(&rendered);
                    }
                }
            } else if let Some(ref p_line) = progress_line {
                self.update_progress_line(p_line);
            }
            return;
        }

        let active_idx = active_span_at(&self.spans, elapsed);
        if !should_redraw_text(self.has_rendered, self.last_rendered_span, active_idx) {
            if let Some(ref p_line) = progress_line {
                self.update_progress_line(p_line);
            }
            return;
        }

        let target_line_idx = active_idx
            .and_then(|idx| {
                let start = self.spans[idx].byte_start;
                self.lines
                    .iter()
                    .position(|l| start >= l.byte_start && start < l.byte_end)
            })
            .unwrap_or(self.current_line_idx);

        // Commit completed display lines as plain text when advancing to a later line.
        while self.current_line_idx < target_line_idx {
            let finished = &self.lines[self.current_line_idx];
            let plain = &self.text[finished.byte_start..finished.byte_end];
            self.commit_line(plain);
            self.current_line_idx += 1;
        }

        let current_line = &self.lines[self.current_line_idx];
        let hl_range = active_idx.map(|idx| {
            let s = &self.spans[idx];
            (s.byte_start, s.byte_end)
        });
        let rendered = render_highlighted_line(self.text, current_line, hl_range, style);
        self.draw_active_line(&rendered, progress_line.as_deref());
        self.has_rendered = true;
        self.last_rendered_span = active_idx;
    }

    pub fn finish(&mut self, total: Option<Duration>) {
        let final_progress = if self.progress.is_enabled() {
            let dur = total.unwrap_or(Duration::ZERO);
            Some(ProgressBar::format_playback_line(
                self.chunk_idx,
                self.total_chunks,
                dur,
                total.or(Some(dur)),
            ))
        } else {
            None
        };

        if self.style.is_some() && !self.lines.is_empty() {
            // Unhighlight the final active line and print any remaining lines cleanly.
            if self.fallback_highlighted {
                if let Some(last) = self.lines.last() {
                    let plain = &self.text[last.byte_start..last.byte_end];
                    self.commit_line(plain);
                }
            } else {
                while self.current_line_idx < self.lines.len() {
                    let line = &self.lines[self.current_line_idx];
                    let plain = &self.text[line.byte_start..line.byte_end];
                    self.commit_line(plain);
                    self.current_line_idx += 1;
                }
            }

            if let Some(ref p_line) = final_progress {
                self.progress.write_stderr_line(p_line, true);
            }
        } else if let Some(ref p_line) = final_progress {
            let is_last_chunk = self.chunk_idx >= self.total_chunks;
            self.progress.write_stderr_line(p_line, is_last_chunk);
        }
    }

    fn draw_active_line(&mut self, text_line: &str, progress_line: Option<&str>) {
        let mut stdout = io::stdout();
        let mut stderr = io::stderr();

        if self.two_line_active && self.stdout_tty && self.stderr_tty {
            let _ = execute!(stderr, MoveUp(1));
        }

        if self.stdout_tty {
            let _ = write!(stdout, "\r");
            let _ = execute!(stdout, Clear(ClearType::UntilNewLine));
            let _ = write!(stdout, "{text_line}");
            let _ = stdout.flush();
        } else {
            let _ = writeln!(stdout, "{text_line}");
            let _ = stdout.flush();
        }

        if let Some(p_line) = progress_line {
            if self.stdout_tty && self.stderr_tty {
                let _ = write!(stderr, "\n\r");
                let _ = execute!(stderr, Clear(ClearType::UntilNewLine));
                let _ = write!(stderr, "{p_line}");
                let _ = stderr.flush();
                self.two_line_active = true;
            } else {
                self.progress.write_stderr_line(p_line, false);
            }
        }
    }

    fn update_progress_line(&self, progress_line: &str) {
        if self.two_line_active {
            let mut stderr = io::stderr();
            if self.stderr_tty {
                let _ = write!(stderr, "\r");
                let _ = execute!(stderr, Clear(ClearType::UntilNewLine));
                let _ = write!(stderr, "{progress_line}");
                let _ = stderr.flush();
            }
        } else {
            self.progress.write_stderr_line(progress_line, false);
        }
    }

    fn commit_line(&mut self, plain_line: &str) {
        let mut stdout = io::stdout();
        let mut stderr = io::stderr();

        if self.two_line_active && self.stdout_tty && self.stderr_tty {
            let _ = write!(stderr, "\r");
            let _ = execute!(stderr, Clear(ClearType::UntilNewLine));
            let _ = execute!(stderr, MoveUp(1));
            let _ = stderr.flush();
            self.two_line_active = false;
        }

        if self.stdout_tty {
            let _ = write!(stdout, "\r");
            let _ = execute!(stdout, Clear(ClearType::UntilNewLine));
            let _ = writeln!(stdout, "{plain_line}");
        } else {
            let _ = writeln!(stdout, "{plain_line}");
        }
        let _ = stdout.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_meta(word: &str, offset_ms: u64, duration_ms: u64) -> AudioMetadata {
        AudioMetadata {
            metadata_type: Some("WordBoundary".to_owned()),
            offset: offset_ms * 10_000,
            duration: duration_ms * 10_000,
            text: Some(word.to_owned()),
            length: word.chars().count() as u64,
            boundary_type: Some("WordBoundary".to_owned()),
        }
    }

    #[test]
    fn parses_valid_interactive_markups() {
        assert_eq!(
            HighlightStyle::parse("").unwrap(),
            HighlightStyle::default()
        );
        assert_eq!(
            HighlightStyle::parse("smso").unwrap(),
            HighlightStyle::default()
        );

        let bold = HighlightStyle::parse("bold").unwrap();
        assert!(bold.style.attributes.has(Attribute::Bold));

        let smul = HighlightStyle::parse("smul").unwrap();
        assert!(smul.style.attributes.has(Attribute::Underlined));

        let green = HighlightStyle::parse("green").unwrap();
        assert_eq!(green.style.foreground_color, Some(Color::Green));
        assert_eq!(green.style.background_color, None);

        let bg_only = HighlightStyle::parse("/green").unwrap();
        assert_eq!(bg_only.style.foreground_color, None);
        assert_eq!(bg_only.style.background_color, Some(Color::Green));

        let fg_bg = HighlightStyle::parse("yellow/blue").unwrap();
        assert_eq!(fg_bg.style.foreground_color, Some(Color::Yellow));
        assert_eq!(fg_bg.style.background_color, Some(Color::Blue));
    }

    #[test]
    fn rejects_invalid_interactive_markups() {
        assert!(HighlightStyle::parse("/").is_err());
        assert!(HighlightStyle::parse("purple").is_err());
        assert!(HighlightStyle::parse("red/green/blue").is_err());
        assert!(HighlightStyle::parse("red/orange").is_err());
    }

    #[test]
    fn aligns_repeated_words_and_cjk_boundaries() {
        let text = "你好世界 你好";
        let meta = vec![
            make_meta("你好", 100, 300),
            make_meta("世界", 400, 300),
            make_meta("你好", 800, 300),
        ];
        let spans = align_word_boundaries(text, &meta);
        assert_eq!(spans.len(), 3);
        assert_eq!(&text[spans[0].byte_start..spans[0].byte_end], "你好");
        assert_eq!(&text[spans[1].byte_start..spans[1].byte_end], "世界");
        assert_eq!(&text[spans[2].byte_start..spans[2].byte_end], "你好");
        assert!(spans[2].byte_start > spans[1].byte_end);

        assert_eq!(active_span_at(&spans, Duration::from_millis(50)), None);
        assert_eq!(active_span_at(&spans, Duration::from_millis(100)), Some(0));
        assert_eq!(active_span_at(&spans, Duration::from_millis(500)), Some(1));
        assert_eq!(active_span_at(&spans, Duration::from_millis(900)), Some(2));
    }

    #[test]
    fn wraps_display_lines_accounting_for_wide_cjk() {
        let text = "一二三四五六七八九十";
        let lines = split_display_lines(text, 10);
        assert_eq!(lines.len(), 2);
        assert_eq!(&text[lines[0].byte_start..lines[0].byte_end], "一二三四五");
        assert_eq!(&text[lines[1].byte_start..lines[1].byte_end], "六七八九十");
    }

    #[test]
    fn formats_progress_bar() {
        assert_eq!(format_bar(0.0, 10), "[----------]   0%");
        assert_eq!(format_bar(0.5, 10), "[#####-----]  50%");
        assert_eq!(format_bar(1.0, 10), "[##########] 100%");
    }

    #[test]
    fn skips_redundant_text_redraws_on_unchanged_span() {
        // Initial render must draw even when active_idx is None.
        assert!(should_redraw_text(false, None, None));
        assert!(should_redraw_text(false, None, Some(0)));
        // Repeated ticks on the same span (including None) must skip text redraw.
        assert!(!should_redraw_text(true, None, None));
        assert!(!should_redraw_text(true, Some(0), Some(0)));
        // Advancing to a new span must redraw.
        assert!(should_redraw_text(true, None, Some(0)));
        assert!(should_redraw_text(true, Some(0), Some(1)));
    }
}
