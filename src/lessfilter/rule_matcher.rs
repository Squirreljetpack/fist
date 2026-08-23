use std::{fmt::Debug, str::FromStr};

use serde::{Deserialize, Deserializer};

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

/// The best matching rule together with its score and the `(score, test)`
/// parts of the rule — as returned by
/// [`RuleMatcher::get_best_match_with_score`].
pub type BestMatchWithScore<'a, A, T> = (&'a A, u8, &'a [(Score, T)]);

#[derive(Debug, Clone)]
// u8 so that Max(u8) guarantees acceptance
pub enum Score {
    Add(u8),
    Sub(u8),
    Max(u8),
    Min(u8),
    Req,
}

impl Score {
    /// Format this score modifier together with the rule part it applies to,
    /// e.g. `>40|mime:image/*`.
    pub fn format<T: std::fmt::Display>(
        &self,
        r: &T,
    ) -> String {
        format_rule_part(self, r)
    }

    // todo: lowpri: should we have invert field on score or filerule
    fn modify(
        &self,
        score: u8,
        success: bool,
    ) -> u8 {
        if success {
            match *self {
                Score::Add(v) => score.saturating_add(v),
                Score::Sub(v) => score.saturating_sub(v),
                Score::Max(v) => score.max(v),
                Score::Min(v) => score.min(v),
                _ => score,
            }
        } else {
            match *self {
                Score::Req => 0,
                _ => score,
            }
        }
    }
}

pub trait Test<I: ?Sized> {
    /// In a run of [`RuleMatcher::get_best_match`] for an item, the context is reused across all tests.
    type Context<'a>;

    /// Test if an item passes. If so, it's score will be accumulated into the containing [`Rule`].
    fn passes<'a>(
        &self,
        item: &I,
        data: &Self::Context<'a>,
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

