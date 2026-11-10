//! A single vocabulary card and its spaced-repetition state.
//!
//! Scheduling uses a simple Leitner system: every card lives in one of
//! [`MAX_BOX`]`+1` boxes. A correct answer promotes it one box (and the gap
//! until the next review grows); a wrong answer sends it back to box 0.
//! Dates are stored as whole days since the Unix epoch, which is plenty of
//! resolution for daily review and keeps the storage format trivial.

/// Highest box index. Cards in this box are reviewed least often.
pub const MAX_BOX: u8 = 4;

/// Review interval in days for each box.
const INTERVALS: [u32; MAX_BOX as usize + 1] = [0, 1, 3, 7, 14];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub word: String,
    pub meaning: String,
    /// Leitner box, `0..=MAX_BOX`.
    pub box_idx: u8,
    /// Day (since Unix epoch) on or after which the card is due.
    pub due_day: u32,
    pub correct: u32,
    pub incorrect: u32,
}

impl Card {
    /// Create a brand-new card that is due immediately.
    pub fn new(word: impl Into<String>, meaning: impl Into<String>, today: u32) -> Self {
        Card {
            word: word.into(),
            meaning: meaning.into(),
            box_idx: 0,
            due_day: today,
            correct: 0,
            incorrect: 0,
        }
    }

    pub fn is_due(&self, today: u32) -> bool {
        self.due_day <= today
    }

    /// Record a review result and reschedule the card.
    pub fn review(&mut self, known: bool, today: u32) {
        if known {
            self.correct += 1;
            self.box_idx = (self.box_idx + 1).min(MAX_BOX);
        } else {
            self.incorrect += 1;
            self.box_idx = 0;
        }
        self.due_day = today + INTERVALS[self.box_idx as usize];
    }

    /// Fraction of reviews answered correctly, or `None` if never reviewed.
    pub fn accuracy(&self) -> Option<f64> {
        let total = self.correct + self.incorrect;
        (total > 0).then(|| self.correct as f64 / total as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_card_is_due_today() {
        let c = Card::new("haus", "house", 100);
        assert!(c.is_due(100));
        assert!(!c.is_due(99));
    }

    #[test]
    fn correct_answers_promote_and_space_out() {
        let mut c = Card::new("haus", "house", 100);
        c.review(true, 100);
        assert_eq!(c.box_idx, 1);
        assert_eq!(c.due_day, 101);
        c.review(true, 101);
        assert_eq!(c.box_idx, 2);
        assert_eq!(c.due_day, 104);
    }

    #[test]
    fn promotion_caps_at_max_box() {
        let mut c = Card::new("haus", "house", 0);
        for day in 0..20 {
            c.review(true, day);
        }
        assert_eq!(c.box_idx, MAX_BOX);
        assert_eq!(c.due_day, 19 + INTERVALS[MAX_BOX as usize]);
    }

    #[test]
    fn wrong_answer_resets_to_box_zero() {
        let mut c = Card::new("haus", "house", 0);
        c.review(true, 0);
        c.review(true, 1);
        c.review(false, 4);
        assert_eq!(c.box_idx, 0);
        assert_eq!(c.due_day, 4);
        assert_eq!((c.correct, c.incorrect), (2, 1));
    }

    #[test]
    fn accuracy_is_none_until_reviewed() {
        let mut c = Card::new("haus", "house", 0);
        assert_eq!(c.accuracy(), None);
        c.review(true, 0);
        c.review(false, 1);
        assert_eq!(c.accuracy(), Some(0.5));
    }
}
