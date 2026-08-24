//! Menu actions and condition evaluation for custom actions.

pub mod plugins;

use cba::{define_collection_wrapper, vecmap::VecMap};
use indexmap::IndexMap;
use matchmaker::render::MMState;
use serde::{Deserialize, Deserializer};

use crate::{
    abspath::AbsPath,
    lessfilter::{
        Categories, LessfilterConfig, LessfilterSettings,
        file_rule::{FileData, FileRule},
        rule_matcher::Test,
    },
    run::{item::PathItem, register::resolve_target, state::STACK},
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

/// Menu action keys reserved for the builtin queue kinds and the queue
/// selectors; defining an action under one of these (case-insensitively) or
/// under the empty key is a config error.
pub const RESERVED_KEYS: [&str; 10] = [
    "copy", "move", "symlink", "none", "all", "builtins", "first", "last", "default", "",
];

impl<'de> Deserialize<'de> for MenuActions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = IndexMap::<String, MenuAction>::deserialize(deserializer)?;
        if let Some(key) = map
            .keys()
            .find(|k| RESERVED_KEYS.iter().any(|r| k.eq_ignore_ascii_case(r)))
        {
            return Err(serde::de::Error::custom(format!(
                "menu action key {key:?} is reserved"
            )));
        }
        Ok(Self::new_from(map))
    }
}

impl MenuActions {
    /// Load the merged menu actions: the primary actions file plus every
    /// `*.toml` in the actions folder and subfolders, merged with innermost
    /// subfolders resolved first, then sorted path order (numeric prefixes order them).
    /// The primary file's entries come first; a key defined in a later file is an error.
    /// A missing primary file or folder yields the empty set.
    pub fn load_all(
        primary: &std::path::Path,
        dir: &std::path::Path,
    ) -> Result<Self, String> {
        let mut merged = Self::new();
        if primary.is_file() {
            let content = std::fs::read_to_string(primary)
                .map_err(|e| format!("{}: {e}", primary.display()))?;
            merge_actions(&mut merged, primary, &content)?;
        }
        let mut files = Vec::new();
        fn collect_toml_files(
            dir: &std::path::Path,
            files: &mut Vec<std::path::PathBuf>,
        ) -> Result<(), std::io::Error> {
            if dir.is_dir() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        collect_toml_files(&path, files)?;
                    } else if path.extension().is_some_and(|x| x == "toml") {
                        files.push(path);
                    }
                }
            }
            Ok(())
        }

        if let Err(e) = collect_toml_files(dir, &mut files) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("{}: {e}", dir.display()));
            }
        }

        // Sort files by path component depth descending (innermost folders first),
        // breaking ties with standard lexicographical path order.
        files.sort_by(|a, b| {
            let depth_a = a.components().count();
            let depth_b = b.components().count();
            depth_b.cmp(&depth_a).then_with(|| a.cmp(b))
        });

        for path in files {
            let content =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            merge_actions(&mut merged, &path, &content)?;
        }
        Ok(merged)
    }
}

fn merge_actions(
    merged: &mut MenuActions,
    path: &std::path::Path,
    content: &str,
) -> Result<(), String> {
    let actions: MenuActions =
        toml::from_str(content).map_err(|e| format!("{}: {e}", path.display()))?;
    for (key, action) in actions.inner {
        if merged.contains_key(&key) {
            return Err(format!(
                "duplicate menu action key {key:?} in {}",
                path.display()
            ));
        }
        merged.insert(key, action);
    }
    Ok(())
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
    /// Whether queued executions of this action require a non-empty
    /// destination: `All` silently skips such a row when its destination is
    /// empty, an exact selector reports an error.
    #[serde(default)]
    pub requires_dest: bool,
    /// Overrides the strategy's default closing behavior: `Some(true)` always
    /// closes the menu after the action is chosen, `Some(false)` keeps it
    /// open, `None` follows the strategy default.
    #[serde(default)]
    pub close: Option<bool>,
}