    /// Find the best matching rule for the item.
    ///
    /// # Notes
    /// - first one wins in tie
    /// - 0 score does not count
    /// - Early exit on 255
    pub fn get_best_match<'a, I: ?Sized>(
        &self,
        item: &I,
        context: T::Context<'a>,
    ) -> Option<&A>
    where
        T: Test<I>,
    {
        let mut best_id: Option<&A> = None;
        let mut best_score: u8 = 0;

        for (rules, id) in &self.rules {
            let mut score = 0u8;

            for r in rules {
                score = r.0.modify(score, r.1.passes(item, &context));
            }

            if score > best_score && score > 0 {
                best_score = score;
                best_id = Some(id);

                if best_score == u8::MAX {
                    break;
                }
            }
        }

        best_id
    }

    /// Find the best matching rule for the item together with its score
    /// and the matched `(score, test)` parts of the rule — for diagnostics.
    ///
    /// # Notes
    /// - first one wins in tie
    /// - 0 score does not count
    /// - Early exit on 255
    pub fn get_best_match_with_score<'a, I: ?Sized>(
        &self,
        item: &I,
        context: T::Context<'a>,
    ) -> Option<BestMatchWithScore<'_, A, T>>
    where
        T: Test<I>,
    {
        let mut best: Option<BestMatchWithScore<'_, A, T>> = None;

        for (rule, id) in &self.rules {
            let mut score = 0u8;

            for r in rule {
                score = r.0.modify(score, r.1.passes(item, &context));
            }

            if score > best.as_ref().map(|(_, s, _)| *s).unwrap_or(0) {
                best = Some((id, score, rule.as_slice()));

                if score == u8::MAX {
                    break;
                }
            }
        }

        best
    }

    // returns (top_score, best_scores)
    fn get_best_matches_with_score<'a, 'b, I: ?Sized>(
        &'a self,
        item: &I,
        context: T::Context<'b>,
    ) -> (u8, BestMatches<'a, T, A>)
    where
        T: Test<I>,
    {
        let mut max_score = 0u8;
        let mut top_indices = Vec::new();

        for (i, (rules, _id)) in self.rules.iter().enumerate() {
            let score = rules.iter().fold(0u8, |score, r| {
                r.0.modify(score, r.1.passes(item, &context))
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
    pub fn get_best_matches<'a, I: ?Sized>(
        &self,
        item: &I,
        context: T::Context<'a>,
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
    // pub fn prepend(
    //     &mut self,
    //     initial: &mut Self,
    // ) {
    //     initial.rules.append(&mut self.rules);
    //     std::mem::swap(initial, self);
    // }
    pub fn append(
        &mut self,
        initial: &mut Self,
    ) {
        self.rules.append(&mut initial.rules);
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
    if let Some(s) = s.strip_prefix('\\') {
        let r = T::from_str(s)?;
        return Ok((r.default_score(), r));
    }

    // single prefix alias
    if let Some((first, rest)) = s.split_at_checked(1) {
        let score = match first {
            "+" => Some(Score::Add(1)),
            "-" => Some(Score::Sub(1)),
            "<" => Some(Score::Min(1)),
            ">" => Some(Score::Max(1)),
            "^" => Some(Score::Req),
            _ => None,
        };

        if let Some(score) = score {
            if let Ok(r) = T::from_str(rest) {
                return Ok((score, r));
            }
        }
    }

    // parse | delimited
    if let Some((prefix, rest)) = s.split_once('|')
        && let Ok(r) = T::from_str(rest)
    {
        let score =
        // don't reflow
        if let Some(stripped) = prefix.strip_prefix('<') {
            stripped.parse().map(Score::Min)
        } else if let Some(stripped) = prefix.strip_prefix('>') {
            stripped.parse().map(Score::Max)
        } else if let Some(stripped) = prefix.strip_prefix('-') {
            stripped.parse().map(Score::Sub)
        } else if let Some(stripped) = prefix.strip_prefix('+') {
            stripped.parse().map(Score::Add)
        } else if prefix == "^" {
            Ok(Score::Req)
        } else {
            // 1|rule -> Max
            prefix.parse().map(Score::Max)
        };

        if let Ok(score) = score {
            return Ok((score, r));
        }
    }

    // default: parse whole string as rule, and fail on error
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
        Score::Max(v) => format!(">{}|{}", v, r),
        Score::Min(v) => format!("<{}|{}", v, r),
        Score::Req => format!("^|{}", r),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        abspath::AbsPath,
        lessfilter::{
            ActionEntry, ActionExecution, Categories, LessfilterSettings,
            action::Action,
            file_rule::{FileData, FileRule},
        },
    };
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    type TestMatcher = RuleMatcher<FileRule, ActionEntry>;

    fn entry(action: Action) -> ActionEntry {
        ActionEntry {
            rule: std::iter::once(action).collect(),
            execution: ActionExecution::All,
        }
    }

    /// A matcher whose rules are the `"score|rule"` strings, in order.
    fn build_matcher(rules: &[(&str, Action)]) -> TestMatcher {
        let mut matcher = TestMatcher::new();
        for (part, action) in rules {
            let (score, rule) = parse_rule_part::<FileRule>(part).unwrap();
            matcher.add(entry(action.clone()), vec![(score, rule)]);
        }
        matcher
    }

    fn file_in(
        dir: &tempfile::TempDir,
        name: &str,
        content: &[u8],
    ) -> std::path::PathBuf {
        let path = dir.path().join(name);
        File::create(&path).unwrap().write_all(content).unwrap();
        path
    }

    #[test]
    fn accumulated_scores_pick_the_best_rule() {
        let dir = tempdir().unwrap();
        let path = file_in(&dir, "main.rs", b"fn main() {}");
        let categories = Categories::default();
        let data = FileData::new(
            AbsPath::new(path.clone()),
            &LessfilterSettings::default(),
            &categories,
        );

        // two passing tests accumulate: 1 (ext:rs) + 1 (mime:text/*) = 2,
        // beating the catch-all rule that scores 1
        let mut matcher = TestMatcher::new();
        matcher.add(
            entry(Action::Text),
            vec![
                (Score::Add(1), "ext:rs".parse().unwrap()),
                (Score::Add(1), "mime:text/*".parse().unwrap()),
            ],
        );
        matcher.add(
            entry(Action::Metadata),
            vec![(Score::Add(1), "*".parse().unwrap())],
        );

        assert_eq!(
            matcher.get_best_match(&path, data),
            Some(&entry(Action::Text))
        );
    }

    #[test]
    fn max_score_wins_over_lower_scores() {
        let dir = tempdir().unwrap();
        let path = file_in(&dir, "main.rs", b"fn main() {}");
        let categories = Categories::default();
        let data = FileData::new(
            AbsPath::new(path.clone()),
            &LessfilterSettings::default(),
            &categories,
        );

        let matcher = build_matcher(&[
            ("1|ext:rs", Action::Text),
            ("40|mime:text/*", Action::Image),
            ("5|*", Action::Metadata),
        ]);

        assert_eq!(
            matcher.get_best_match(&path, data),
            Some(&entry(Action::Image))
        );
    }

    #[test]
    fn required_test_failure_eliminates_the_rule() {
        let dir = tempdir().unwrap();
        let path = file_in(&dir, "notes.txt", b"hello");
        let categories = Categories::default();
        let data = FileData::new(
            AbsPath::new(path.clone()),
            &LessfilterSettings::default(),
            &categories,
        );

        // a failing Req zeroes the rule's accumulated score; with the Req
        // last, the rule cannot match even though the catch-all test passed
        let mut matcher = TestMatcher::new();
        matcher.add(
            entry(Action::Text),
            vec![
                (Score::Add(1), "*".parse().unwrap()),
                (Score::Req, "ext:rs".parse().unwrap()),
            ],
        );
        assert_eq!(matcher.get_best_match(&path, data), None);

        // without the Req test the same rule matches
        let categories = Categories::default();
        let data = FileData::new(
            AbsPath::new(path.clone()),
            &LessfilterSettings::default(),
            &categories,
        );
        let matcher = build_matcher(&[("1|*", Action::Metadata)]);
        assert_eq!(
            matcher.get_best_match(&path, data),
            Some(&entry(Action::Metadata))
        );
    }

    #[test]
    fn ties_go_to_the_first_added_rule() {
        let dir = tempdir().unwrap();
        let path = file_in(&dir, "main.rs", b"fn main() {}");
        let categories = Categories::default();
        let data = FileData::new(
            AbsPath::new(path.clone()),
            &LessfilterSettings::default(),
            &categories,
        );

        let matcher = build_matcher(&[
            ("1|ext:rs", Action::Text),
            ("1|mime:text/*", Action::Metadata),
        ]);

        assert_eq!(
            matcher.get_best_match(&path, data),
            Some(&entry(Action::Text))
        );
    }

    #[test]
    fn zero_score_is_not_a_match() {
        let dir = tempdir().unwrap();
        let path = file_in(&dir, "main.rs", b"fn main() {}");
        let categories = Categories::default();
        let data = FileData::new(
            AbsPath::new(path.clone()),
            &LessfilterSettings::default(),
            &categories,
        );

        // a Sub that zeroes the score counts as no match at all
        let matcher = build_matcher(&[("-1|*", Action::Metadata)]);
        assert_eq!(matcher.get_best_match(&path, data), None);
    }
}
