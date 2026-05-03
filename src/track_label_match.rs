//! Word-set and fallback character overlap for matching saved track labels to `track-list` rows.
//! Prefer higher shared word counts; when word overlap is zero, rank by alphanumeric character multiset overlap.

use std::collections::HashMap;

/// Match quality: lexicographic `word` then `char` (both descending when picking a winner).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct LabelMatchScore {
    pub word_intersection: usize,
    pub char_intersection: usize,
}

pub fn seed_row_score(seed: &str, row_primary: &str, row_secondary: &str) -> LabelMatchScore {
    let a = match_score(seed, row_primary);
    let b = match_score(seed, row_secondary);
    a.max(b)
}

pub fn match_score(seed: &str, candidate: &str) -> LabelMatchScore {
    let seed_n = normalize(seed);
    let cand_n = normalize(candidate);
    let words = word_intersection_count(&seed_n, &cand_n);
    let chars = multiset_char_overlap(&seed_n, &cand_n);
    LabelMatchScore {
        word_intersection: words,
        char_intersection: chars,
    }
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

fn word_tokens(s: &str) -> HashMap<String, usize> {
    let mut m = HashMap::new();
    for w in s
        .split(|c: char| !c.is_alphanumeric())
        .map(str::trim)
        .filter(|w| !w.is_empty())
    {
        *m.entry(w.to_string()).or_insert(0) += 1;
    }
    m
}

fn word_intersection_count(a: &str, b: &str) -> usize {
    let ma = word_tokens(a);
    let mb = word_tokens(b);
    ma.iter()
        .map(|(k, va)| (*va).min(mb.get(k).copied().unwrap_or(0)))
        .sum()
}

fn multiset_char_overlap(a: &str, b: &str) -> usize {
    let ma = alphanumeric_char_counts(a);
    let mb = alphanumeric_char_counts(b);
    ma.iter()
        .map(|(k, va)| (*va).min(mb.get(k).copied().unwrap_or(0)))
        .sum()
}

fn alphanumeric_char_counts(s: &str) -> HashMap<char, usize> {
    let mut m = HashMap::new();
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_alphanumeric() {
            *m.entry(c).or_insert(0) += 1;
        }
    }
    m
}

/// Whether subtitle auto-pick should apply for this winner (avoids meaningless weak picks).
pub fn subtitle_autopick_qualifies(best: LabelMatchScore) -> bool {
    best.word_intersection > 0 || best.char_intersection >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_overlap_beats_pure_char_noise() {
        let seed = "english – commentary";
        let good = match_score(seed, "English – Commentary (AC3)");
        let worse = match_score(seed, "Français – forced");
        assert!(good.word_intersection > worse.word_intersection);
        assert!(good > worse);
    }

    #[test]
    fn zero_words_falls_back_to_chars() {
        let seed = "en";
        let toward_english = match_score(seed, "English");
        let unrelated = match_score(seed, "日本語サブ");
        assert_eq!(toward_english.word_intersection, 0);
        assert_eq!(unrelated.word_intersection, 0);
        assert!(
            toward_english.char_intersection > unrelated.char_intersection,
            "english overlaps seed `en` more than unrelated script"
        );
    }

    #[test]
    fn seed_row_secondary_picks_best_of_two_fields() {
        let seed = "eng";
        let s = seed_row_score(seed, "some title – extra", "eng");
        assert!(s.word_intersection >= 1);
    }

    #[test]
    fn subtitle_gate_rejects_coincidence() {
        assert!(!subtitle_autopick_qualifies(LabelMatchScore {
            word_intersection: 0,
            char_intersection: 1
        }));
        assert!(subtitle_autopick_qualifies(LabelMatchScore {
            word_intersection: 1,
            char_intersection: 0
        }));
        assert!(subtitle_autopick_qualifies(LabelMatchScore {
            word_intersection: 0,
            char_intersection: 2
        }));
    }
}
