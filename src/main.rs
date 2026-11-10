//! `vocab` — a tiny command-line vocabulary trainer.
//!
//! Usage:
//!   vocab add <word> <meaning...>   add a card
//!   vocab remove <word>             delete a card
//!   vocab list                      show all cards
//!   vocab quiz [N]                  review up to N due cards (default 10)
//!   vocab stats                     show progress summary
//!
//! The deck lives in `$VOCAB_FILE` or `~/.vocab.tsv`.

mod card;
mod store;

use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use card::{Card, MAX_BOX};

const USAGE: &str = "\
vocab — command-line vocabulary trainer

USAGE:
  vocab add <word> <meaning...>   add a card
  vocab remove <word>             delete a card
  vocab list                      show all cards
  vocab quiz [N]                  review up to N due cards (default 10)
  vocab stats                     show progress summary

The deck is stored in $VOCAB_FILE or ~/.vocab.tsv.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first() else {
        println!("{USAGE}");
        return Ok(());
    };
    let path = store::default_path();
    let mut cards = store::load(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let today = today();

    match cmd.as_str() {
        "add" => {
            let (word, meaning) = parse_add_args(&args[1..])?;
            add(&mut cards, word, meaning, today)?;
            store::save(&path, &cards).map_err(|e| e.to_string())?;
            println!("added {word}");
        }
        "remove" => {
            let word = args.get(1).ok_or("usage: vocab remove <word>")?;
            let before = cards.len();
            cards.retain(|c| !c.word.eq_ignore_ascii_case(word));
            if cards.len() == before {
                return Err(format!("no card for {word:?}"));
            }
            store::save(&path, &cards).map_err(|e| e.to_string())?;
            println!("removed {word}");
        }
        "list" => list(&cards, today),
        "quiz" => {
            let limit = match args.get(1) {
                Some(n) => n.parse::<usize>().map_err(|_| format!("not a number: {n}"))?,
                None => 10,
            };
            quiz(&mut cards, limit, today)?;
            store::save(&path, &cards).map_err(|e| e.to_string())?;
        }
        "stats" => stats(&cards, today),
        "help" | "-h" | "--help" => println!("{USAGE}"),
        other => return Err(format!("unknown command {other:?}\n\n{USAGE}")),
    }
    Ok(())
}

/// Days since the Unix epoch, in UTC.
fn today() -> u32 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before 1970")
        .as_secs();
    (secs / 86_400) as u32
}

fn parse_add_args(rest: &[String]) -> Result<(&str, String), String> {
    let [word, meaning @ ..] = rest else {
        return Err("usage: vocab add <word> <meaning...>".into());
    };
    if meaning.is_empty() {
        return Err("usage: vocab add <word> <meaning...>".into());
    }
    Ok((word.as_str(), meaning.join(" ")))
}

fn add(cards: &mut Vec<Card>, word: &str, meaning: String, today: u32) -> Result<(), String> {
    let word = word.trim();
    let meaning = meaning.trim().to_string();
    if word.is_empty() || meaning.is_empty() {
        return Err("word and meaning must not be empty".into());
    }
    for s in [word, meaning.as_str()] {
        if s.contains(['\t', '\n']) {
            return Err("word and meaning must not contain tabs or newlines".into());
        }
    }
    if cards.iter().any(|c| c.word.eq_ignore_ascii_case(word)) {
        return Err(format!("{word:?} already exists (remove it first to change it)"));
    }
    cards.push(Card::new(word, meaning, today));
    Ok(())
}

fn list(cards: &[Card], today: u32) {
    if cards.is_empty() {
        println!("no cards yet — try: vocab add <word> <meaning>");
        return;
    }
    let width = cards.iter().map(|c| c.word.chars().count()).max().unwrap_or(0);
    for c in cards {
        let due = if c.is_due(today) { " due" } else { "" };
        let acc = match c.accuracy() {
            Some(a) => format!(" {:.0}%", a * 100.0),
            None => String::new(),
        };
        println!("{:<width$}  {}  [box {}{acc}]{due}", c.word, c.meaning, c.box_idx);
    }
}

