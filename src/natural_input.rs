// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::{HashMap, HashSet};

use lazy_regex::regex;
use thiserror::Error;

#[derive(PartialEq, Eq, Debug)]
pub struct AmbiguousCandidate<'a, T> {
    pub candidate: &'a T,
    pub matched_label: &'a str,
}

#[derive(PartialEq, Debug)]
pub enum PlanResult<'a, T> {
    /// The message did not parse as `<name> <quantity>[unit]` (ordinary chatter).
    NotADose,
    /// The message parsed as a dose attempt, but no candidate matched.
    NoMatch,
    /// Exactly one candidate matched.
    Match {
        candidate: &'a T,
        matched_label: &'a str,
        quantity: f64,
    },
    /// Multiple candidates tied for the best match.
    Ambiguous {
        candidates: Vec<AmbiguousCandidate<'a, T>>,
        quantity: f64,
    },
}

/// A single collision between an `added` label and an `existing` candidate's label.
#[derive(PartialEq, Eq, Debug)]
pub struct Collision<'a, T> {
    pub added_label: &'a str,
    pub existing: &'a T,
    pub existing_label: &'a str,
}

pub trait Matchable: std::fmt::Debug {
    fn labels(&self) -> impl Iterator<Item = &str>;
}

fn collision_key(label: &str) -> String {
    let label = normalize(label);
    let mut normalized_tokens = label.split_whitespace().collect::<Vec<_>>();
    normalized_tokens.sort();
    normalized_tokens.join(" ")
}

/// Find labels of `added` that collide with labels of the `existing` candidates.
///
/// Two labels collide when their [`collision_key`]s match — i.e. they normalize
/// (lowercase, punctuation → spaces) to the same sorted multi-set of tokens.
/// This catches the same medication under different word order or punctuation,
/// e.g. `"Nurofen Kids"` and `"Kids Nurofen"`.
///
/// Returns one entry per colliding label of `added`, in `added.labels()` order:
/// a [`Collision`] carrying the added label, one of the colliding existing
/// candidates, and its colliding label.
///
/// - When several existing candidates/labels share a collision key, *any* one
///   of them may be returned
/// - Labels of `added` are not deduplicated, so a repeated label yields
///   repeated entries.
#[must_use]
pub fn find_collisions<'a, T: Matchable>(added: &'a T, existing: &'a [T]) -> Vec<Collision<'a, T>> {
    let mut existing_labels = HashMap::new();
    for ex in existing {
        for lbl in ex.labels() {
            // Overwriting the colliding example is OK, we only need one example of why it's a
            // collision
            existing_labels.insert(collision_key(lbl), (lbl, ex));
        }
    }

    added
        .labels()
        .filter_map(|lbl| {
            let key = collision_key(lbl);
            existing_labels
                .get(&key)
                .map(|&(colliding_label, colliding_ex)| Collision {
                    added_label: lbl,
                    existing: colliding_ex,
                    existing_label: colliding_label,
                })
        })
        .collect()
}

pub fn plan<'a, T: Matchable>(query: &str, candidates: &'a [T]) -> PlanResult<'a, T> {
    let Some((name_query, quantity)) = parse(query) else {
        return PlanResult::NotADose;
    };
    let name_query = normalize(&name_query);

    let matches = find_best_match(&name_query, candidates);
    if let [(candidate, matched_label, ..)] = matches.as_slice() {
        PlanResult::Match {
            candidate,
            matched_label,
            quantity,
        }
    } else if matches.is_empty() {
        PlanResult::NoMatch
    } else {
        PlanResult::Ambiguous {
            candidates: matches
                .into_iter()
                .map(|(candidate, matched_label, _)| AmbiguousCandidate {
                    candidate,
                    matched_label,
                })
                .collect(),
            quantity,
        }
    }
}

/// Returns the best-matching candidates, a label that matched, and their score.
///
/// - Empty: no candidate matched the query.
/// - Single element: an unambiguous winner.
/// - Multiple elements: a tie (all share the same best score).
fn find_best_match<'a, T: Matchable>(
    query: &str,
    candidates: &'a [T],
) -> Vec<(&'a T, &'a str, Score)> {
    let scored: Vec<(&T, &str, Score)> = candidates
        .iter()
        .filter_map(|candidate| {
            let min_match = candidate
                .labels()
                .filter_map(|label| score(query, label).map(|s| (label, s)))
                .min_by_key(|(_, s)| *s);
            min_match.map(|(label, min_score)| (candidate, label, min_score))
        })
        .collect();

    log::debug!("find_best_match({query:?}, {candidates:?}) -> {scored:?}");

    let Some(best_score) = scored.iter().map(|(_, _, s)| s).min().copied() else {
        return vec![];
    };

    scored
        .into_iter()
        .filter(|(_, _, s)| *s == best_score)
        .collect()
}