impl MenuAction {
    /// Whether choosing this action closes the menu: `close` overrides the
    /// strategy default (Execute/ExecuteSilent/ExecPaged exit, Queue/QueueBatch
    /// keep open).
    pub fn closes(&self) -> bool {
        self.close.unwrap_or(matches!(
            self.strategy,
            MenuStrategy::Execute | MenuStrategy::ExecuteSilent | MenuStrategy::ExecPaged
        ))
    }

    /// Pure target resolution according to action condition variants.
    pub fn resolve_targets(
        &self,
        selected: Vec<AbsPath>,
        fallback_target: AbsPath,
        cursor_disabled: bool,
        cwd: Option<AbsPath>,
        nav_cwd: Option<AbsPath>,
    ) -> Vec<AbsPath> {
        if self.condition.iter().any(|c| {
            matches!(
                c,
                MenuCondition::Repeat(RepeatCondition {
                    selected: SelectedCondition::Cwd,
                    strict: true,
                    ..
                })
            )
        }) {
            nav_cwd
                .map(|p| vec![p])
                .unwrap_or_else(|| vec![fallback_target])
        } else if self.condition.iter().any(|c| {
            matches!(
                c,
                MenuCondition::Repeat(RepeatCondition {
                    selected: SelectedCondition::Cwd,
                    ..
                })
            )
        }) {
            cwd.map(|p| vec![p])
                .unwrap_or_else(|| vec![fallback_target])
        } else if self.condition.iter().any(|c| {
            matches!(
                c,
                MenuCondition::Repeat(RepeatCondition {
                    selected: SelectedCondition::Active,
                    ..
                })
            )
        }) && selected.is_empty()
            && cursor_disabled
        {
            cwd.map(|p| vec![p])
                .unwrap_or_else(|| vec![fallback_target])
        } else if selected.is_empty() {
            vec![fallback_target]
        } else {
            selected
        }
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
    Queue,
    /// Enqueue the target items into queue items of at most `n` paths each and
    /// keep the menu open.
    QueueBatch(usize),
}

/// Repetition of a single rule, scoped by [`SelectedCondition`].
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepeatCondition {
    /// Which state the rule is evaluated against.
    #[serde(default)]
    pub selected: SelectedCondition,
    pub condition: FileRule,
    #[serde(default)]
    pub strict: bool,
}

/// A criterion evaluated against the picker state at menu open.
/// The action is visible iff at least one condition is satisfied.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum MenuCondition {
    /// Positional: exactly as many items must be selected as there are rules,
    /// and rule *i* must match the *i*-th selected item in selection order.
    Seq(Vec<FileRule>),
    /// Repetition of a single rule, scoped by [`SelectedCondition`].
    Repeat(RepeatCondition),
}

/// The picker state a [`MenuCondition::Repeat`] rule is evaluated against.
///
/// Serialized as the lowercase variant name, or as a bare usize for
/// [`SelectedCondition::Selections`] (`selected = 2`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SelectedCondition {
    /// The current results row item. Requires an enabled cursor with an item;
    /// strict additionally requires that nothing is selected. The rule is
    /// evaluated against the cursor item.
    #[default]
    Cursor,
    /// The current directory. Non-strict requires a current directory while
    /// the cursor is disabled (the prompt state); strict requires the Nav
    /// pane directory, regardless of the cursor. The rule is evaluated
    /// against that directory.
    Cwd,
    /// The selected items: strict requires exactly *n*, non-strict at least
    /// *n*. The rule must match every selected item.
    Selections(usize),
    /// One active target: the selected items when any are selected (any amount),
    /// the cursor item otherwise, falling back to the current directory while the
    /// cursor is disabled; fails when none of these resolve. Strict requires
    /// the resolved target set to contain exactly one path. The rule must
    /// match every path in the target set.
    Active,
}

impl serde::Serialize for SelectedCondition {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match self {
            SelectedCondition::Cursor => serializer.serialize_str("cursor"),
            SelectedCondition::Cwd => serializer.serialize_str("cwd"),
            SelectedCondition::Selections(n) => serializer.serialize_u64(*n as u64),
            SelectedCondition::Active => serializer.serialize_str("active"),
        }
    }
}

