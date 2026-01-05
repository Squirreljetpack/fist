#![allow(unused)]

use std::{fmt::Debug, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Deserializer};

use crate::abspath::AbsPath;

// Could we come up with some kind of heuristic to optimize how many checks are needed to break above a certain threshold?

/// Collects rules, each of which can be thought of as a `Vec<Test, Action>`.
/// Given an item, this can be used to find (the action corresponding to) the best matching rule for that item.
/// The fit of a rule to an item is computed by accumulating the score of all passing tests in the rule for the item.
///
/// # Note
/// Deserialization, the items are flipped (Action on left)
#[derive(Default, Debug, Clone)]
pub struct RuleMatcher<T, A> {
    rules: Vec<(Rule<T>, A)>,
}

/// A rule is a sequence of `(Test, Action)`'s.
/// The fit of a rule to an item is computed by accumulating the score of all passing tests in the rule for the item.
pub type Rule<T> = Vec<(Score, T)>;

#[derive(Debug, Clone)]
// u8 so that Max(u8) guarantees acceptance
pub enum Score {
    Add(u8),
    Sub(u8),
    Max(u8),
    Min(u8),
}

impl Score {
    fn modify(
        &self,
        score: u8,
    ) -> u8 {
        match *self {
            Score::Add(v) => score.saturating_add(v),
            Score::Sub(v) => score.saturating_sub(v),
            Score::Max(v) => score.max(v),
            Score::Min(v) => score.min(v),
        }
    }
}

pub trait Test<I: ?Sized> {
    /// In a run of [`RuleMatcher::get_best_match`] for an item, the context is reused across all tests.
    type Context;

    /// Test if an item passes. If so, it's score will be accumulated into the containing [`Rule`].
    fn passes(
        &self,
        item: &I,
        data: &Self::Context,
    ) -> bool;
}

impl<T, A> RuleMatcher<T, A> {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add(
        &mut self,
        id: A,
        rule: Rule<T>,
    ) {
        self.rules.push((rule, id));
    }

    #[allow(clippy::multiple_bound_locations)]
    /// Find the best matching rule for the item.
    ///
    /// # Notes
    /// - last one wins in tie
    /// - 0 score does not count
    /// - Early exit on 255
    #[cfg(not(test))]
    pub fn get_best_match<I: ?Sized>(
        &self,
        item: &I,
        context: T::Context,
    ) -> Option<&A>
    where
        T: Test<I>,
    {
        let mut best_id: Option<&A> = None;
        let mut best_score: u8 = 0;

        for (rules, id) in &self.rules {
            let mut score = 0u8;

            for r in rules {
                if r.1.passes(item, &context) {
                    score = r.0.modify(score);
                }
            }

            if score >= best_score && score > 0 {
                best_score = score;
                best_id = Some(id);

                if best_score == u8::MAX {
                    break;
                }
            }
        }

        best_id
    }

    #[allow(clippy::multiple_bound_locations)]
    #[cfg(test)]
    pub fn get_best_match<I: ?Sized>(
        &self,
        item: &I,
        context: T::Context,
    ) -> Option<&A>
    where
        T: Test<I>,
        A: std::fmt::Debug,
        I: std::fmt::Debug,
        T::Context: std::fmt::Debug,
    {
        let mut best_id: Option<&A> = None;
        let mut best_score: u8 = 0;

        for (rules, id) in &self.rules {
            let mut score = 0u8;

            for r in rules {
                if r.1.passes(item, &context) {
                    score = r.0.modify(score);
                }
            }

            eprintln!("rule id: {:?}, score: {}", id, score);

            if score >= best_score && score > 0 {
                best_score = score;
                best_id = Some(id);

                if best_score == u8::MAX {
                    break;
                }
            }
        }

        eprintln!("best match: {:?}", best_id);
        best_id
    }

