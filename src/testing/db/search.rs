use super::{exact_match_score, fuzzy_score};

#[test]
fn fuzzy_score_prefers_tighter_match() {
    let compact = fuzzy_score("alphabet", "alp").unwrap();
    let loose = fuzzy_score("a long phrase with letters", "alp").unwrap();
    assert!(compact > loose);
}

#[test]
fn exact_match_prefers_full_match_over_prefix() {
    let full = exact_match_score("actor", "actor").unwrap();
    let prefix = exact_match_score("actor_name", "actor").unwrap();
    assert!(full > prefix);
}