impl<'de> Deserialize<'de> for SelectedCondition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = SelectedCondition;

            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("a non-negative integer or \"cursor\", \"cwd\", \"active\"")
            }

            fn visit_u64<E: serde::de::Error>(
                self,
                v: u64,
            ) -> Result<Self::Value, E> {
                Ok(SelectedCondition::Selections(v as usize))
            }

            fn visit_i64<E: serde::de::Error>(
                self,
                v: i64,
            ) -> Result<Self::Value, E> {
                if v < 0 {
                    Err(E::custom("selections count must be non-negative"))
                } else {
                    Ok(SelectedCondition::Selections(v as usize))
                }
            }

            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> Result<Self::Value, E> {
                match v {
                    "cursor" => Ok(SelectedCondition::Cursor),
                    "cwd" => Ok(SelectedCondition::Cwd),
                    "active" | "single" => Ok(SelectedCondition::Active),
                    other => Err(E::unknown_variant(other, &["cursor", "cwd", "active"])),
                }
            }

            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> Result<Self::Value, E> {
                self.visit_str(&v)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// Whether at least one condition is satisfied against the state at menu
/// open. An empty condition list means always visible. `cache` holds the
/// [`FileData`] computed per file so each file's data is built at most once
/// per menu open.
pub fn condition_passes<'a>(
    conditions: &[MenuCondition],
    selected: &[AbsPath],
    cursor: Option<&AbsPath>,
    cursor_disabled: bool,
    cwd: Option<&AbsPath>,
    nav_cwd: Option<&AbsPath>,
    cache: &mut VecMap<AbsPath, FileData<'a>>,
    settings: &LessfilterSettings,
    categories: &'a Categories,
) -> bool {
    conditions.is_empty()
        || conditions.iter().any(|c| {
            c.passes(
                selected,
                cursor,
                cursor_disabled,
                cwd,
                nav_cwd,
                cache,
                settings,
                categories,
            )
        })
}

fn data_for<'c, 'a>(
    cache: &'c mut VecMap<AbsPath, FileData<'a>>,
    settings: &LessfilterSettings,
    categories: &'a Categories,
    path: &AbsPath,
) -> Option<&'c FileData<'a>> {
    if cache.contains_key(path) {
        cache.get(path)
    } else {
        Some(cache.get_or_insert(
            path.clone(),
            FileData::new(path.clone(), settings, categories),
        ))
    }
}

impl MenuCondition {
    fn passes<'a>(
        &self,
        selected: &[AbsPath],
        cursor: Option<&AbsPath>,
        cursor_disabled: bool,
        cwd: Option<&AbsPath>,
        nav_cwd: Option<&AbsPath>,
        cache: &mut VecMap<AbsPath, FileData<'a>>,
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
            MenuCondition::Repeat(RepeatCondition {
                selected: which,
                condition,
                strict,
            }) => match which {
                SelectedCondition::Cursor => {
                    if cursor_disabled {
                        return false;
                    }
                    if *strict && !selected.is_empty() {
                        return false;
                    }
                    cursor.is_some_and(|cursor| passes_rule(cursor, condition))
                }
                SelectedCondition::Cwd => {
                    if *strict {
                        nav_cwd.is_some_and(|nav| passes_rule(nav, condition))
                    } else {
                        cursor_disabled && cwd.is_some_and(|cwd| passes_rule(cwd, condition))
                    }
                }
                SelectedCondition::Selections(n) => {
                    if *strict {
                        if selected.len() != *n {
                            return false;
                        }
                    } else if selected.len() < *n {
                        return false;
                    }
                    selected.iter().all(|path| passes_rule(path, condition))
                }
                SelectedCondition::Active => {
                    // resolve the target set: selections → cursor → cwd
                    let targets: Vec<&AbsPath> = if !selected.is_empty() {
                        selected.iter().collect()
                    } else if !cursor_disabled {
                        match cursor {
                            Some(cursor) => vec![cursor],
                            None => return false,
                        }
                    } else {
                        match cwd {
                            Some(cwd) => vec![cwd],
                            None => return false,
                        }
                    };
                    if *strict && targets.len() != 1 {
                        return false;
                    }
                    targets.iter().all(|path| passes_rule(path, condition))
                }
            },
        }
    }
}

