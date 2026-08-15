use cba::define_collection_wrapper;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};

use crate::{
    abspath::AbsPath,
    lessfilter::{
        Categories, LessfilterSettings,
        file_rule::{FileData, FileRule},
        rule_matcher::Test,
    },
};

define_collection_wrapper!(
    /// Custom actions, keyed by name. Insertion order is the menu display order.
    #[derive(Debug, Clone, serde::Serialize)]
    #[serde(transparent)]
    MenuActions: IndexMap<String, MenuAction>
);

impl Default for MenuActions {
    fn default() -> Self {
        Self::new()
    }
}

/// Menu action keys reserved for the builtin queue kinds; defining an action
/// under one of these is a config error.
pub const RESERVED_KEYS: [&str; 5] = ["copy", "cut", "symlink", "app", "none"];

impl<'de> Deserialize<'de> for MenuActions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = IndexMap::<String, MenuAction>::deserialize(deserializer)?;
        if let Some(key) = map.keys().find(|k| RESERVED_KEYS.contains(&k.as_str())) {
            return Err(serde::de::Error::custom(format!(
                "menu action key {key:?} is reserved"
            )));
        }
        Ok(Self::new_from(map))
    }
}

/// A menu action is activated through [`crate::ui::menu_overlay::MenuOverlay`]
/// and executes a user-defined lua script on the focused items.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MenuAction {
    /// The action is visible in the menu iff at least one condition is
    /// satisfied; an empty list means always visible. Accepts a single
    /// condition without the surrounding array.
    #[serde(default, with = "cba::bird::one_or_many")]
    pub condition: Vec<MenuCondition>,
    /// Lua script executed for this action (`@file` syntax supported).
    pub command: String,
    /// Alias shown in the second column of the menu (e.g. `w`).
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub strategy: MenuStrategy,
    /// Overrides the strategy's default closing behavior: `Some(true)` always
    /// closes the menu after the action is chosen, `Some(false)` keeps it
    /// open, `None` follows the strategy default.
    #[serde(default)]
    pub close: Option<bool>,
}

impl MenuAction {
    /// Whether choosing this action closes the menu: `close` overrides the
    /// strategy default (Execute/ExecuteSilent/ExecPaged exit, Stash/Batch keep
    /// open).
    pub fn closes(&self) -> bool {
        self.close.unwrap_or(matches!(
            self.strategy,
            MenuStrategy::Execute | MenuStrategy::ExecuteSilent | MenuStrategy::ExecPaged
        ))
    }
}

/// How a menu action runs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MenuStrategy {
    /// Run the action's command through the lua engine and wait for it.
    Execute,
    /// Run the action's command through the lua engine without waiting.
    #[default]
    #[serde(alias = "silent")]
    ExecuteSilent,
    /// Run the action's command through the lua engine, wait for it, and page
    /// its stdout.
    ExecPaged,
    /// Enqueue all target items into a single queue item and keep the menu open.
    Stash,
    /// Enqueue the target items into queue items of at most `n` paths each and
    /// keep the menu open.
    Batch(usize),
}

/// A criterion evaluated against the picker state at menu open.
/// The action is visible iff at least one condition is satisfied.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MenuCondition {
    /// Positional: exactly as many items must be selected as there are rules,
    /// and rule *i* must match the *i*-th selected item in selection order.
    Seq(Vec<FileRule>),
    /// Repetition of a single rule, scoped by count.
    Repeat {
        /// `Some(0)`: prompt-scoped — strict requires the prompt to be active,
        /// non-strict shows outside it too; both require a cwd, and the rule
        /// is evaluated against the cwd.
        /// `Some(n ≥ 1)`: selection-scoped — strict requires exactly *n*
        /// selected items, non-strict at least *n*; the rule must match every
        /// selected item.
        /// `None`: cursor-scoped — requires an enabled cursor with an item;
        /// strict hides the action whenever anything is selected, and the rule
        /// must match the cursor item.
        #[serde(default)]
        count: Option<usize>,
        condition: FileRule,
        #[serde(default)]
        strict: bool,
    },
}

/// Whether at least one condition is satisfied against the state at menu
/// open. An empty condition list means always visible. `cache` holds the
/// [`FileData`] computed per file so each file's data is built at most once
/// per menu open.
pub fn condition_passes<'a>(
    conditions: &[MenuCondition],
    selected: &[AbsPath],
    cursor: Option<&AbsPath>,
    in_prompt: bool,
    cwd: Option<&AbsPath>,
    cache: &mut Vec<(AbsPath, FileData<'a>)>,
    settings: &LessfilterSettings,
    categories: &'a Categories,
) -> bool {
    conditions.is_empty()
        || conditions.iter().any(|c| {
            c.passes(
                selected, cursor, in_prompt, cwd, cache, settings, categories,
            )
        })
}

