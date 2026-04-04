use scad_data::params::{ParameterKind, ParameterValue, parse_parameters};

#[test]
fn parses_grouped_visible_and_hidden_parameters() {
    let source = r#"
/* [尺寸] */
height = 12; // [4:0.5:20]
draft = true; // or false
name = "fine"; // [draft, fine, ultra]
/* [Hidden] */
internal_seed = 7; // [1:1:9]
"#;

    let parsed = parse_parameters(source);

    assert!(parsed.warnings.is_empty());
    assert_eq!(parsed.items.len(), 4);
    assert_eq!(parsed.items[0].group.as_deref(), Some("尺寸"));
    assert!(!parsed.items[0].hidden);
    assert!(matches!(
        parsed.items[0].kind,
        ParameterKind::Number {
            min: Some(4.0),
            step: Some(0.5),
            max: Some(20.0),
        }
    ));
    assert_eq!(parsed.items[1].default_value, ParameterValue::Bool(true));
    assert!(matches!(
        parsed.items[2].kind,
        ParameterKind::Choice { ref options } if options == &["draft", "fine", "ultra"]
    ));
    assert!(parsed.items[3].hidden);
}

#[test]
fn parameter_store_preserves_overrides_on_reparse() {
    let original = parse_parameters("length = 10; // [5:1:30]\nflag = false; // or true\n");
    let reparsed = parse_parameters(
        "length = 10; // [5:1:30]\nflag = false; // or true\nname = \"A\"; // [A, B]\n",
    );
    let mut store = scad_data::params::ParameterStore::from_parsed(original);

    store.set_value("length", ParameterValue::Number(18.0)).unwrap();
    store.merge_reparsed(reparsed);

    assert_eq!(store.value("length"), Some(&ParameterValue::Number(18.0)));
    assert_eq!(store.value("flag"), Some(&ParameterValue::Bool(false)));
    assert_eq!(store.value("name"), Some(&ParameterValue::Text("A".into())));
}

#[test]
fn parameter_store_builds_cli_defines_and_restore_default() {
    let parsed = parse_parameters(
        "length = 10; // [5:1:30]\nflag = false; // or true\nname = \"A\"; // [A, B]\n",
    );
    let mut store = scad_data::params::ParameterStore::from_parsed(parsed);

    store.set_value("length", ParameterValue::Number(12.5)).unwrap();
    store.set_value("flag", ParameterValue::Bool(true)).unwrap();
    store.set_value("name", ParameterValue::Text("B".into())).unwrap();

    let defines = store.cli_defines();
    assert_eq!(
        defines,
        vec![
            "length=12.5".to_string(),
            "flag=true".to_string(),
            "name=\"B\"".to_string(),
        ]
    );

    store.restore_default("flag").unwrap();
    assert_eq!(store.value("flag"), Some(&ParameterValue::Bool(false)));
}