fn stats(cards: &[Card], today: u32) {
    let due = cards.iter().filter(|c| c.is_due(today)).count();
    println!("cards: {}   due today: {due}", cards.len());
    let mut per_box = [0usize; MAX_BOX as usize + 1];
    for c in cards {
        per_box[c.box_idx as usize] += 1;
    }
    for (i, n) in per_box.iter().enumerate() {
        println!("  box {i}: {n}");
    }
    let (correct, incorrect) = cards
        .iter()
        .fold((0, 0), |(c, i), card| (c + card.correct, i + card.incorrect));
    if correct + incorrect > 0 {
        let pct = 100.0 * correct as f64 / (correct + incorrect) as f64;
        println!("overall accuracy: {pct:.0}% ({correct}/{})", correct + incorrect);
    }
}

fn quiz(cards: &mut [Card], limit: usize, today: u32) -> Result<(), String> {
    let mut due: Vec<usize> = (0..cards.len()).filter(|&i| cards[i].is_due(today)).collect();
    if due.is_empty() {
        println!("nothing due — come back tomorrow");
        return Ok(());
    }
    shuffle(&mut due, today as u64 ^ SystemTime::now().elapsed_nanos());
    due.truncate(limit);

    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();
    let total = due.len();
    let mut known = 0;

    for (n, &i) in due.iter().enumerate() {
        let card = &mut cards[i];
        println!("\n[{}/{total}] {}", n + 1, card.word);
        print!("  (press Enter to reveal) ");
        io::stdout().flush().ok();
        if !read_line(&mut input, &mut line)? {
            break; // EOF: stop quietly, keep progress so far
        }
        println!("  → {}", card.meaning);
        let answer = loop {
            print!("  did you know it? [y/n] ");
            io::stdout().flush().ok();
            if !read_line(&mut input, &mut line)? {
                return finish(known, n);
            }
            match line.trim().to_ascii_lowercase().as_str() {
                "y" | "yes" => break true,
                "n" | "no" => break false,
                _ => continue,
            }
        };
        card.review(answer, today);
        if answer {
            known += 1;
        }
    }
    finish(known, total)
}

fn finish(known: usize, answered: usize) -> Result<(), String> {
    println!("\ndone: {known}/{answered} known");
    Ok(())
}

/// Read one line into `buf`. Returns `false` on EOF.
fn read_line(input: &mut impl BufRead, buf: &mut String) -> Result<bool, String> {
    buf.clear();
    let n = input.read_line(buf).map_err(|e| e.to_string())?;
    Ok(n > 0)
}

/// Fisher–Yates with a small xorshift generator. Avoids pulling in `rand`
/// for what is only cosmetic ordering.
fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut state = seed | 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for i in (1..items.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}

trait ElapsedNanos {
    fn elapsed_nanos(&self) -> u64;
}

impl ElapsedNanos for SystemTime {
    fn elapsed_nanos(&self) -> u64 {
        self.duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_rejects_duplicates_case_insensitively() {
        let mut cards = Vec::new();
        add(&mut cards, "Haus", "house".into(), 0).unwrap();
        assert!(add(&mut cards, "haus", "building".into(), 0).is_err());
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn add_rejects_tabs_and_blank_input() {
        let mut cards = Vec::new();
        assert!(add(&mut cards, "a\tb", "x".into(), 0).is_err());
        assert!(add(&mut cards, "  ", "x".into(), 0).is_err());
        assert!(add(&mut cards, "a", "  ".into(), 0).is_err());
        assert!(cards.is_empty());
    }

    #[test]
    fn add_args_join_multiword_meaning() {
        let args: Vec<String> = ["haus", "a", "house"].iter().map(|s| s.to_string()).collect();
        let (w, m) = parse_add_args(&args).unwrap();
        assert_eq!((w, m.as_str()), ("haus", "a house"));
        assert!(parse_add_args(&args[..1]).is_err());
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut v: Vec<u32> = (0..50).collect();
        shuffle(&mut v, 12345);
        let mut sorted = v.clone();
        sorted.sort();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
    }
}
