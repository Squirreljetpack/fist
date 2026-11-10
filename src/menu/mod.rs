//! Menu actions and condition evaluation for custom actions.

pub mod plugins;

use std::collections::HashSet;

use cba::{define_collection_wrapper, vecmap::VecMap};
use indexmap::IndexMap;
use matchmaker::render::MMState;
use serde::{Deserialize, Deserializer, Serialize as _, Serializer, ser::SerializeSeq};

use crate::{
    abspath::AbsPath,
    lessfilter::{
        Categories, LessfilterConfig, LessfilterSettings,
        file_rule::{FileData, FileRule, FileRuleKind},
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

/// Sentinel `prefix` replaced with the stem of the declaring file at load
/// time; an empty prefix drops the prefix from the menu row entirely.
pub const DEFAULT_PREFIX: &str = ":";

fn default_prefix() -> String {
    DEFAULT_PREFIX.to_string()
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
        // an enqueued row starts right away, so nothing can supply the
        // destination `requires_dest` waits for
        if let Some(key) = map.iter().find_map(|(k, a)| {
            (a.strategy == MenuStrategy::Enqueue && a.requires_dest).then_some(k)
        }) {
            return Err(serde::de::Error::custom(format!(
                "menu action {key:?} cannot combine `requires_dest` with \
                 `strategy = \"Enqueue\"`: the row starts immediately"
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
    ///
    /// A file that fails to parse or redefine an existing key is skipped whole
    /// and its error returned, so one bad file neither aborts loading nor
    /// hides the rest.
    ///
    /// With `filter`, actions that can never fire because every alternative
    /// requires a program that is not installed are dropped; pass `false` to
    /// keep them so tooling can inspect them.
    pub fn load_all(
        primary: &std::path::Path,
        dir: &std::path::Path,
        filter: bool,
    ) -> (Self, Vec<String>) {
        let mut merged = Self::new();
        let mut seen = HashSet::new();
        let mut errors = Vec::new();
        if primary.is_file() {
            match std::fs::read_to_string(primary) {
                Ok(content) => {
                    if let Err(e) = merge_actions(&mut merged, &mut seen, primary, &content, filter)
                    {
                        errors.push(e);
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", primary.display())),
            }
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

        if let Err(e) = collect_toml_files(dir, &mut files)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            errors.push(format!("{}: {e}", dir.display()));
        }

        // Sort files by path component depth descending (innermost folders first),
        // breaking ties with standard lexicographical path order.
        files.sort_by(|a, b| {
            let depth_a = a.components().count();
            let depth_b = b.components().count();
            depth_b.cmp(&depth_a).then_with(|| a.cmp(b))
        });

        for path in files {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    if let Err(e) = merge_actions(&mut merged, &mut seen, &path, &content, filter) {
                        errors.push(e);
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        (merged, errors)
    }
}

fn merge_actions(
    merged: &mut MenuActions,
    seen: &mut HashSet<String>,
    path: &std::path::Path,
    content: &str,
    filter: bool,
) -> Result<(), String> {
    let actions: MenuActions =
        toml::from_str(content).map_err(|e| format!("{}: {e}", path.display()))?;
    // Duplicate keys are a config error whether or not the action survives
    // filtering: an action gated on an uninstalled program must not let a
    // later file redefine its key only in normal runs.
    if let Some(key) = actions.keys().find(|key| seen.contains(*key)) {
        return Err(format!(
            "duplicate menu action key {key:?} in {}",
            path.display()
        ));
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    // a file merges whole or not at all: never leave half of it behind
    let mut resolved = Vec::with_capacity(actions.inner.len());
    for (key, mut action) in actions.inner {
        // a gate on a program that is not installed would hide the action
        // anyway, but its key is still taken
        if filter && !action.retain_possible_alternatives() {
            seen.insert(key);
            continue;
        }
        if action.prefix == DEFAULT_PREFIX {
            action.prefix = stem.clone();
        }
        resolved.push((key, action));
    }
    for (key, action) in resolved {
        seen.insert(key.clone());
        merged.insert(key, action);
    }
    Ok(())
}

/// A menu action is activated through [`crate::ui::menu_overlay::MenuOverlay`]
/// and executes a user-defined lua script on the focused items.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MenuAction {
    /// The action is visible in the menu iff at least one alternative passes;
    /// pure alternatives are written `{ any = [ ... ] }`, while a lone
    /// condition needs no wrapper. An empty list means always visible.
    #[serde(
        default,
        deserialize_with = "deserialize_conditions",
        serialize_with = "serialize_conditions"
    )]
    pub condition: Vec<MenuCondition>,
    /// Lua script executed for this action (`@file` syntax supported).
    pub command: String,
    /// Menu group label: the row reads `{prefix}: {key}`. The default `:`
    /// resolves to the stem of the file declaring the action, and `""`
    /// drops the prefix from the row.
    #[serde(default = "default_prefix")]
    pub prefix: String,
    /// Alias shown in the second column of the menu (e.g. `w`).
    #[serde(default)]
    pub alias: Option<String>,
    /// Free-form description of the action. Not rendered anywhere yet, it is
    /// there for documentation and tooling.
    #[serde(default)]
    pub help: String,
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
/// Read a `condition` field: a lone condition, or `{ any = [ ... ] }`
/// alternatives.
pub fn deserialize_conditions<'de, D>(deserializer: D) -> Result<Vec<MenuCondition>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Any {
        any: Vec<MenuCondition>,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        Any(Any),
        One(MenuCondition),
    }

    Ok(match Helper::deserialize(deserializer)? {
        Helper::Any(any) if any.any.is_empty() => {
            return Err(serde::de::Error::custom(
                "an `any` condition needs at least one alternative",
            ));
        }
        Helper::Any(any) => any.any,
        Helper::One(one) => vec![one],
    })
}

/// Write a `condition` back in the shape [`deserialize_conditions`] reads.
pub fn serialize_conditions<S>(
    conditions: &[MenuCondition],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(serde::Serialize)]
    struct Any<'a> {
        any: &'a [MenuCondition],
    }

    match conditions {
        [] => serializer.serialize_seq(Some(0))?.end(),
        [one] => one.serialize(serializer),
        many => Any { any: many }.serialize(serializer),
    }
}

impl MenuAction {
    /// The row label: `{prefix}: {key}`, or just `{key}` when the prefix is
    /// empty.
    pub fn display_name(
        &self,
        key: &str,
    ) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}: {key}", self.prefix)
        }
    }

    /// Remove the alternatives that require a program that is not installed,
    /// since they can never pass. Returns whether anything is left to show:
    /// an action whose every alternative is gone is invisible for good.
    fn retain_possible_alternatives(&mut self) -> bool {
        // no conditions at all means always visible
        if self.condition.is_empty() {
            return true;
        }
        self.condition.retain(|condition| !condition.fails_has());
        !self.condition.is_empty()
    }

    /// Whether choosing this action closes the menu: `close` overrides the
    /// strategy default (Exec/ExecWait/ExecSilent/ExecPaged/Enqueue exit,
    /// Queue/QueueBatch keep open).
    pub fn closes(&self) -> bool {
        self.close.unwrap_or(matches!(
            self.strategy,
            MenuStrategy::Exec
                | MenuStrategy::ExecWait
                | MenuStrategy::ExecSilent
                | MenuStrategy::ExecPaged
                | MenuStrategy::Enqueue
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
    #[serde(alias = "Execute")]
    Exec,
    /// Run the action's command through the lua engine, wait for it, and then prompt for Enter.
    #[serde(alias = "ExecuteWait")]
    ExecWait,
    /// Run the action's command through the lua engine in the background.
    #[default]
    #[serde(alias = "ExecuteSilent", alias = "silent")]
    ExecSilent,
    /// Run the action's command through the lua engine, wait for it, and page
    /// its stdout.
    #[serde(alias = "ExecutePaged")]
    ExecPaged,
    /// Enqueue all target items into a single queue item and keep the menu open.
    Queue,
    /// Enqueue the target items into queue items of at most `n` paths each and
    /// keep the menu open.
    QueueBatch(usize),
    /// Enqueue all target items into a single queue item and start it right
    /// away.
    Enqueue,
}

/// Repetition of a rule set, scoped by [`SelectedCondition`].
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepeatCondition {
    /// Which state the rule set is evaluated against.
    #[serde(default)]
    pub selected: SelectedCondition,
    /// The rules every target must satisfy; defaults to `*` (any target).
    #[serde(default = "default_rules")]
    pub condition: RuleSet,
    #[serde(default)]
    pub strict: bool,
}

/// The rule set a [`RepeatCondition`] falls back to when it omits one.
fn default_rules() -> RuleSet {
    RuleSet(vec!["*".parse().expect("`*` is a valid file rule")])
}

/// One or more [`FileRule`]s that must all pass: a set passes iff every rule in
/// it passes, so a single rule and a one-element set are equivalent. Written as
/// a bare rule string or as an array of them.
#[derive(Debug, Clone)]
pub struct RuleSet(Vec<FileRule>);

impl RuleSet {
    pub fn rules(&self) -> &[FileRule] {
        &self.0
    }

    /// Whether every rule in the set passes.
    fn passes(
        &self,
        rule_passes: impl FnMut(&FileRule) -> bool,
    ) -> bool {
        self.0.iter().all(rule_passes)
    }

    /// Whether one of the rules requires a program that is not installed, so
    /// the set can never pass.
    fn fails_has(&self) -> bool {
        self.0.iter().any(rule_fails_has)
    }
}

impl From<FileRule> for RuleSet {
    fn from(rule: FileRule) -> Self {
        Self(vec![rule])
    }
}

impl From<Vec<FileRule>> for RuleSet {
    fn from(rules: Vec<FileRule>) -> Self {
        Self(rules)
    }
}

impl<'de> serde::Deserialize<'de> for RuleSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        cba::bird::one_or_many::deserialize(deserializer).map(Self)
    }
}

impl serde::Serialize for RuleSet {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        cba::bird::one_or_many::serialize(&self.0, serializer)
    }
}

/// Whether a single rule requires a program that is not on `PATH`.
fn rule_fails_has(rule: &FileRule) -> bool {
    !rule.invert && matches!(&rule.kind, FileRuleKind::Have(program) if !cba::broc::has(program))
}

/// A criterion evaluated against the picker state at menu open.
/// The action is visible iff at least one of the conditions is satisfied;
/// none of them = always visible.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum MenuCondition {
    /// Positional: exactly as many items must be selected as there are sets,
    /// and set *i* must match the *i*-th selected item in selection order.
    Seq(Vec<RuleSet>),
    /// Repetition of a rule set, scoped by [`SelectedCondition`].
    Repeat(RepeatCondition),
}

impl MenuCondition {
    /// Whether the condition can never pass because it requires a program that
    /// is not installed.
    fn fails_has(&self) -> bool {
        match self {
            // every position must pass, so one bad position kills the seq
            MenuCondition::Seq(sets) => sets.iter().any(RuleSet::fails_has),
            // the rules of one repeat are all required at once
            MenuCondition::Repeat(repeat) => repeat.condition.fails_has(),
        }
    }
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

/// Whether the action is applicable against the state at menu open.
/// `cache` holds the [`FileData`] computed per file so each file's data is
/// built at most once per menu open.
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
        let mut passes_rules = |path: &AbsPath, rules: &RuleSet| {
            rules.passes(|rule| {
                data_for(cache, settings, categories, path).is_some_and(|d| rule.passes(path, d))
            })
        };
        match self {
            MenuCondition::Seq(sets) => {
                selected.len() == sets.len()
                    && selected
                        .iter()
                        .zip(sets)
                        .all(|(path, rules)| passes_rules(path, rules))
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
                    cursor.is_some_and(|cursor| passes_rules(cursor, condition))
                }
                SelectedCondition::Cwd => {
                    if *strict {
                        nav_cwd.is_some_and(|nav| passes_rules(nav, condition))
                    } else {
                        cursor_disabled && cwd.is_some_and(|cwd| passes_rules(cwd, condition))
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
                    selected.iter().all(|path| passes_rules(path, condition))
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
                    targets.iter().all(|path| passes_rules(path, condition))
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
    fn silent_strategy_alias_parse() {
        let actions: MenuActions = toml::from_str(
            "\
            [my-action]\n\
            command = \"print('x')\"\n\
            strategy = \"silent\"\n\
        ",
        )
        .unwrap();
        assert_eq!(actions["my-action"].strategy, MenuStrategy::ExecSilent);
    }

    #[test]
    fn duplicate_keys_error_even_when_filtered() {
        let temp_dir = std::env::temp_dir().join("fist_test_duplicate_filtered");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(
            temp_dir.join("a.toml"),
            "\
            [dup]\n\
            condition = { condition = [\"*\", \"have:no-such-program-fist\"] }\n\
            command = \"print('a')\"\n\
        ",
        )
        .unwrap();
        std::fs::write(
            temp_dir.join("b.toml"),
            "\
            [dup]\n\
            command = \"print('b')\"\n\
        ",
        )
        .unwrap();

        let (actions, errors) =
            MenuActions::load_all(&temp_dir.join("primary.toml"), &temp_dir, true);
        assert!(actions.is_empty());
        assert!(errors.iter().any(|e| e.contains("duplicate")), "{errors:?}");

        let _ = std::fs::remove_dir_all(&temp_dir);
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
        let (actions, errors) = MenuActions::load_all(&primary, &temp_dir, true);
        assert!(errors.is_empty(), "unexpected load errors: {errors:?}");

        // Check both actions are present
        assert!(actions.contains_key("top_action"));
        assert!(actions.contains_key("inner_action"));

        // the default prefix resolves to the stem of the declaring file
        assert_eq!(actions["top_action"].prefix, "top");
        assert_eq!(actions["inner_action"].prefix, "inner");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn load_all_drops_failing_has_gates() {
        let temp_dir = std::env::temp_dir().join("fist_test_has_gates");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(
            temp_dir.join("gates.toml"),
            "\
            [installed]\n\
            condition = [\"*\", \"have:sh\"]\n\
            command = \"print('a')\"\n\
            \n\
            [missing]\n\
            condition = [\"*\", \"have:no-such-program-fist\"]\n\
            command = \"print('b')\"\n\
            \n\
            [still_possible]\n\
            condition = { any = [\n\
                { selected = \"active\", condition = [\"*\", \"have:no-such-program-fist\"] },\n\
                { selected = \"active\" },\n\
            ] }\n\
            command = \"print('c')\"\n\
            \n\
            [always]\n\
            command = \"print('d')\"\n\
        ",
        )
        .unwrap();

        let (actions, errors) =
            MenuActions::load_all(&temp_dir.join("primary.toml"), &temp_dir, true);
        assert!(errors.is_empty(), "unexpected load errors: {errors:?}");
        assert!(actions.contains_key("installed"));
        assert!(!actions.contains_key("missing"));
        // the dead alternative is stripped, the viable one keeps the action
        assert_eq!(actions["still_possible"].condition.len(), 1);
        // an action with no conditions at all stays visible
        assert!(actions.contains_key("always"));

        // tooling can inspect the unfiltered set
        let (unfiltered, errors) =
            MenuActions::load_all(&temp_dir.join("primary.toml"), &temp_dir, false);
        assert!(errors.is_empty(), "unexpected load errors: {errors:?}");
        assert!(unfiltered.contains_key("missing"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
