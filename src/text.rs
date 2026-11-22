use std::fs;
use std::io::{self, IsTerminal, Read};

use anyhow::{Context, Result};

use crate::cli::Cli;

pub const DEFAULT_MAX_CHUNK_CHARS: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInput {
    InteractiveTty,
    FullText(String),
}

/// Resolve input text source following priority:
/// 1. `-f <FILE>` (`-` reads stdin)
/// 2. positional `MESSAGE` arguments (joined by spaces)
/// 3. stdin (interactive line-by-line if TTY, full text if piped)
pub fn resolve_input(cli: &Cli) -> Result<TextInput> {
    if let Some(ref input_file) = cli.input_file {
        if input_file == "-" {
            return read_stdin_input();
        }
        let content = fs::read_to_string(input_file)
            .with_context(|| format!("Failed to read input file '{input_file}'"))?;
        return Ok(TextInput::FullText(strip_bom(&content).to_owned()));
    }

    if !cli.message.is_empty() {
        let joined = cli.message.join(" ");
        return Ok(TextInput::FullText(strip_bom(&joined).to_owned()));
    }

    read_stdin_input()
}

fn read_stdin_input() -> Result<TextInput> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        Ok(TextInput::InteractiveTty)
    } else {
        let mut buf = String::new();
        stdin
            .lock()
            .read_to_string(&mut buf)
            .context("Failed to read text from standard input")?;
        Ok(TextInput::FullText(strip_bom(&buf).to_owned()))
    }
}

pub fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

/// Escape text so it can be safely embedded inside Edge TTS SSML `<prosody>...</prosody>`.
pub fn escape_ssml_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\t' | '\n' | '\r' => escaped.push(ch),
            c if (c as u32) < 0x20 => {}
            c => escaped.push(c),
        }
    }
    escaped
}

/// Smart-chunk long text along paragraph and sentence boundaries (`。！？.!?\n`)
/// so every chunk stays within `max_chars` Unicode characters.
pub fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0usize;

    for raw_line in trimmed.split('\n') {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let line_chars = line.chars().count();
        if line_chars <= max_chars {
            let sep_len = if current.is_empty() { 0 } else { 1 };
            if current_chars + sep_len + line_chars <= max_chars {
                if sep_len == 1 {
                    current.push('\n');
                    current_chars += 1;
                }
                current.push_str(line);
                current_chars += line_chars;
            } else {
                flush_chunk(&mut chunks, &mut current, &mut current_chars);
                current.push_str(line);
                current_chars = line_chars;
            }
            continue;
        }

        // Line itself exceeds max_chars: flush pending buffer and split by sentences.
        flush_chunk(&mut chunks, &mut current, &mut current_chars);
        for piece in split_long_paragraph(line, max_chars) {
            let piece_chars = piece.chars().count();
            if current_chars + piece_chars <= max_chars {
                current.push_str(&piece);
                current_chars += piece_chars;
            } else {
                flush_chunk(&mut chunks, &mut current, &mut current_chars);
                current.push_str(&piece);
                current_chars = piece_chars;
            }
        }
    }

    flush_chunk(&mut chunks, &mut current, &mut current_chars);
    chunks
}

fn flush_chunk(chunks: &mut Vec<String>, current: &mut String, current_chars: &mut usize) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        chunks.push(trimmed.to_owned());
    }
    current.clear();
    *current_chars = 0;
}

fn split_long_paragraph(paragraph: &str, max_chars: usize) -> Vec<String> {
    let sentences = split_after_delimiters(paragraph, is_sentence_terminator);
    let mut pieces = Vec::new();

    for sentence in sentences {
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }
        if sentence.chars().count() <= max_chars {
            pieces.push(sentence.to_owned());
        } else {
            // Try secondary clause punctuation first.
            let clauses = split_after_delimiters(sentence, is_clause_delimiter);
            for clause in clauses {
                let clause = clause.trim();
                if clause.is_empty() {
                    continue;
                }
                if clause.chars().count() <= max_chars {
                    pieces.push(clause.to_owned());
                } else {
                    pieces.extend(split_by_whitespace_or_chars(clause, max_chars));
                }
            }
        }
    }

    pieces
}

