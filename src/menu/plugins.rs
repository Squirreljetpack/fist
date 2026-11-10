//! Shipped plugin definitions and tests.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::{
        abspath::AbsPath,
        lessfilter::{
            Categories, LessfilterSettings,
            file_rule::{FileRule, FileRuleKind},
        },
    };

    #[test]
    fn single_condition_object_parses() {
        // the single-object condition form (one_or_many) with flat table
        let actions: MenuActions = toml::from_str(
            "\
            [my-action]
\
            condition = { condition = \"git\", strict = true }
\
            command = \"print('x')\"
\
            strategy = \"ExecPaged\"
\
        ",
        )
        .unwrap();
        assert!(matches!(
            &actions["my-action"].condition[0],
            MenuCondition::Repeat(RepeatCondition {
                selected: SelectedCondition::Cursor,
                condition,
                strict: true
            }) if matches!(condition.rules(), [rule]
                if !rule.invert && matches!(rule.kind, FileRuleKind::Git))
        ));
        assert_eq!(actions["my-action"].strategy, MenuStrategy::ExecPaged);
    }

    #[test]
    fn test_untagged_condition_formats() {
        // 1. Flat repeat object: { selected = "active", condition = "*" }
        let actions: MenuActions = toml::from_str(
            "\
            [a1]\n\
            condition = { selected = \"active\", condition = \"*\" }\n\
            command = \"print('x')\"\n\
            ",
        )
        .unwrap();
        assert!(matches!(
            &actions["a1"].condition[0],
            MenuCondition::Repeat(RepeatCondition {
                selected: SelectedCondition::Active,
                ..
            })
        ));

        // 2. Direct array of rules: condition = ["type:f", "type:d"]
        let actions: MenuActions = toml::from_str(
            "\
            [a2]\n\
            condition = [\"type:f\", \"type:d\"]\n\
            command = \"print('x')\"\n\
            ",
        )
        .unwrap();
        assert!(matches!(
            &actions["a2"].condition[0],
            MenuCondition::Seq(rules) if rules.len() == 2
        ));

        // 3. Unknown field in flat table is rejected (deny_unknown_fields)
        assert!(
            toml::from_str::<MenuActions>(
                "\
            [a3]\n\
            condition = { invalid_key = true, condition = \"*\" }\n\
            command = \"print('x')\"\n\
            "
            )
            .is_err()
        );
    }

    #[test]
    fn test_seq_condition_evaluation() {
        let toml_str = include_str!("../../assets/config/actions.dev.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        let stash_action = &actions["stash: 2 items"];

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest = AbsPath::new(root.join("Cargo.toml"));
        let src_dir = AbsPath::new(root.join("src"));

        let settings = LessfilterSettings::default();
        let categories = Categories::default();

        let mut cache = cba::vecmap::VecMap::new();

        // Selected [Cargo.toml, src] (file, dir)
        let selected = vec![manifest.clone(), src_dir.clone()];
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
        let actions: MenuActions =
            toml::from_str(include_str!("../../assets/actions/compress.toml")).unwrap();
        // every action needs the program it shells out to
        for (key, action) in actions.iter() {
            let [MenuCondition::Repeat(RepeatCondition { condition, .. })] =
                action.condition.as_slice()
            else {
                panic!("{key}: expected a Repeat condition");
            };
            let [rule] = condition.rules() else {
                panic!("{key}: expected a single rule");
            };
            let FileRuleKind::Have(program) = &rule.kind else {
                panic!("{key}: expected a have: rule");
            };
            assert!(!program.is_empty(), "{key}: the gate needs a program");
            assert_eq!(action.strategy, MenuStrategy::Exec);
        }
    }

    #[test]
    fn shipped_diff_plugin_parses() {
        let toml_str = include_str!("../../assets/actions/diff.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        assert_eq!(actions["diff"].strategy, MenuStrategy::ExecPaged);
        assert_eq!(actions["compare"].strategy, MenuStrategy::ExecPaged);
        assert!(actions["diff"].alias.is_none());
        assert!(actions["compare"].alias.is_none());
    }

    #[test]
    fn shipped_conversion_plugin_parses() {
        let toml_str = include_str!("../../assets/actions/conversion.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        for (_key, action) in actions.iter() {
            crate::lua::check_compiles(&action.command).unwrap();
        }
    }

    #[test]
    fn shipped_organize_plugin_parses() {
        let toml_str = include_str!("../../assets/actions/organize.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        assert_eq!(actions["flatten"].strategy, MenuStrategy::ExecSilent);
        assert_eq!(actions["bulk rename"].strategy, MenuStrategy::Exec);
        assert_eq!(actions["dedupe"].strategy, MenuStrategy::Exec);
        assert!(!actions["dedupe"].help.is_empty());
        for (_key, action) in actions.iter() {
            crate::lua::check_compiles(&action.command).unwrap();
        }
    }

    #[test]
    fn shipped_share_plugin_parses() {
        let toml_str = include_str!("../../assets/actions/share.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        assert_eq!(actions["drag"].strategy, MenuStrategy::ExecSilent);
        for (_key, action) in actions.iter() {
            crate::lua::check_compiles(&action.command).unwrap();
        }
    }

    #[test]
    fn shipped_docs_plugin_parses() {
        let toml_str = include_str!("../../assets/actions/docs.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        for (_key, action) in actions.iter() {
            crate::lua::check_compiles(&action.command).unwrap();
        }
    }

    #[test]
    fn shipped_test_plugin_parses_and_executes() {
        use crate::run::state::GLOBAL;

        GLOBAL::init_test_senders();
        let toml_str = include_str!("../../assets/actions/test.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        let action = actions.get("test").expect("test action should be present");
        assert_eq!(action.strategy, MenuStrategy::ExecWait);
        crate::lua::check_compiles(&action.command).expect("test action command should compile");

        let p = AbsPath::new(std::path::PathBuf::from("/tmp/test_file"));
        let res = crate::lua::execute(&action.command, &[p], "dest_dir", None, None, true);
        assert!(res.is_ok(), "test action execution failed: {res:?}");

        let test_input = actions
            .get("test_input")
            .expect("test_input action should be present");
        assert_eq!(test_input.strategy, MenuStrategy::ExecWait);
        crate::lua::check_compiles(&test_input.command)
            .expect("test_input action command should compile");
    }

    #[test]
    fn rule_sets_require_every_rule() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::File::create(&file).unwrap();
        let a = AbsPath::new(file);

        let settings = LessfilterSettings::default();
        let categories = Categories::default();
        let cond = |condition: &str| {
            let actions: MenuActions = toml::from_str(&format!(
                "\
                [a]\n\
                condition = {condition}\n\
                command = \"print(1)\"\n\
            "
            ))
            .unwrap();
            actions["a"].condition.clone()
        };
        let eval = |conditions: &[MenuCondition], selected: &[AbsPath]| {
            condition_passes(
                conditions,
                selected,
                Some(&a),
                false,
                None,
                None,
                &mut cba::vecmap::VecMap::new(),
                &settings,
                &categories,
            )
        };

        // a set passes iff every rule in it passes
        assert!(eval(
            &cond("{ selected = \"active\", condition = [\"type:f\", \"have:sh\"] }"),
            std::slice::from_ref(&a)
        ));
        assert!(!eval(
            &cond(
                "{ selected = \"active\", condition = [\"type:f\", \"have:no-such-program-fist\"] }"
            ),
            std::slice::from_ref(&a)
        ));
        assert!(!eval(
            &cond("{ selected = \"active\", condition = [\"type:d\", \"have:sh\"] }"),
            std::slice::from_ref(&a)
        ));

        // alternatives stay OR'd, only repeat tables can be alternatives
        assert!(eval(
            &cond(
                "{ any = [{ selected = \"active\", condition = [\"type:f\", \"have:no-such-program-fist\"] }, { selected = \"active\", condition = \"type:f\" }] }"
            ),
            std::slice::from_ref(&a)
        ));

        // positional sets apply to their own selection only
        let two = [a.clone(), a.clone()];
        assert!(eval(&cond("[[\"type:f\", \"have:sh\"], \"type:f\"]"), &two));
        assert!(!eval(
            &cond("[[\"type:f\", \"have:sh\"], \"type:d\"]"),
            &two
        ));
        // one position per selected item
        assert!(!eval(
            &cond("[\"type:f\", \"type:f\"]"),
            std::slice::from_ref(&a)
        ));
    }

    #[test]
    fn enqueue_strategy_parses_without_destination() {
        let actions: MenuActions = toml::from_str(
            "\
            [a]\n\
            strategy = \"Enqueue\"\n\
            condition = { condition = \"*\" }\n\
            command = \"print(1)\"\n\
        ",
        )
        .unwrap();
        assert_eq!(actions["a"].strategy, MenuStrategy::Enqueue);
        assert!(actions["a"].closes());

        // an enqueued row starts immediately, so there is no destination to wait for
        let err = toml::from_str::<MenuActions>(
            "\
            [a]\n\
            strategy = \"Enqueue\"\n\
            requires_dest = true\n\
            condition = { condition = \"*\" }\n\
            command = \"print(1)\"\n\
        ",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("Enqueue"), "{err}");
    }

    #[test]
    fn selected_condition_spellings_parse() {
        for (spelling, expected) in [
            ("selected = \"cursor\"", SelectedCondition::Cursor),
            ("selected = \"cwd\"", SelectedCondition::Cwd),
            ("selected = \"active\"", SelectedCondition::Active),
            ("selected = \"single\"", SelectedCondition::Active),
            ("selected = 2", SelectedCondition::Selections(2)),
        ] {
            let actions: MenuActions = toml::from_str(&format!(
                "\
                [my-action]\n\
                condition = {{ {spelling}, condition = \"*\" }}\n\
                command = \"print('x')\"\n\
            "
            ))
            .unwrap_or_else(|e| panic!("{spelling}: {e}"));
            let MenuCondition::Repeat(RepeatCondition {
                selected: parsed, ..
            }) = &actions["my-action"].condition[0]
            else {
                panic!("{spelling}: expected a Repeat condition");
            };
            assert_eq!(parsed, &expected, "{spelling}");
        }

        // omitting `selected` defaults to Cursor
        let actions: MenuActions = toml::from_str(
            "\
            [my-action]\n\
            condition = { condition = \"*\" }\n\
            command = \"print('x')\"\n\
        ",
        )
        .unwrap();
        assert!(matches!(
            &actions["my-action"].condition[0],
            MenuCondition::Repeat(RepeatCondition {
                selected: SelectedCondition::Cursor,
                ..
            })
        ));

        // the rule itself defaults to `*`
        let actions: MenuActions = toml::from_str(
            "\
            [my-action]\n\
            condition = { selected = 2 }\n\
            command = \"print('x')\"\n\
        ",
        )
        .unwrap();
        let MenuCondition::Repeat(RepeatCondition { condition, .. }) =
            &actions["my-action"].condition[0]
        else {
            panic!("expected a Repeat condition");
        };
        assert!(matches!(condition.rules(), [rule] if matches!(rule.kind, FileRuleKind::Any)));

        // the old `count` spelling is rejected (deny_unknown_fields)
        assert!(
            toml::from_str::<MenuActions>(
                "\
            [my-action]\n\
            condition = { count = 0, condition = \"*\" }\n\
            command = \"print('x')\"\n\
        ",
            )
            .is_err()
        );
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
            vec![MenuCondition::Repeat(RepeatCondition {
                selected,
                condition: RuleSet::from("*".parse::<FileRule>().unwrap()),
                strict,
            })]
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
                &mut cba::vecmap::VecMap::new(),
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
            std::slice::from_ref(&d),
            Some(&a),
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Cursor, false),
            std::slice::from_ref(&d),
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
            std::slice::from_ref(&a),
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
            std::slice::from_ref(&a),
            None,
            false,
            None,
            None
        ));

        // Active: selections → cursor → cwd chain; strict needs exactly one
        // resolved target.
        assert!(eval(
            &cond(SelectedCondition::Active, false),
            &two,
            None,
            false,
            None,
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Active, true),
            &two,
            None,
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Active, true),
            std::slice::from_ref(&a),
            None,
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Active, false),
            &[],
            Some(&a),
            false,
            None,
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Active, false),
            &[],
            None,
            true,
            Some(&d),
            None
        ));
        assert!(eval(
            &cond(SelectedCondition::Active, true),
            &[],
            None,
            true,
            Some(&d),
            None
        ));
        assert!(!eval(
            &cond(SelectedCondition::Active, false),
            &[],
            None,
            true,
            None,
            None
        ));

        // the rule must match every target in the resolved set
        let file_rule: Vec<MenuCondition> = vec![MenuCondition::Repeat(RepeatCondition {
            selected: SelectedCondition::Active,
            condition: RuleSet::from("type:f".parse::<FileRule>().unwrap()),
            strict: false,
        })];
        assert!(eval(
            &file_rule,
            std::slice::from_ref(&a),
            None,
            false,
            None,
            None
        ));
        assert!(!eval(&file_rule, &two, None, false, None, None));
    }
}
