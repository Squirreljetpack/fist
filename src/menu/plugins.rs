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
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            &actions["my-action"].condition[0],
            MenuCondition::Repeat(RepeatCondition {
                selected: SelectedCondition::Cursor,
                condition: FileRule {
                    invert: false,
                    kind: FileRuleKind::Git
                },
                strict: true
            })
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
        let toml_str = include_str!("../../assets/actions/compress.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        assert_eq!(actions.len(), 4);
        // every action is gated on the program it shells out to
        for (key, action) in actions.iter() {
            let MenuCondition::Repeat(RepeatCondition { condition, .. }) = &action.condition[0]
            else {
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
    fn shipped_diff_plugin_parses() {
        let toml_str = include_str!("../../assets/actions/diff.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions["diff"].strategy, MenuStrategy::ExecPaged);
        assert_eq!(actions["compare"].strategy, MenuStrategy::ExecPaged);
        assert_eq!(actions["compare"].condition.len(), 4);
        assert!(actions["diff"].alias.is_none());
        assert!(actions["compare"].alias.is_none());
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
                condition: "*".parse().unwrap(),
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
            &[a.clone()],
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
            condition: "type:f".parse().unwrap(),
            strict: false,
        })];
        assert!(eval(&file_rule, &[a.clone()], None, false, None, None));
        assert!(!eval(&file_rule, &two, None, false, None, None));
    }
}
