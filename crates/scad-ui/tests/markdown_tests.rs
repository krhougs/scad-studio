use scad_ui::markdown::{MarkdownBlock, MarkdownDocument, MarkdownInline, MarkdownListKind};

#[test]
fn parse_markdown_extracts_basic_block_types() {
    let source = r#"# Title

Paragraph with **bold**, *italic*, and `code`.

- first item
- second item

```rust
fn main() {}
```
"#;

    let doc = MarkdownDocument::parse(source);
    assert_eq!(doc.blocks().len(), 4);

    match &doc.blocks()[0] {
        MarkdownBlock::Heading { level, content } => {
            assert_eq!(*level, 1);
            assert_eq!(content.plain_text(), "Title");
        }
        other => panic!("expected heading, got {other:?}"),
    }

    match &doc.blocks()[1] {
        MarkdownBlock::Paragraph(content) => {
            assert_eq!(
                content.plain_text(),
                "Paragraph with bold, italic, and code."
            );
            assert!(
                content
                    .iter()
                    .any(|span| matches!(span, MarkdownInline::Code(text) if text == "code"))
            );
        }
        other => panic!("expected paragraph, got {other:?}"),
    }

    match &doc.blocks()[2] {
        MarkdownBlock::List { kind, items } => {
            assert_eq!(*kind, MarkdownListKind::Unordered);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].plain_text(), "first item");
            assert_eq!(items[1].plain_text(), "second item");
        }
        other => panic!("expected list, got {other:?}"),
    }

    match &doc.blocks()[3] {
        MarkdownBlock::CodeBlock { language, content } => {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(content.contains("fn main() {}"));
        }
        other => panic!("expected code block, got {other:?}"),
    }
}
