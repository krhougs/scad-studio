use egui_commonmark::CommonMarkCache;
use scad_ui::markdown::MarkdownDocument;

#[test]
fn parse_keeps_markdown_source_intact() {
    let source = "# Title\n\n| a | b |\n| - | - |\n| 1 | 2 |\n";
    let doc = MarkdownDocument::parse(source);
    assert_eq!(doc.source(), source);
}

#[test]
fn show_renders_commonmark_features_without_panicking() {
    let source = r#"# Title

| Feature | Status |
| --- | --- |
| Table | yes |

```rust
fn main() {}
```
"#;
    let doc = MarkdownDocument::parse(source);

    egui::__run_test_ui(|ui| {
        let mut cache = CommonMarkCache::default();
        doc.show(ui, &mut cache);
    });
}