// cSpell: word bidi NFKC
fn strip_bidi(s: &str) -> String {
    // Strip bidirectional control characters. Hebrew text, especially when pasted or mixed
    // with digits and Latin, often carries invisible direction marks. They aren't whitespace,
    // so they would otherwise glue themselves onto a word or onto the quantity.
    s.chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200E}' | '\u{200F}'   // LRM/RLM
                | '\u{202A}'..='\u{202E}' // LRE/RLE/PDF/LRO/RLO
                | '\u{2066}'..='\u{2069}' // LRI/RLI/FSI/PDI
            )
        })
        .collect()
}

fn normalize(s: &str) -> String {
    let base = pre_normalize(s);
    regex!(r"\p{P}").replace_all(&base, " ").into_owned()
}

const MAX_NAME_CHARS: usize = 60;
const MAX_NAME_TOKENS: usize = 6;
const MAX_ALIASES: usize = 20;

#[derive(Debug, PartialEq, Eq, Error)]
pub enum AliasError {
    #[error("Alias {0:?} must not contain commas or newlines")]
    ForbiddenSeparator(String),
    #[error(
        "Invalid alias {0:?}: must contain a letter and be at most {MAX_NAME_CHARS} characters and {MAX_NAME_TOKENS} words"
    )]
    InvalidLabel(String),
    #[error("Too many aliases (maximum {MAX_ALIASES})")]
    TooMany,
}

/// Validates, trims, and deduplicates a medication's alias list for saving.
///
/// Rejects separator characters (`\n`, `\r`, `,`) before trimming so trailing
/// newlines cannot slip through, drops empty entries and aliases colliding with
/// `medication_name`, deduplicates by [`collision_key`], and enforces
/// [`MAX_ALIASES`].
pub fn clean_aliases(medication_name: &str, aliases: &[String]) -> Result<Vec<String>, AliasError> {
    let mut seen_keys = HashSet::new();
    let own_key = collision_key(medication_name);
    if !own_key.is_empty() {
        seen_keys.insert(own_key);
    }

    let mut cleaned = Vec::new();
    for raw in aliases {
        if raw.contains(['\n', '\r', ',']) {
            return Err(AliasError::ForbiddenSeparator(raw.clone()));
        }

        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !is_valid_label(trimmed) {
            return Err(AliasError::InvalidLabel(trimmed.to_string()));
        }

        if seen_keys.insert(collision_key(trimmed)) {
            cleaned.push(trimmed.to_string());
        }
    }

    if cleaned.len() > MAX_ALIASES {
        return Err(AliasError::TooMany);
    }

    Ok(cleaned)
}

fn is_valid_label(name: &str) -> bool {
    if !name.chars().any(char::is_alphabetic) || name.chars().count() > MAX_NAME_CHARS {
        log::debug!("Rejecting {name:?}: no letter or exceeds {MAX_NAME_CHARS} chars");
        return false;
    }

    if normalize(name).split_whitespace().count() > MAX_NAME_TOKENS {
        log::debug!("Rejecting {name:?} as too many tokens");
        return false;
    }

    true
}