    // returns (top_score, best_scores)
    fn get_best_matches_with_score<'a, I: ?Sized>(
        &'a self,
        item: &I,
        context: T::Context,
    ) -> (u8, BestMatches<'a, T, A>)
    where
        T: Test<I>,
    {
        let mut max_score = 0u8;
        let mut top_indices = Vec::new();

        for (i, (rules, _id)) in self.rules.iter().enumerate() {
            let score = rules.iter().fold(0u8, |s, r| {
                if !r.1.passes(item, &context) {
                    s
                } else {
                    r.0.modify(s)
                }
            });

            if score > max_score {
                max_score = score;
                top_indices.clear();
                top_indices.push(i);
            } else if score == max_score {
                top_indices.push(i);
            }
        }

        (
            max_score,
            BestMatches {
                matcher: self,
                indices: top_indices,
                pos: 0,
            },
        )
    }

    /// Find the best matching rules for the item. (See [`Self::get_best_match`]).
    pub fn get_best_matches<I: ?Sized>(
        &self,
        item: &I,
        context: T::Context,
    ) -> impl Iterator<Item = &A>
    where
        T: Test<I>,
    {
        let (s, m) = self.get_best_matches_with_score(item, context);
        if s > 0 {
            m
        } else {
            BestMatches {
                matcher: self,
                indices: Vec::new(),
                pos: 0,
            }
        }
    }

    // -------------------
    pub fn prepend(
        &mut self,
        other: Self,
    ) {
        let mut new_rules = other.rules;
        new_rules.append(&mut self.rules);
        self.rules = new_rules;
    }

    pub fn append(
        &mut self,
        other: Self,
    ) {
        self.rules.extend(other.rules);
    }
    pub fn len(&self) -> usize {
        self.rules.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

// ---------- Score, Test ----------------
/// When parsing a string into a (Score, Test), the default value from [`DefaultScore`] is used when the score is unspecified.
/// Scores accumulate in sequential order into the final score of a rule.
///
/// A rule is deserialized as a sequence of (Score, Test)'s.
///
/// A (Score, Test) is parsed from a string by "{score_symbol}|{test}":
/// ```rust,ignore
///   Score::Add(int) => format!("+{}|{}", v, r),
///   Score::Sub(int) => format!("-{}|{}", v, r),
///   Score::Max(int) => format!("{}|{}", v, r),
///   Score::Min(int) => format!("_{}|{}", v, r),
/// ```
///
/// When the seperator is omitted, this is the default used.
pub trait DefaultScore {
    fn default_score(&self) -> Score {
        Score::Add(1)
    }
}

#[allow(clippy::collapsible_if)]
fn parse_rule_part<T: FromStr + DefaultScore>(s: &str) -> Result<(Score, T), <T as FromStr>::Err> {
    // escaped: \... → entire string is the rule, score comes from R
    if s.starts_with('\\') {
        let r = T::from_str(s)?;
        return Ok((r.default_score(), r));
    }

    if let Some((first, rest)) = s.split_at_checked(1) {
        let score = match first {
            "+" => Some(Score::Add(1)),
            "-" => Some(Score::Sub(1)),
            "_" => Some(Score::Min(1)),
            _ => None,
        };

        if let Some(score) = score {
            if let Ok(r) = T::from_str(rest) {
                return Ok((score, r));
            }
        }
    }

    if let Some((prefix, rest)) = s.split_once('|')
        && let Ok(r) = T::from_str(rest)
    {
        let score = if let Some(stripped) = prefix.strip_prefix('_') {
            stripped.parse().map(Score::Min)
        } else if let Some(stripped) = prefix.strip_prefix('-') {
            stripped.parse().map(Score::Sub)
        } else if let Some(stripped) = prefix.strip_prefix('+') {
            stripped.parse().map(Score::Add)
        } else {
            prefix.parse().map(Score::Max)
        };

        if let Ok(score) = score {
            return Ok((score, r));
        }
    }

    // default: parse whole string as rule
    let r = T::from_str(s)?;
    Ok((r.default_score(), r))
}

fn format_rule_part<T: std::fmt::Display>(
    score: &Score,
    r: &T,
) -> String {
    match score {
        Score::Add(v) => format!("+{}|{}", v, r),
        Score::Sub(v) => format!("-{}|{}", v, r),
        Score::Max(v) => format!("{}|{}", v, r),
        Score::Min(v) => format!("_{}|{}", v, r),
    }
}

// ---------- Serde ----------------------
use serde::{Serialize, Serializer};

impl<T, A> Serialize for RuleMatcher<T, A>
where
    A: Serialize,
    T: std::fmt::Display,
{
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let seq: Vec<(Vec<String>, &A)> = self
            .rules
            .iter()
            .map(|(rule, id)| {
                let strs = rule
                    .iter()
                    .map(|(score, r)| format_rule_part(score, r))
                    .collect();
                (strs, id)
            })
            .collect();
        seq.serialize(serializer)
    }
}

impl<'de, T, A> Deserialize<'de> for RuleMatcher<T, A>
where
    A: Deserialize<'de>,
    T: FromStr + DefaultScore,
    T::Err: std::fmt::Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seq: Vec<(A, Vec<String>)> = Vec::deserialize(deserializer)?;
        let mut rules = Vec::with_capacity(seq.len());

        for (id, vec) in seq {
            let mut parsed_rule = Vec::with_capacity(vec.len());
            for s in vec {
                let part = parse_rule_part::<T>(&s).map_err(serde::de::Error::custom)?;
                parsed_rule.push(part);
            }
            rules.push((parsed_rule, id));
        }

        Ok(RuleMatcher { rules })
    }
}

// -------------- BOILERPLATE ----------------

pub struct BestMatches<'a, T, A> {
    matcher: &'a RuleMatcher<T, A>,
    indices: Vec<usize>,
    pos: usize,
}

impl<'a, R, I> Iterator for BestMatches<'a, R, I> {
    type Item = &'a I;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.indices.len() {
            None
        } else {
            let idx = self.indices[self.pos];
            self.pos += 1;
            Some(&self.matcher.rules[idx].1)
        }
    }
}

// ----------------
// fn parse_rule<R: FromStr + DefaultScore>(s: &str) -> Result<Rule<R>, R::Err> {
//     let mut parts = Vec::new();
//     let mut buf = String::new();
//     let mut escaped = false;

//     for c in s.chars() {
//         if escaped {
//             buf.push(c);
//             escaped = false;
//         } else if c == '\\' {
//             escaped = true;
//         } else if c == ',' {
//             if !buf.is_empty() {
//                 parts.push(parse_rule_part(buf.trim())?);
//                 buf.clear();
//             }
//         } else {
//             buf.push(c);
//         }
//     }

//     if !buf.is_empty() {
//         parts.push(parse_rule_part(buf.trim())?);
//     }

//     Ok(parts)
// }