fn is_sentence_terminator(ch: char) -> bool {
    matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '；' | ';')
}

fn is_clause_delimiter(ch: char) -> bool {
    matches!(ch, '，' | ',' | '、' | '：' | ':')
}

fn split_after_delimiters<F>(text: &str, is_delim: F) -> Vec<&str>
where
    F: Fn(char) -> bool,
{
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        if is_delim(ch) {
            let mut end = idx + ch.len_utf8();
            // Keep consecutive closing punctuation or quotes attached to the sentence.
            while let Some(&(next_idx, next_ch)) = chars.peek() {
                if is_delim(next_ch)
                    || matches!(next_ch, '"' | '\'' | '”' | '’' | '」' | '』' | ')' | '）')
                {
                    end = next_idx + next_ch.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            parts.push(&text[start..end]);
            start = end;
        }
    }

    if start < text.len() {
        parts.push(&text[start..]);
    }
    parts
}

fn split_by_whitespace_or_chars(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0usize;

    for word in text.split_whitespace() {
        let word_chars = word.chars().count();
        if word_chars > max_chars {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current_chars = 0;
            }
            let mut slice = String::new();
            let mut count = 0usize;
            for ch in word.chars() {
                slice.push(ch);
                count += 1;
                if count == max_chars {
                    out.push(std::mem::take(&mut slice));
                    count = 0;
                }
            }
            if !slice.is_empty() {
                current = slice;
                current_chars = count;
            }
            continue;
        }

        let sep = if current.is_empty() { 0 } else { 1 };
        if current_chars + sep + word_chars <= max_chars {
            if sep == 1 {
                current.push(' ');
                current_chars += 1;
            }
            current.push_str(word);
            current_chars += word_chars;
        } else {
            out.push(std::mem::take(&mut current));
            current.push_str(word);
            current_chars = word_chars;
        }
    }

    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_xml_and_control_chars() {
        assert_eq!(
            escape_ssml_text("Tom & Jerry <3 > 0\x00\x07\nOK"),
            "Tom &amp; Jerry &lt;3 &gt; 0\nOK"
        );
    }

    #[test]
    fn strips_utf8_bom() {
        assert_eq!(strip_bom("\u{feff}你好"), "你好");
        assert_eq!(strip_bom("Hello"), "Hello");
    }

    #[test]
    fn empty_or_whitespace_produces_no_chunks() {
        assert!(chunk_text("", 2000).is_empty());
        assert!(chunk_text("   \n\t\r\n  ", 2000).is_empty());
    }

    #[test]
    fn chunks_cjk_by_sentence_boundaries() {
        let text = "這是第一句話。這是第二句話！這是第三句話？";
        let chunks = chunk_text(text, 14);
        assert_eq!(
            chunks,
            vec!["這是第一句話。這是第二句話！", "這是第三句話？"]
        );
        assert!(chunks.iter().all(|c| c.chars().count() <= 14));
    }

    #[test]
    fn chunks_ascii_paragraphs_and_sentences() {
        let text = "First sentence. Second sentence!\n\nThird sentence? Fourth sentence.";
        let chunks = chunk_text(text, 35);
        assert_eq!(
            chunks,
            vec![
                "First sentence. Second sentence!",
                "Third sentence? Fourth sentence."
            ]
        );
    }

    #[test]
    fn hard_splits_unpunctuated_long_cjk_text() {
        let text = "你".repeat(45);
        let chunks = chunk_text(&text, 20);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].chars().count(), 20);
        assert_eq!(chunks[1].chars().count(), 20);
        assert_eq!(chunks[2].chars().count(), 5);
    }
}
