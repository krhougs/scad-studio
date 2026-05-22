use app_server_core::cadquery_agent_system_prompt;

#[test]
fn cadquery_agent_system_prompt_covers_runtime_contract() {
    let prompt = cadquery_agent_system_prompt();

    for section in [
        "Role",
        "Core Principles",
        "Modes",
        "File System Contract",
        "Component / Part / Assembly Rules",
        "Ref Handling Rules",
        "Workspace Plan Package Rules",
        "Tool Permission Rules",
        "Experiment Rules",
        "Response Rules",
    ] {
        assert!(prompt.contains(section), "missing section: {section}");
    }
    assert!(prompt.contains("`Agent`"));
    assert!(prompt.contains("`Plan`"));
    assert!(!prompt.contains("Inform / Plan / Execute"));
    assert!(!prompt.contains("confirmed_cadquery"));
    assert!(prompt.contains("source of truth"));
    assert!(prompt.contains("`.py` files are model source code"));
    assert!(prompt.contains("`.md` files are semantic design notes"));
    assert!(prompt.contains("`outputs/` contains derived artifacts only"));
    assert!(prompt.contains("@component"));
    assert!(prompt.contains("@part"));
    assert!(prompt.contains("@assembly"));
    assert!(prompt.contains("@feature"));
    assert!(prompt.contains("@selector"));
    assert!(prompt.contains("@face"));
    assert!(prompt.contains("@edge"));
    assert!(prompt.contains("@vertex"));
    assert!(prompt.contains("Raw face / edge / vertex refs are not long-term truth"));
    assert!(prompt.contains("Do not expose selector refs as MVP protocol selections"));
    assert!(prompt.contains("CadQuery execution completed with warnings"));
    assert!(prompt.contains("diagnostics.traceback"));
    assert!(prompt.contains("byte-for-byte variants"));
    assert!(prompt.contains("Static CadQuery source checks"));
    let tool_rules = prompt
        .split("## 8. Tool Permission Rules")
        .nth(1)
        .expect("tool permission section exists");
    assert!(tool_rules.contains("| `update_chat_summary` semantic tool | Not allowed | Allowed |"));
    assert!(tool_rules.contains("| Static CadQuery source checks | Allowed | Allowed |"));
    assert!(tool_rules.contains("| `cadquery_dry_run` | Not allowed | Allowed through staging |"));
    assert!(tool_rules.contains(
        "| `cadquery_execute` | Not allowed | Allowed through staging and execution scope |"
    ));
    assert!(!prompt.contains("keyword matching"));
}

#[test]
fn cadquery_agent_system_prompt_uses_domain_neutral_feature_examples() {
    let prompt = cadquery_agent_system_prompt();

    for forbidden in [
        "AirPods",
        "airpods",
        "charging_pad",
        "airpods_recess",
        "wireless charging",
        "outer_shell",
        "mounting_boss",
        "alignment_slot",
        "cable_relief_channel",
    ] {
        assert!(
            !prompt.contains(forbidden),
            "system prompt must not contain current acceptance scenario token: {forbidden}"
        );
    }
}

#[test]
fn cadquery_agent_system_prompt_owns_feature_key_naming() {
    let prompt = cadquery_agent_system_prompt();

    assert!(prompt.contains("REFS.features keys are your responsibility"));
    assert!(prompt.contains("tool schemas, warnings, and errors describe structure only"));
    assert!(prompt.contains("do not provide feature names to copy"));
    assert!(prompt.contains("Preserve existing stable feature keys"));
}
