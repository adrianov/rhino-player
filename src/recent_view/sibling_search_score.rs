// Neighbour-search scoring: padded character trigrams + token Jaccard (feature 33).

use std::collections::HashSet;

/// Minimum Jaccard similarity for a non-substring hit (feature 33).
/// Below classic `pg_trgm` 0.3 so one adjacent transposition on a short word still hits.
pub(super) const TRIGRAM_MIN_SCORE: f64 = 0.25;

type Trigram = (char, char, char);

/// Space-padded character trigrams (same idea as PostgreSQL `pg_trgm`).
fn char_trigrams(s: &str) -> HashSet<Trigram> {
    let mut chars = Vec::with_capacity(s.chars().count() + 4);
    chars.extend([' ', ' ']);
    chars.extend(s.chars());
    chars.extend([' ', ' ']);
    let mut set = HashSet::with_capacity(chars.len().saturating_sub(2));
    for i in 0..chars.len().saturating_sub(2) {
        set.insert((chars[i], chars[i + 1], chars[i + 2]));
    }
    set
}

fn jaccard(a: &HashSet<Trigram>, b: &HashSet<Trigram>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.iter().filter(|t| b.contains(t)).count();
    let union = a.len() + b.len() - inter;
    if union == 0 {
        0.0
    } else {
        inter as f64 / union as f64
    }
}

/// Best Jaccard of `q_tri` against the full name and each alphanumeric token.
fn best_token_jaccard(q_tri: &HashSet<Trigram>, name: &str) -> f64 {
    let mut best = jaccard(q_tri, &char_trigrams(name));
    for token in name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        let score = jaccard(q_tri, &char_trigrams(token));
        if score > best {
            best = score;
            if best >= 1.0 {
                return 1.0;
            }
        }
    }
    best
}

/// Score of a file name against query `q` (callers lowercase first). `None` = not a hit.
/// Letterlike capitals can stay uppercase after `to_lowercase()` — do not assert otherwise
/// (this runs from a GLib timeout that cannot unwind).
/// Containment uses a cheap prefix rank (no HashSet). Trigram fuzzy needs 3+ characters.
pub(super) fn name_match_score(name_lower: &str, q: &str, q_tri: &HashSet<Trigram>) -> Option<f64> {
    if name_lower.contains(q) {
        return Some(substring_score(name_lower, q));
    }
    if q.chars().count() < 3 {
        return None;
    }
    let score = best_token_jaccard(q_tri, name_lower);
    (score >= TRIGRAM_MIN_SCORE).then_some(score)
}

/// Exact token / name prefix outranks a later substring (one-letter queries stay cheap).
fn substring_score(name: &str, q: &str) -> f64 {
    if name.starts_with(q) {
        return 1.0;
    }
    let mut best = 0.5_f64;
    for token in name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        if token == q {
            return 1.0;
        }
        if token.starts_with(q) {
            best = best.max(0.9);
        }
    }
    best
}

/// Trigram set for a settled query string (call once per filter).
pub(super) fn query_trigrams(q: &str) -> HashSet<Trigram> {
    char_trigrams(q)
}

#[cfg(test)]
mod score_tests {
    use super::*;

    #[test]
    fn substring_is_kept_even_when_jaccard_is_low() {
        let q = "s01e04";
        let q_tri = query_trigrams(q);
        let long = "some.long.show.name.with.s01e04.and.many.extra.tokens.mkv";
        let score = name_match_score(long, q, &q_tri).expect("substring hit");
        assert!(score < TRIGRAM_MIN_SCORE || long.contains(q));
        assert!(long.contains(q));
    }

    #[test]
    fn closer_name_scores_higher() {
        let q = "somm";
        let q_tri = query_trigrams(q);
        let close = name_match_score("somm.2012.mkv", q, &q_tri).unwrap();
        let weak = name_match_score("summer.vacation.mkv", q, &q_tri);
        assert!(close > weak.unwrap_or(0.0));
    }

    #[test]
    fn unrelated_name_is_dropped() {
        let q = "sideways";
        let q_tri = query_trigrams(q);
        assert!(name_match_score("totally.different.avi", q, &q_tri).is_none());
    }

    #[test]
    fn case_folded_query_matches() {
        let q = "episode";
        let q_tri = query_trigrams(q);
        assert!(name_match_score("episode 7.mp4", q, &q_tri).is_some());
    }

    #[test]
    fn misspellings_match_tokens_in_long_names() {
        let q_tri = query_trigrams("epsiode");
        assert!(name_match_score("episode 7.mp4", "epsiode", &q_tri).is_some());
        let q_tri = query_trigrams("matirx");
        assert!(name_match_score("the.matrix.reloaded.2003.mkv", "matirx", &q_tri).is_some());
        let q_tri = query_trigrams("mvoie");
        assert!(name_match_score("movie.mkv", "mvoie", &q_tri).is_some());
        let q_tri = query_trigrams("inceotion");
        assert!(name_match_score("inception.2010.mkv", "inceotion", &q_tri).is_some());
    }

    #[test]
    fn short_noise_does_not_fuzzy_match() {
        let q = "cats";
        let q_tri = query_trigrams(q);
        assert!(name_match_score("summer.vacation.mkv", q, &q_tri).is_none());
    }

    #[test]
    fn one_letter_matches_containment_only() {
        let q = "z";
        let q_tri = query_trigrams(q);
        assert!(name_match_score("lazy.dog.mkv", q, &q_tri).is_some());
        assert!(name_match_score("apple.mkv", q, &q_tri).is_none());
    }

    #[test]
    fn prefix_outranks_later_substring() {
        let q = "s";
        let q_tri = query_trigrams(q);
        let prefix = name_match_score("show.mkv", q, &q_tri).unwrap();
        let later = name_match_score("episode.mkv", q, &q_tri).unwrap();
        assert!(prefix > later);
    }

    #[test]
    fn leftover_uppercase_after_fold_does_not_panic() {
        let name = "show.ℝ.mkv".to_lowercase();
        let q = "show";
        let q_tri = query_trigrams(q);
        assert!(name.chars().any(|c| c.is_uppercase()));
        assert!(name_match_score(&name, q, &q_tri).is_some());
    }
}
