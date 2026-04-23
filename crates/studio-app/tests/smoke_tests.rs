#[test]
fn app_menu_label_is_present() {
    assert_eq!(studio_app::platform_menu::APP_NAME, "SCAD Studio");
}

#[test]
fn desktop_smoke_roundtrip_succeeds() {
    let workspace =
        std::env::temp_dir().join(format!("studio-app-desktop-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(workspace.join("README.md"), "hello desktop smoke").unwrap();

    studio_app::run_desktop_smoke_for_test(workspace.clone()).unwrap();

    let _ = std::fs::remove_file(workspace.join("README.md"));
    let _ = std::fs::remove_dir_all(workspace);
}
