//! 按固定顺序、固定宽度的子项做贪心换行：得到每一行包含的块下标区间，供工具栏等手动分行绘制。

use std::ops::Range;

pub fn line_ranges(item_widths: &[f32], max_width: f32, item_spacing_x: f32) -> Vec<Range<usize>> {
    let max_width = max_width.max(1.0);
    let mut out = Vec::new();
    if item_widths.is_empty() {
        return out;
    }
    let mut line_start = 0usize;
    let mut used = 0.0f32;
    for (i, &w) in item_widths.iter().enumerate() {
        let gap = if used > 0.0 { item_spacing_x } else { 0.0 };
        if used > 0.0 && used + gap + w > max_width {
            out.push(line_start..i);
            line_start = i;
            used = w;
        } else if used == 0.0 {
            used = w;
        } else {
            used += gap + w;
        }
    }
    out.push(line_start..item_widths.len());
    out
}

pub fn line_count(item_widths: &[f32], max_width: f32, item_spacing_x: f32) -> usize {
    let ranges = line_ranges(item_widths, max_width, item_spacing_x);
    if ranges.is_empty() { 1 } else { ranges.len() }
}
