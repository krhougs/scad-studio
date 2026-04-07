use egui::TextStyle;
use egui_commonmark::CommonMarkCache;
use scad_ui::markdown::{MarkdownDocument, reading_typography_style};

#[test]
fn reading_typography_sets_article_scale_font_sizes() {
    let base = egui::Style::default();
    let s = reading_typography_style(&base);
    assert_eq!(s.text_styles[&TextStyle::Body].size, 14.0);
    assert_eq!(s.text_styles[&TextStyle::Heading].size, 26.0);
    assert_eq!(s.text_styles[&TextStyle::Monospace].size, 12.5);
    assert_eq!(s.text_styles[&TextStyle::Small].size, 12.0);
}

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
