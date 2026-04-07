use scad_viewer::ui::toolbar;

#[test]
fn embedded_height_is_single_row_when_very_wide() {
    let h = toolbar::embedded_height(8000.0, false);
    assert!(
        (h - 28.0).abs() < 1.5,
        "极宽内宽应单行条带，期望约 28，实际 {}",
        h
    );
}

#[test]
fn embedded_height_grows_when_narrow() {
    let wide = toolbar::embedded_height(4000.0, false);
    let narrow = toolbar::embedded_height(120.0, false);
    assert!(
        narrow > wide + 8.0,
        "窄内宽应多行增高，wide={} narrow={}",
        wide,
        narrow
    );
}

#[test]
fn file_group_never_reduces_line_budget() {
    let w = 160.0;
    assert!(
        toolbar::embedded_height(w, true) + 0.5 >= toolbar::embedded_height(w, false),
        "含「打开/设置」块时不应比无文件块时更矮"
    );
}

#[test]
fn wider_available_width_never_increases_strip_height() {
    let h_small = toolbar::embedded_height(180.0, false);
    let h_large = toolbar::embedded_height(900.0, false);
    assert!(
        h_large <= h_small + 1.0,
        "更宽内宽换行应不多于更窄，small={} large={}",
        h_small,
        h_large
    );
}