fn data_for<'c, 'a>(
    cache: &'c mut Vec<(AbsPath, FileData<'a>)>,
    settings: &LessfilterSettings,
    categories: &'a Categories,
    path: &AbsPath,
) -> Option<&'c FileData<'a>> {
    if let Some(i) = cache.iter().position(|(p, _)| p == path) {
        return cache.get(i).map(|(_, d)| d);
    }
    cache.push((
        path.clone(),
        FileData::new(path.clone(), settings, categories),
    ));
    cache.last().map(|(_, d)| d)
}

impl MenuCondition {
    fn passes<'a>(
        &self,
        selected: &[AbsPath],
        cursor: Option<&AbsPath>,
        in_prompt: bool,
        cwd: Option<&AbsPath>,
        cache: &mut Vec<(AbsPath, FileData<'a>)>,
        settings: &LessfilterSettings,
        categories: &'a Categories,
    ) -> bool {
        let mut passes_rule = |path: &AbsPath, rule: &FileRule| {
            data_for(cache, settings, categories, path).is_some_and(|d| rule.passes(path, d))
        };
        match self {
            MenuCondition::Seq(rules) => {
                selected.len() == rules.len()
                    && selected
                        .iter()
                        .zip(rules)
                        .all(|(path, rule)| passes_rule(path, rule))
            }
            MenuCondition::Repeat {
                count,
                condition,
                strict,
            } => match count {
                Some(0) => {
                    if *strict && !in_prompt {
                        return false;
                    }
                    cwd.is_some_and(|cwd| passes_rule(cwd, condition))
                }
                Some(n) => {
                    if *strict {
                        if selected.len() != *n {
                            return false;
                        }
                    } else if selected.len() < *n {
                        return false;
                    }
                    selected.iter().all(|path| passes_rule(path, condition))
                }
                None => {
                    if *strict && !selected.is_empty() {
                        return false;
                    }
                    cursor.is_some_and(|cursor| passes_rule(cursor, condition))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_keys_are_rejected() {
        let err = toml::from_str::<MenuActions>(
            "\
            [copy]\n\
            command = \"print('x')\"\n\
        ",
        )
        .unwrap_err();
        assert!(err.to_string().contains("reserved"), "{err}");
    }

    #[test]
    fn non_reserved_keys_parse() {
        let actions: MenuActions = toml::from_str(
            "\
            [my-action]\n\
            command = \"print('x')\"\n\
            strategy = \"Stash\"\n\
        ",
        )
        .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions["my-action"].strategy, MenuStrategy::Stash);
    }
}

#[cfg(test)]
mod single_condition_tests {
    use super::*;
    use crate::lessfilter::file_rule::{FileRule, FileRuleKind};

    #[test]
    fn single_condition_object_parses() {
        // the single-object condition form (one_or_many) with lowercase keys
        let actions: MenuActions = toml::from_str(
            "\
            [my-action]
\
            condition = { repeat = { condition = \"git\", strict = true } }
\
            command = \"print('x')\"
\
            strategy = \"ExecPaged\"
\
        ",
        )
        .unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            &actions["my-action"].condition[0],
            MenuCondition::Repeat {
                count: None,
                condition: FileRule {
                    invert: false,
                    kind: FileRuleKind::Git
                },
                strict: true
            }
        ));
        assert_eq!(actions["my-action"].strategy, MenuStrategy::ExecPaged);
    }

    #[test]
    fn test_seq_condition_evaluation() {
        let toml_str = include_str!("../../assets/config/dev.toml");
        let cfg: crate::config::Config = toml::from_str(toml_str).unwrap();
        let stash_action = &cfg.actions["stash: 2 items"];

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let todo_md = AbsPath::new(root.join("TODO.md"));
        let todo_dir = AbsPath::new(root.join("TODO"));

        let settings = LessfilterSettings::default();
        let categories = Categories::default();

        let mut cache = Vec::new();

        // Selected [TODO.md, TODO] (file, dir)
        let selected = vec![todo_md.clone(), todo_dir.clone()];
        let passes = condition_passes(
            &stash_action.condition,
            &selected,
            None,
            false,
            Some(&AbsPath::new(root.to_path_buf())),
            &mut cache,
            &settings,
            &categories,
        );
        assert!(
            passes,
            "stash: 2 items should pass when file and dir are selected in order"
        );
    }
}
