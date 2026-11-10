# vocab

A small command-line vocabulary trainer written in Rust, with Leitner-style
spaced repetition. No dependencies — it builds with nothing but `cargo`.

```
$ vocab add haus house
added haus
$ vocab add katze the cat
added katze
$ vocab quiz
[1/2] katze
  (press Enter to reveal)
  → the cat
  did you know it? [y/n] y
...
done: 2/2 known
$ vocab stats
cards: 2   due today: 0
  box 0: 0
  box 1: 2
  ...
```

## Install

```
cargo install --path .
```

or just `cargo run -- <command>` while developing.

## Commands

| Command                        | What it does                              |
| ------------------------------ | ----------------------------------------- |
| `vocab add <word> <meaning...>` | Add a card (meaning may be several words) |
| `vocab remove <word>`          | Delete a card                             |
| `vocab list`                   | Show all cards with box and accuracy      |
| `vocab quiz [N]`               | Review up to N due cards (default 10)     |
| `vocab stats`                  | Deck size, cards per box, overall accuracy|

The deck is stored in `$VOCAB_FILE` if set, otherwise `~/.vocab.tsv`.

## How scheduling works

Each card sits in one of five boxes. Answering correctly moves it up one
box; answering wrongly drops it back to box 0. The box decides when the
card is next due:

| Box | Next review |
| --- | ----------- |
| 0   | today       |
| 1   | 1 day       |
| 2   | 3 days      |
| 3   | 7 days      |
| 4   | 14 days     |

## Design notes

- **Storage is plain TSV**, one card per line. It is readable, greppable,
  and diffs cleanly in git. Saving writes a temp file and renames it, so a
  crash can't leave a half-written deck. Tabs/newlines are rejected at
  `add` time rather than escaped, keeping the parser trivial.
- **Dates are whole days** since the Unix epoch. Daily review doesn't need
  finer resolution, and it avoids any date/time dependency.
- **Self-graded quiz.** The app reveals the meaning and asks whether you
  knew it, instead of string-matching a typed answer. Typo handling and
  synonym matching are hard to get right and easy to get annoying.
- **Modules:** `card.rs` (scheduling logic, pure and fully unit-tested),
  `store.rs` (file I/O and parsing), `main.rs` (CLI and interaction).

## Development

```
cargo test
cargo clippy
```

## Ideas for later

- Reverse quizzes (meaning → word)
- Import/export from CSV or Anki
- Tags / multiple decks
- Switch to `clap` once the CLI grows beyond a handful of commands