fn parse(s: &str) -> Option<(String, f64)> {
    // Perform bidi strip and NFKC before parsing, so full-width digits and stray direction marks
    // don't break the regex. Lowercase and punctuation replacement are left to `normalize`, so the
    // name keeps its case here and decimal periods survive.
    let prepared = prepare_for_parse(s);
    let trimmed = prepared.trim();

    let r = regex!(
        r"^(?P<name>.+?)\s*(?P<qty>-?(?:[0-9]+(?:[.,][0-9]+)?|[.,][0-9]+))\s*(?P<unit>[^\s0-9]{0,8})$"
    );

    let caps = r.captures(trimmed)?;
    log::debug!("Parsed {s:?} as {:?}", caps);

    let name = &caps["name"];
    let unit = &caps["unit"];

    if !is_valid_label(name) {
        return None;
    }

    // If a unit is present, it must contain at least one letter. This needs no allowlist:
    // `ml`, `mL`, `מ"ל` pass, while a trailing `?` (as in "fever 39?") fails, so questions
    // that happen to end in a number are treated as chatter.
    if !unit.is_empty() && !unit.chars().any(char::is_alphabetic) {
        log::debug!("Rejecting {s:?} as the unit is {unit:?}");
        return None;
    }

    // f64::parse() only allows `.` as the decimal separator, but we insist on supporting commas as
    // decimal separators for i18n purposes.
    let qty: f64 = caps["qty"].replace(',', ".").parse().ok()?;
    // Reject non-finite, zero and negative quantities (0 is "skip", which has its own button).
    if !qty.is_finite() || qty <= 0.0 {
        log::debug!("Rejecting {s:?} because the quantity is {qty:?}");
        return None;
    }

    Some((name.to_owned(), qty))
}

fn prepare_for_parse(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    // Bidi strip + NFKC only (no lowercasing, no punctuation replacement).
    strip_bidi(s).nfkc().collect()
}

