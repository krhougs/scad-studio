use app_server_core::{
    PlanTimestamp, SaveCadPlanPackageInput, save_plan_package_with_timestamp, slugify_plan_title,
};
use app_server_protocol::CadQueryObjectKind;

#[test]
fn slugify_plan_title_uses_ascii_lowercase_digits_and_hyphens() {
    assert_eq!(slugify_plan_title("Add 3 Lid Vents!"), "add-3-lid-vents");
    assert_eq!(slugify_plan_title("新增滑盖"), "cad-plan");
}

#[test]
fn save_plan_package_with_timestamp_uses_existing_daily_max_sequence() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans/2026042900-old")).unwrap();
    std::fs::create_dir_all(dir.path().join("plans/2026042903-other")).unwrap();

    let saved = save_plan_package_with_timestamp(
        dir.path(),
        &sample_input(),
        PlanTimestamp {
            date_prefix: "20260429".into(),
            created_at: "2026-04-29T14:00:00+0800".into(),
        },
    )
    .expect("package should be saved");

    assert_eq!(saved.paths.plan_id, "2026042904-add-lid-vents");
    assert!(
        dir.path()
            .join("plans/2026042904-add-lid-vents/plan.md")
            .is_file()
    );
}

fn sample_input() -> SaveCadPlanPackageInput {
    SaveCadPlanPackageInput {
        title: "Add lid vents".into(),
        request: "Add vents.".into(),
        target_ref: "@part[top_lid]".into(),
        target_path: "parts/top_lid.py".into(),
        target_type: CadQueryObjectKind::Part,
        affected_files: vec!["parts/top_lid.py".into()],
        new_files: Vec::new(),
        export_targets: vec!["outputs/top_lid.step".into()],
        strategy: "Cut vents.".into(),
        risks: Vec::new(),
        acceptance: vec!["STEP export builds".into()],
        execution_scope: "Only Agent mode writes the model.".into(),
        source_chat_session: None,
    }
}
