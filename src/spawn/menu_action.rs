use cba::define_collection_wrapper;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};

use crate::{
    abspath::AbsPath,
    lessfilter::{
        file_rule::{FileData, FileRule},
        rule_matcher::Test,
        Categories, LessfilterSettings,
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

/// Menu action keys reserved for the builtin queue kinds and the queue
/// selectors; defining an action under one of these (case-insensitively) or
/// under the empty key is a config error.
pub const RESERVED_KEYS: [&str; 8] = [
    "copy", "cut", "symlink", "none", "all", "builtins", "first", "last",
];

impl<'de> Deserialize<'de> for MenuActions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = IndexMap::<String, MenuAction>::deserialize(deserializer)?;
        if let Some(key) = map
            .keys()
            .find(|k| k.is_empty() || RESERVED_KEYS.iter().any(|r| k.eq_ignore_ascii_case(r)))
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
    /// `*.toml` in the actions folder, merged in sorted filename order
    /// (numeric prefixes order them). The primary file's entries come
    /// first; a key defined in a later file is an error. A missing primary
    /// file or folder yields the empty set.
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
        let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(dir) {
            Ok(rd) => rd
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "toml"))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        };
        files.sort();
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

/// A criterion evaluated against the picker state at menu open.
/// The action is visible iff at least one condition is satisfied.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum MenuCondition {
    /// Positional: exactly as many items must be selected as there are rules,
    /// and rule *i* must match the *i*-th selected item in selection order.
    Seq(Vec<FileRule>),
    /// Repetition of a single rule, scoped by [`SelectedCondition`].
    Repeat {
        /// Which state the rule is evaluated against.
        #[serde(default)]
        selected: SelectedCondition,
        condition: FileRule,
        #[serde(default)]
        strict: bool,
    },
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
    /// One target: the selected items when any are selected (any amount), the
    /// cursor item otherwise, falling back to the current directory while the
    /// cursor is disabled; fails when none of these resolve. Strict requires
    /// the resolved target set to contain exactly one path. The rule must
    /// match every path in the target set.
    Single,
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
            SelectedCondition::Single => serializer.serialize_str("single"),
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
                formatter.write_str("a non-negative integer or \"cursor\", \"cwd\", \"single\"")
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
                    "single" => Ok(SelectedCondition::Single),
                    other => Err(E::unknown_variant(other, &["cursor", "cwd", "single"])),
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
    cache: &mut Vec<(AbsPath, FileData<'a>)>,
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
        cursor_disabled: bool,
        cwd: Option<&AbsPath>,
        nav_cwd: Option<&AbsPath>,
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
                selected: which,
                condition,
                strict,
            } => match which {
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
                SelectedCondition::Single => {
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
                selected: SelectedCondition::Cursor,
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
        let toml_str = include_str!("../../assets/config/actions.dev.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        let stash_action = &actions["stash: 2 items"];

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
            None,
            &mut cache,
            &settings,
            &categories,
        );
        assert!(
            passes,
            "stash: 2 items should pass when file and dir are selected in order"
        );
    }

    #[test]
    fn shipped_compress_plugin_parses() {
        let toml_str = include_str!("../../assets/actions/compress.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        assert_eq!(actions.len(), 4);
        // every action is gated on the program it shells out to
        for (key, action) in actions.iter() {
            let MenuCondition::Repeat { condition, .. } = &action.condition[0] else {
                panic!("{key}: expected a Repeat condition");
            };
            let FileRuleKind::Have(program) = &condition.kind else {
                panic!("{key}: expected a have: rule");
            };
            assert!(!program.is_empty());
            assert_eq!(action.strategy, MenuStrategy::Execute);
        }
    }

    #[test]
    fn selected_condition_spellings_parse() {
        for (spelling, expected) in [
            ("selected = \"cursor\"", SelectedCondition::Cursor),
            ("selected = \"cwd\"", SelectedCondition::Cwd),
            ("selected = \"single\"", SelectedCondition::Single),
            ("selected = 2", SelectedCondition::Selections(2)),
        ] {
            let actions: MenuActions = toml::from_str(&format!(
                "\
                [my-action]\n\
                condition = {{ repeat = {{ {spelling}, condition = \"*\" }} }}\n\
                command = \"print('x')\"\n\
            "
            ))
            .unwrap_or_else(|e| panic!("{spelling}: {e}"));
            let MenuCondition::Repeat {
                selected: parsed, ..
            } = &actions["my-action"].condition[0]
            else {
                panic!("{spelling}: expected a Repeat condition");
            };
            assert_eq!(parsed, &expected, "{spelling}");
        }

        // omitting `selected` defaults to Cursor
        let actions: MenuActions = toml::from_str(
            "\
            [my-action]\n\
            condition = { repeat = { condition = \"*\" } }\n\
            command = \"print('x')\"\n\
        ",
        )
        .unwrap();
        assert!(matches!(
            &actions["my-action"].condition[0],
            MenuCondition::Repeat {
                selected: SelectedCondition::Cursor,
                ..
            }
        ));

        // the old `count` spelling is rejected (deny_unknown_fields; the
        // one_or_many untagged wrapper reports the generic message)
        assert!(toml::from_str::<MenuActions>(
            "\
            [my-action]\n\
            condition = { repeat = { count = 0, condition = \"*\" } }\n\
            command = \"print('x')\"\n\
        ",
        )
        .is_err());
    }

    #[test]
    fn selected_condition_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::File::create(&file).unwrap();
        let a = AbsPath::new(file.clone());
        let d = AbsPath::new(dir.path().to_path_buf());

        let settings = LessfilterSettings::default();
        let categories = Categories::default();

        let cond = |selected: SelectedCondition, strict: bool| {
            vec![MenuCondition::Repeat {
                selected,
                condition: "*".parse().unwrap(),
                strict,
            }]
        };
        let eval = |conds: &[MenuCondition],
                    selected: &[AbsPath],
                    cursor: Option<&AbsPath>,
                    cursor_disabled: bool,
                    cwd: Option<&AbsPath>,
                    nav_cwd: Option<&AbsPath>| {
            condition_passes(
                conds,
                selected,
                cursor,
                cursor_disabled,
                cwd,
                nav_cwd,
                &mut Vec::new(),
                &settings,
                &categories,
            )
        };

        // Cursor: enabled cursor required; strict additionally forbids
        // selections; non-strict ignores them.
        assert!(eval(
            &cond(SelectedCondition::Cursor, false),
            &[],
            Some(&a),
            false,
            None,
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Cursor, false),
            &[],
            Some(&a),
            true,
            None,
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Cursor, true),
            &[d.clone()],
            Some(&a),
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Cursor, false),
            &[d.clone()],
            Some(&a),
            false,
            None,
            None
        ));

        // Cwd: non-strict needs a cwd while the cursor is disabled; strict
        // needs the nav cwd regardless of the cursor.
        assert!(eval(
            &cond(SelectedCondition::Cwd, false),
            &[],
            None,
            true,
            Some(&d),
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Cwd, false),
            &[],
            None,
            false,
            Some(&d),
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Cwd, false),
            &[],
            None,
            true,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Cwd, true),
            &[],
            None,
            false,
            None,
            Some(&d)
        ));
        assert!(!eval(
            &cond(SelectedCondition::Cwd, true),
            &[],
            None,
            true,
            Some(&d),
            None
        ));

        // Selections(n): strict exactly n, non-strict at least n.
        let two = [a.clone(), d.clone()];
        let three = [a.clone(), d.clone(), d.clone()];
        assert!(eval(
            &cond(SelectedCondition::Selections(2), true),
            &two,
            None,
            false,
            None,
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Selections(2), true),
            &[a.clone()],
            None,
            false,
            None,
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Selections(2), true),
            &three,
            None,
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Selections(2), false),
            &three,
            None,
            false,
            None,
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Selections(2), false),
            &[a.clone()],
            None,
            false,
            None,
            None
        ));

        // Single: selections → cursor → cwd chain; strict needs exactly one
        // resolved target.
        assert!(eval(
            &cond(SelectedCondition::Single, false),
            &two,
            None,
            false,
            None,
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Single, true),
            &two,
            None,
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Single, true),
            &[a.clone()],
            None,
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Single, false),
            &[],
            Some(&a),
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Single, false),
            &[],
            None,
            true,
            Some(&d),
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Single, true),
            &[],
            None,
            true,
            Some(&d),
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Single, false),
            &[],
            None,
            true,
            None,
            None
        ));

        // the rule must match every target in the resolved set
        let file_rule: Vec<MenuCondition> = vec![MenuCondition::Repeat {
            selected: SelectedCondition::Single,
            condition: "type:f".parse().unwrap(),
            strict: false,
        }];
        assert!(eval(&file_rule, &[a.clone()], None, false, None, None));
        assert!(!eval(&file_rule, &two, None, false, None, None));
    }
}