pub struct MenuEvaluationContext<'a> {
    pub selected: Vec<AbsPath>,
    pub cursor: Option<AbsPath>,
    pub cursor_disabled: bool,
    pub cwd: Option<AbsPath>,
    pub nav_cwd: Option<AbsPath>,
    pub fallback: AbsPath,
    pub cache: VecMap<AbsPath, FileData<'a>>,
    pub settings: &'a LessfilterSettings,
    pub categories: &'a Categories,
}

impl<'a> MenuEvaluationContext<'a> {
    pub fn new(
        state: &MMState<'_, PathItem, ()>,
        lcfg: &'a LessfilterConfig,
    ) -> Self {
        let selected: Vec<AbsPath> = state.map_selections_to_vec(|_, item| item.path.clone());
        let cursor_disabled = state.picker_ui.results.cursor_disabled();
        let cursor = if cursor_disabled {
            None
        } else {
            state.current_raw().map(|item| item.path.clone())
        };
        let cwd = STACK::cwd();
        let nav_cwd = STACK::nav_cwd();
        let fallback = resolve_target(state, true)
            .or_else(STACK::cwd)
            .unwrap_or_else(STACK::_cwd);
        Self {
            selected,
            cursor,
            cursor_disabled,
            cwd,
            nav_cwd,
            fallback,
            cache: VecMap::new(),
            settings: &lcfg.settings,
            categories: &lcfg.categories,
        }
    }

    pub fn is_applicable(
        &mut self,
        action: &MenuAction,
    ) -> bool {
        condition_passes(
            &action.condition,
            &self.selected,
            self.cursor.as_ref(),
            self.cursor_disabled,
            self.cwd.as_ref(),
            self.nav_cwd.as_ref(),
            &mut self.cache,
            self.settings,
            self.categories,
        )
    }

    pub fn resolve_targets(
        &self,
        action: &MenuAction,
    ) -> Vec<AbsPath> {
        action.resolve_targets(
            self.selected.clone(),
            self.fallback.clone(),
            self.cursor_disabled,
            self.cwd.clone(),
            self.nav_cwd.clone(),
        )
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
    fn selector_names_and_empty_keys_are_rejected() {
        for key in ["all", "builtins", "first", "last", "none", "", "ALL"] {
            let actions = toml::from_str::<MenuActions>(&format!(
                "\
                [{key}]\n\
                command = \"print('x')\"\n\
            "
            ));
            assert!(actions.is_err(), "key {key:?} should be reserved");
        }
    }

    #[test]
    fn non_reserved_keys_parse() {
        let actions: MenuActions = toml::from_str(
            "\
            [my-action]\n\
            command = \"print('x')\"\n\
            strategy = \"Queue\"\n\
            requires_dest = true\n\
        ",
        )
        .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions["my-action"].strategy, MenuStrategy::Queue);
        assert!(actions["my-action"].requires_dest);
    }

    #[test]
    fn load_all_recursive_innermost_first() {
        let temp_dir = std::env::temp_dir().join("fist_test_recursive_actions");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let sub_dir = temp_dir.join("sub").join("inner");
        std::fs::create_dir_all(&sub_dir).unwrap();

        let top_file = temp_dir.join("top.toml");
        let inner_file = sub_dir.join("inner.toml");

        std::fs::write(&top_file, "[top_action]\ncommand = \"print('top')\"\n").unwrap();
        std::fs::write(
            &inner_file,
            "[inner_action]\ncommand = \"print('inner')\"\n",
        )
        .unwrap();

        let primary = temp_dir.join("primary.toml");
        let actions = MenuActions::load_all(&primary, &temp_dir).unwrap();

        // Check both actions are present
        assert!(actions.contains_key("top_action"));
        assert!(actions.contains_key("inner_action"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
