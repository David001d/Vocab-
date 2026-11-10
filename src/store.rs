//! Persistence: a plain tab-separated text file, one card per line.
//!
//! Format: `word \t meaning \t box \t due_day \t correct \t incorrect`
//!
//! A text format was chosen deliberately: it is human-readable, diffs well
//! in git, and needs no dependencies. Words and meanings may not contain
//! tabs or newlines; `add` rejects them so the file can never be corrupted.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::card::{Card, MAX_BOX};

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    /// A line in the file could not be parsed. Holds the 1-based line number.
    Parse { line: usize, reason: String },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "I/O error: {e}"),
            StoreError::Parse { line, reason } => write!(f, "line {line}: {reason}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

/// Location of the vocabulary file.
///
/// `VOCAB_FILE` overrides everything; otherwise `~/.vocab.tsv`, falling back
/// to `./vocab.tsv` if no home directory can be determined.
pub fn default_path() -> PathBuf {
    if let Some(p) = std::env::var_os("VOCAB_FILE") {
        return PathBuf::from(p);
    }
    match std::env::var_os("HOME") {
        Some(home) => Path::new(&home).join(".vocab.tsv"),
        None => PathBuf::from("vocab.tsv"),
    }
}

/// Load all cards. A missing file is treated as an empty deck.
pub fn load(path: &Path) -> Result<Vec<Card>, StoreError> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    parse(&text)
}

/// Write all cards, replacing the file atomically (write temp, then rename)
/// so a crash mid-write can't leave a half-written deck.
pub fn save(path: &Path, cards: &[Card]) -> Result<(), StoreError> {
    let tmp = path.with_extension("tsv.tmp");
    fs::write(&tmp, serialize(cards))?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn serialize(cards: &[Card]) -> String {
    let mut out = String::new();
    for c in cards {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            c.word, c.meaning, c.box_idx, c.due_day, c.correct, c.incorrect
        ));
    }
    out
}

fn parse(text: &str) -> Result<Vec<Card>, StoreError> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| parse_line(l).map_err(|reason| StoreError::Parse { line: i + 1, reason }))
        .collect()
}

fn parse_line(line: &str) -> Result<Card, String> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != 6 {
        return Err(format!("expected 6 tab-separated fields, found {}", fields.len()));
    }
    let num = |idx: usize, name: &str| -> Result<u32, String> {
        fields[idx]
            .parse::<u32>()
            .map_err(|_| format!("{name} is not a number: {:?}", fields[idx]))
    };
    let box_idx = num(2, "box")?;
    if box_idx > MAX_BOX as u32 {
        return Err(format!("box {box_idx} exceeds maximum {MAX_BOX}"));
    }
    Ok(Card {
        word: fields[0].to_string(),
        meaning: fields[1].to_string(),
        box_idx: box_idx as u8,
        due_day: num(3, "due_day")?,
        correct: num(4, "correct")?,
        incorrect: num(5, "incorrect")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_cards() {
        let cards = vec![
            Card::new("haus", "house", 10),
            Card {
                word: "katze".into(),
                meaning: "cat".into(),
                box_idx: 3,
                due_day: 42,
                correct: 5,
                incorrect: 1,
            },
        ];
        assert_eq!(parse(&serialize(&cards)).unwrap(), cards);
    }

    #[test]
    fn empty_and_blank_lines_are_ignored() {
        assert!(parse("").unwrap().is_empty());
        assert!(parse("\n\n  \n").unwrap().is_empty());
    }

    #[test]
    fn reports_line_number_on_bad_input() {
        let err = parse("a\tb\t0\t0\t0\t0\nbroken line\n").unwrap_err();
        match err {
            StoreError::Parse { line, .. } => assert_eq!(line, 2),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_out_of_range_box() {
        assert!(parse("a\tb\t9\t0\t0\t0\n").is_err());
    }

    #[test]
    fn missing_file_is_empty_deck() {
        let p = std::env::temp_dir().join("vocab-test-does-not-exist.tsv");
        assert!(load(&p).unwrap().is_empty());
    }
}
