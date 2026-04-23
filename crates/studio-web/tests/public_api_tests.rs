#[test]
fn web_shell_label_is_present() {
    assert_eq!(studio_web::web_shell_label(), "studio-web wasm shell");
}
