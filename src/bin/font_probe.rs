#[path = "../system_fonts.rs"]
mod system_fonts;

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, TextStyle};
use ttf_parser::Face;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let style = egui::Style::default();
    let button_font = style
        .text_styles
        .get(&TextStyle::Button)
        .cloned()
        .expect("button font should exist");
    let body_font = style
        .text_styles
        .get(&TextStyle::Body)
        .cloned()
        .expect("body font should exist");

    println!("当前语言优先级: {:?}", system_fonts::current_language_preferences());
    println!(
        "TextStyle::Button -> {:?} {:.1}",
        button_font.family, button_font.size
    );
    println!(
        "TextStyle::Body -> {:?} {:.1}",
        body_font.family, body_font.size
    );

    let (font_definitions, fallback_fonts) = system_fonts::build_font_definitions_for_current_ui()?;
    println!("已注入系统 fallback 字体面数量: {}", fallback_fonts.len());
    for (index, font_spec) in fallback_fonts.iter().take(12).enumerate() {
        println!(
            "  [{}] {}#{}",
            index,
            font_spec.path.display(),
            font_spec.index
        );
    }

    print_family_summary(&font_definitions, &FontFamily::Proportional, "Proportional");
    print_family_summary(&font_definitions, &FontFamily::Monospace, "Monospace");

    let probe_text = "文件打开预览等待检测渲染完成错误中文按钮状态";
    println!("\n字符命中情况:");
    for ch in probe_text.chars() {
        if let Some(hit) = first_matching_font(&font_definitions, &FontFamily::Proportional, ch) {
            println!(
                "  '{}' -> {} (family order #{})",
                ch, hit.font_name, hit.order_index
            );
        } else {
            println!("  '{}' -> 未命中任何 Proportional family 字体", ch);
        }
    }

    Ok(())
}

fn print_family_summary(fonts: &FontDefinitions, family: &FontFamily, label: &str) {
    let names = fonts
        .families
        .get(family)
        .cloned()
        .unwrap_or_default();
    println!("\n{label} family 字体顺序前 16 项:");
    for (index, name) in names.iter().take(16).enumerate() {
        let suffix = if is_system_fallback_font(name) {
            system_font_suffix(fonts.font_data.get(name))
        } else {
            String::new()
        };
        println!("  [{}] {}{}", index, name, suffix);
    }
}

fn is_system_fallback_font(name: &str) -> bool {
    name.starts_with("system-fallback-")
}

fn system_font_suffix(font_data: Option<&Arc<FontData>>) -> String {
    let Some(font_data) = font_data else {
        return String::new();
    };
    format!(" (face #{})", font_data.index)
}

fn first_matching_font(
    fonts: &FontDefinitions,
    family: &FontFamily,
    ch: char,
) -> Option<FontHit> {
    let names = fonts.families.get(family)?;
    for (order_index, font_name) in names.iter().enumerate() {
        let font_data = fonts.font_data.get(font_name)?;
        if has_glyph(font_data, ch) {
            return Some(FontHit {
                font_name: font_name.clone(),
                order_index,
            });
        }
    }
    None
}

fn has_glyph(font_data: &FontData, ch: char) -> bool {
    Face::parse(font_data.font.as_ref(), font_data.index)
        .ok()
        .and_then(|face| face.glyph_index(ch))
        .is_some()
}

struct FontHit {
    font_name: String,
    order_index: usize,
}