fn pre_normalize(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    strip_bidi(s).nfkc().collect::<String>().to_lowercase()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Score {
    total_edits: usize,
    unmatched_label_tokens: usize,
}

fn score(query: &str, label: &str) -> Option<Score> {
    // cSpell: word damerau
    use strsim::damerau_levenshtein;

    let query = normalize(query);
    let label = normalize(label);

    // Greedy token matching: for each query token, pick the closest remaining label token. This is
    // NOT guaranteed to be optimal (a tie-break can consume a label token that a later query token
    // needed) nor fully reproducible across runs (HashSet iteration order is randomized). We accept
    // that because it's very rare for two tokens within a multi-token medication name to be similar
    // enough to collide, and the practical cost of a mismatched tie is a single missed dose record.
    let mut label_tokens: HashSet<String> = label.split_whitespace().map(String::from).collect();

    let mut total_edits = 0;

    for query_token in query.split_whitespace() {
        if label_tokens.is_empty() {
            return None;
        }

        let budget = match query_token.chars().count() {
            1..=3 => 0,
            4..=6 => 1,
            _ => 2,
        };

        let (best_match, distance) = label_tokens
            .iter()
            .map(|t| (t.clone(), damerau_levenshtein(query_token, t)))
            .min_by_key(|(_, d)| *d)?;

        if distance > budget {
            return None;
        }

        label_tokens.remove(&best_match);
        total_edits += distance;
    }

    Some(Score {
        unmatched_label_tokens: label_tokens.len(),
        total_edits,
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    #[test]
    fn test_score_ord() {
        assert!(
            Score {
                total_edits: 0,
                unmatched_label_tokens: 0,
            } < Score {
                total_edits: 1,
                unmatched_label_tokens: 0,
            }
        );

        assert!(
            Score {
                total_edits: 1,
                unmatched_label_tokens: 2,
            } < Score {
                total_edits: 2,
                unmatched_label_tokens: 1,
            }
        );
    }

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("hello,world"), "hello world");
    }

    // cSpell: word נורופן Nurofen

    #[rstest]
    #[case("Nurofen2", Some(("Nurofen", 2.0)))]
    #[case("Nurofen kids 7.5", Some(("Nurofen kids", 7.5)))]
    #[case("Nurofen kids 7.5ml", Some(("Nurofen kids", 7.5)))]
    #[case("Nurofen kids 7.5 ml", Some(("Nurofen kids", 7.5)))]
    #[case("Nurfen kidss 7.5ml", Some(("Nurfen kidss", 7.5)))] // cSpell: disable-line
    #[case("Nurofen 7.5ml", Some(("Nurofen", 7.5)))]
    #[case("kids nurofen 7.5", Some(("kids nurofen", 7.5)))]
    #[case("נורופן 7.5", Some(("נורופן", 7.5)))]
    #[case("נורופן7.5", Some(("נורופן", 7.5)))]
    #[case("נורופן 7.5מל", Some(("נורופן", 7.5)))]
    #[case("נורופן 7.5 מ\"ל", Some(("נורופן", 7.5)))]
    #[case("\u{200f}נורופן 7.5\u{200f}", Some(("נורופן", 7.5)))]
    #[case("Nurofen kids ７.５", Some(("Nurofen kids", 7.5)))]
    #[case("Nurofen kids 7,5", Some(("Nurofen kids", 7.5)))]
    #[case("Nurofen .5", Some(("Nurofen", 0.5)))]
    #[case("Nurofen ,5", Some(("Nurofen", 0.5)))]
    #[case("Nurofen .5ml", Some(("Nurofen", 0.5)))]
    #[case("Nurofen .0", None)]
    #[case("Nurofen -.5", None)]
    #[case("Unrelated message", None)]
    #[case("nurofen", None)]
    #[case("7.5", None)]
    #[case("123 456", None)]
    #[case("Nurofen 0", None)]
    #[case("Nurofen 0.0", None)]
    #[case("Nurofen -1", None)]
    #[case("Is her fever 39?", None)]
    #[case("one two three four five six seven 5", None)]
    #[case(
        "a_very_long_medication_name_that_exceeds_sixty_characters_in_total 5",
        None
    )]
    fn test_parse(#[case] input: &str, #[case] want: Option<(&str, f64)>) {
        assert_eq!(
            parse(input)
                .as_ref()
                .map(|(name, quantity)| (name.as_str(), *quantity)),
            want
        )
    }

    // cSpell: disable
    #[rstest]
    #[case("Nurofen", "Nurofen", Some((0, 0)))]
    #[case("Nurofn", "Nurofen", Some((1, 0)))]
    #[case("Nurofn", "Nurofen Kids", Some((1, 1)))]
    #[case("Kids Nurofn", "Nurofen Kids", Some((1, 0)))]
    #[case("Nurfen kidss", "Nurofen Kids", Some((2, 0)))]
    #[case("Kids", "Nurofen Kids", Some((0, 1)))]
    #[case("Nurofen", "Nurofen kids", Some((0, 1)))]
    #[case("Nurofen kids", "Nurofen", None)]
    #[case("abc", "abc", Some((0, 0)))]
    #[case("abd", "abc", None)]
    #[case("acb", "abc", None)]
    #[case("kidz", "kids", Some((1, 0)))]
    #[case("kdis", "kids", Some((1, 0)))]
    #[case("kizz", "kids", None)]
    #[case("nxroffn", "nurofen", Some((2, 0)))]
    #[case("nxroffx", "nurofen", None)]
    // cSpell: enable
    fn test_score(#[case] query: &str, #[case] label: &str, #[case] want: Option<(usize, usize)>) {
        let want = want.map(|(total_edits, unmatched_label_tokens)| Score {
            total_edits,
            unmatched_label_tokens,
        });

        assert_eq!(
            score(query, label),
            want,
            "score({query:?}, {label:?}) != {want:?}"
        );
    }

    #[fixture]
    fn candidates() -> Vec<FakeMed> {
        vec![
            FakeMed::new(&["Nurofen Kids", "נורופן"]),
            FakeMed::new(&["Paracetamol"]),
            // "Zed A" and "Zed B" are designed to tie each other (same score for any "Zed"-like query)
            FakeMed::new(&["Zed A"]),
            FakeMed::new(&["Zed B"]),
            // "Drug X" and "Drug Y" share alias "SharedNick" to test ambiguous alias matches
            FakeMed::new(&["Drug X", "SharedNick"]),
            FakeMed::new(&["Drug Y", "SharedNick"]),
        ]
    }

    #[derive(Debug, PartialEq)]
    struct FakeMed {
        name: String,
        nicknames: Vec<String>,
    }

    impl FakeMed {
        fn new(nicknames: &[&str]) -> Self {
            Self {
                name: nicknames.first().unwrap().to_string(),
                nicknames: nicknames.iter().map(|&s| s.to_string()).collect(),
            }
        }
    }

    impl Matchable for FakeMed {
        fn labels(&self) -> impl Iterator<Item = &str> {
            self.nicknames.iter().map(|s| s.as_str())
        }
    }

    fn get_candidate_by_name<'a>(candidates: &'a [FakeMed], name: &str) -> &'a FakeMed {
        candidates
            .iter()
            .find(|med| med.name == name)
            .unwrap_or_else(|| {
                let available: Vec<_> = candidates
                    .iter()
                    .map(|l| l.nicknames.first().unwrap())
                    .collect();
                panic!("No candidate called {name:?}; candidates are {available:?}")
            })
    }

    // cSpell: disable
    #[rstest]
    #[case("Unobtainium", &[])]
    #[case("Nurofen kids", &[("Nurofen Kids", 0, 0)])]
    #[case("Nurofe kids", &[("Nurofen Kids", 1, 0)])]
    #[case("Nurofen", &[("Nurofen Kids", 0, 1)])]
    #[case("Nurofe", &[("Nurofen Kids", 1, 1)])]
    #[case("נורופ", &[("Nurofen Kids", 1, 0)])]
    #[case("Paracetam", &[("Paracetamol", 2, 0)])]
    #[case("Zed", &[("Zed A", 0, 1), ("Zed B", 0, 1)])]
    #[case("SharedNick", &[("Drug X", 0, 0), ("Drug Y", 0, 0)])]
    // cSpell: enable
    fn test_find_best_match(
        #[case] query: &str,
        #[case] want: &[(&str, usize, usize)],
        candidates: Vec<FakeMed>,
    ) {
        let got: Vec<(&str, usize, usize)> = find_best_match(query, &candidates)
            .into_iter()
            .map(|(c, _label, s)| (c.name.as_str(), s.total_edits, s.unmatched_label_tokens))
            .collect();
        assert_eq!(want, got.as_slice(), "{query:?} didn't resolve as expected");
    }

    #[rstest]
    // cSpell: disable
    #[case("Nurofen turbo 1", None)]
    #[case("Unobtainium 2", None)]
    #[case("ok see you at 5", None)]
    #[case("Nurofen at 5", None)]
    #[case("Nurofen kids 2.5", Some(("Nurofen Kids", 2.5)))]
    #[case("Nurofe kids 2.5", Some(("Nurofen Kids", 2.5)))]
    #[case("Nurfen kidss 7.5ml", Some(("Nurofen Kids", 7.5)))]
    #[case("kids nurofen 7.5", Some(("Nurofen Kids", 7.5)))]
    #[case("Nurofen kids 2,5", Some(("Nurofen Kids", 2.5)))] // comma as decimal separator
    #[case("Nurofen kids 2.5ml", Some(("Nurofen Kids", 2.5)))] // unit with no space
    #[case("Nurofen kids 2.5 ml", Some(("Nurofen Kids", 2.5)))] // unit with space
    #[case("Nurofen 7.5ml", Some(("Nurofen Kids", 7.5)))]
    #[case("Nurofen .5", Some(("Nurofen Kids", 0.5)))]
    #[case("נורופן 7.5", Some(("Nurofen Kids", 7.5)))]
    #[case("נורופן7.5", Some(("Nurofen Kids", 7.5)))]
    #[case("\u{200f}נורופן 7.5\u{200f}", Some(("Nurofen Kids", 7.5)))]
    #[case("נורופן 2.5מל", Some(("Nurofen Kids", 2.5)))] // Hebrew unit, no space
    #[case("נורופן 2.5 מ\"ל", Some(("Nurofen Kids", 2.5)))] // Hebrew unit with punctuation
    #[case("Nurofen 3", Some(("Nurofen Kids", 3.0)))]
    #[case("Nurofe 3", Some(("Nurofen Kids", 3.0)))]
    #[case("נורופ 4", Some(("Nurofen Kids", 4.0)))]
    #[case("Paracetam 2", Some(("Paracetamol", 2.0)))]
    // cSpell: enable
    // See also test_plan_ambiguous below.
    fn test_plan_unambiguous(
        #[case] query: &str,
        #[case] want: Option<(&str, f64)>,
        candidates: Vec<FakeMed>,
    ) {
        let got = plan(query, &candidates);
        match (want, got) {
            (None, PlanResult::NoMatch) => {}
            (
                Some((name, quantity)),
                PlanResult::Match {
                    candidate,
                    quantity: got_qty,
                    ..
                },
            ) => {
                assert_eq!(
                    get_candidate_by_name(&candidates, name),
                    candidate,
                    "wrong candidate for {query:?}"
                );
                assert_eq!(quantity, got_qty, "wrong quantity for {query:?}");
            }
            (None, other) => {
                panic!("{query:?}: expected NoMatch, got {other:?}")
            }
            (Some((name, _)), other) => {
                panic!("{query:?}: expected Match({name:?}, ...), got {other:?}")
            }
        }
    }

    #[test]
    fn test_plan_both_nurofen_and_nurofen_kids_enabled() {
        let both = vec![
            FakeMed::new(&["Nurofen"]),
            FakeMed::new(&["Nurofen Kids", "נורופן"]),
        ];

        assert_eq!(
            plan("Nurofen kids 7.5", &both),
            PlanResult::Match {
                candidate: &both[1],
                matched_label: "Nurofen Kids",
                quantity: 7.5,
            }
        );

        assert_eq!(
            plan("Nurofen 7.5", &both),
            PlanResult::Match {
                candidate: &both[0],
                matched_label: "Nurofen",
                quantity: 7.5,
            }
        );
    }

    #[rstest]
    #[case("Unrelated message")]
    #[case("Nurofen")]
    #[case("7.5")]
    #[case("Nurofen 0")]
    #[case("Nurofen 0.0")]
    #[case("Nurofen -1")]
    #[case("Is her fever 39?")]
    #[case("one two three four five six seven 5")]
    fn test_plan_not_a_dose(#[case] query: &str, candidates: Vec<FakeMed>) {
        assert_eq!(plan(query, &candidates), PlanResult::NotADose);
    }

    #[rstest]
    #[case("Zed 1", 1.0, &[("Zed A", "Zed A"), ("Zed B", "Zed B")])]
    #[case("Zed 1ml", 1.0, &[("Zed A", "Zed A"), ("Zed B", "Zed B")])]
    #[case("SharedNick 2", 2.0, &[("Drug X", "SharedNick"), ("Drug Y", "SharedNick")])]
    fn test_plan_ambiguous(
        #[case] query: &str,
        #[case] want_qty: f64,
        #[case] want_names: &[(&str, &str)],
        candidates: Vec<FakeMed>,
    ) {
        let PlanResult::Ambiguous {
            candidates: tied,
            quantity,
        } = plan(query, &candidates)
        else {
            panic!("{query:?}: expected Ambiguous");
        };
        assert_eq!(want_qty, quantity, "{query:?}: wrong quantity");
        let mut got: Vec<(&str, &str)> = tied
            .iter()
            .map(|m| (m.candidate.name.as_str(), m.matched_label))
            .collect();
        got.sort_unstable();
        let mut want = want_names.to_vec();
        want.sort_unstable();
        assert_eq!(want, got, "{query:?}: wrong tied candidates");
    }

    #[test]
    fn test_find_collisions_empty_existing() {
        let existing: Vec<FakeMed> = vec![];
        let added = FakeMed::new(&["Nurofen Kids"]);
        assert!(find_collisions(&added, &existing).is_empty());
    }

    #[test]
    fn test_find_collisions_no_match() {
        let existing = vec![FakeMed::new(&["Paracetamol"])];
        let added = FakeMed::new(&["Nurofen Kids"]);
        assert!(find_collisions(&added, &existing).is_empty());
    }

    #[test]
    fn test_find_collisions_exact_match() {
        let existing = vec![FakeMed::new(&["Nurofen Kids"])];
        let added = FakeMed::new(&["Nurofen Kids"]);
        let got = find_collisions(&added, &existing);
        assert_eq!(
            got,
            vec![Collision {
                added_label: "Nurofen Kids",
                existing: &existing[0],
                existing_label: "Nurofen Kids",
            }]
        );
    }

    #[test]
    fn test_find_collisions_token_reorder() {
        let existing = vec![FakeMed::new(&["Nurofen Kids"])];
        let added = FakeMed::new(&["Kids Nurofen"]);
        let got = find_collisions(&added, &existing);
        assert_eq!(
            got,
            vec![Collision {
                added_label: "Kids Nurofen",
                existing: &existing[0],
                existing_label: "Nurofen Kids",
            }]
        );
    }

    #[test]
    fn test_find_collisions_multiple_labels() {
        let existing = vec![
            FakeMed::new(&["Nurofen Kids"]),
            FakeMed::new(&["Paracetamol"]),
        ];
        let added = FakeMed::new(&["Nurofen Kids", "Paracetamol"]);
        let got = find_collisions(&added, &existing);
        assert_eq!(
            got,
            vec![
                Collision {
                    added_label: "Nurofen Kids",
                    existing: &existing[0],
                    existing_label: "Nurofen Kids",
                },
                Collision {
                    added_label: "Paracetamol",
                    existing: &existing[1],
                    existing_label: "Paracetamol",
                },
            ]
        );
    }

    #[test]
    fn test_find_collisions_normalization() {
        let existing = vec![FakeMed::new(&["Nurofen-Kids"])];
        let added = FakeMed::new(&["NUROFEN KIDS", "\u{200f}kids nurofen\u{200e}"]);
        let got = find_collisions(&added, &existing);
        assert_eq!(
            got,
            vec![
                Collision {
                    added_label: "NUROFEN KIDS",
                    existing: &existing[0],
                    existing_label: "Nurofen-Kids",
                },
                Collision {
                    added_label: "\u{200f}kids nurofen\u{200e}",
                    existing: &existing[0],
                    existing_label: "Nurofen-Kids",
                },
            ]
        );
    }

    #[test]
    fn test_find_collisions_added_no_labels() {
        let existing = vec![FakeMed::new(&["Nurofen Kids"])];
        let added = FakeMed {
            name: "x".into(),
            nicknames: vec![],
        };
        assert!(find_collisions(&added, &existing).is_empty());
    }

    fn to_strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[rstest]
    #[case("foo\nbar")]
    #[case("foo\n")]
    #[case("\n")]
    #[case("foo\rbar")]
    #[case("\r")]
    #[case("foo, bar")]
    #[case(",")]
    fn test_clean_aliases_rejects_separators(#[case] bad: &str) {
        let got = clean_aliases("Nurofen", &to_strings(&["valid", bad]));
        assert_eq!(got, Err(AliasError::ForbiddenSeparator(bad.to_string())));
    }

    #[test]
    fn test_clean_aliases_trims_and_drops_empty() {
        let got = clean_aliases("Nurofen", &to_strings(&["", "   ", "\t", "  נורופן  "]));
        assert_eq!(got, Ok(to_strings(&["נורופן"])));
    }

    // cSpell: word Advil
    #[test]
    fn test_clean_aliases_deduplicates_by_collision_key() {
        let got = clean_aliases(
            "Nurofen",
            &to_strings(&[
                "Kids Advil",
                "advil-kids",
                "ADVIL KIDS",
                "advil kids",
                "\u{200f}Kids Advil",
            ]),
        );
        assert_eq!(got, Ok(to_strings(&["Kids Advil"])));
    }

    #[test]
    fn test_clean_aliases_drops_own_name_variants() {
        let got = clean_aliases(
            "Nurofen Kids",
            &to_strings(&[
                "Nurofen Kids",
                "nurofen kids",
                "Kids Nurofen",
                "Nurofen-Kids",
            ]),
        );
        assert_eq!(got, Ok(Vec::<String>::new()));
    }

    #[rstest]
    #[case("123")]
    #[case("---")]
    #[case("\u{200f}")]
    #[case("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")] // 61 chars
    #[case("one two three four five six seven")] // 7 words
    fn test_clean_aliases_rejects_invalid_labels(#[case] bad: &str) {
        let got = clean_aliases("Nurofen", &to_strings(&[bad]));
        assert_eq!(got, Err(AliasError::InvalidLabel(bad.to_string())));
    }

    #[rstest]
    #[case("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")] // 60 chars
    #[case("one two three four five six")] // 6 words
    fn test_clean_aliases_accepts_boundary_labels(#[case] ok_label: &str) {
        let got = clean_aliases("Nurofen", &to_strings(&[ok_label]));
        assert_eq!(got, Ok(to_strings(&[ok_label])));
    }

    #[test]
    fn test_clean_aliases_enforces_max_cap() {
        let twenty: Vec<String> = (0..MAX_ALIASES)
            .map(|i| format!("alias-{}", (b'a' + (i as u8)) as char))
            .collect();
        assert_eq!(clean_aliases("Nurofen", &twenty), Ok(twenty.clone()));

        let mut twenty_with_noise = vec![
            String::new(),
            "   ".to_string(),
            "Nurofen".to_string(),
            "alias-a".to_string(),
        ];
        twenty_with_noise.extend(twenty.clone());
        assert_eq!(clean_aliases("Nurofen", &twenty_with_noise), Ok(twenty));

        let twenty_one: Vec<String> = (0..=MAX_ALIASES)
            .map(|i| format!("alias-{}", (b'a' + (i as u8)) as char))
            .collect();
        assert_eq!(
            clean_aliases("Nurofen", &twenty_one),
            Err(AliasError::TooMany)
        );
    }
}
