# Plan: 四项 UI 改进

## Context

用户提出四个改进需求：
1. 相机 reset 距离写死为 3.0，应该根据模型实际大小计算使其能显示完全
2. close_btn 的 `\u{2715}` (✕) 符号渲染为方块，需修复；font_probe 应改为测试并补充符号/emoji 探测
3. 参数面板和相机面板的 UI 属性（透明度、排版）应共享代码
4. 日志面板应从固定底部面板改为浮动面板，共享同样的 UI 行为

---

## Phase 1: 共享浮动面板基础设施 + 修复 close 按钮

### 变更

- `src/ui/theme.rs` — 新增 `floating_frame(opacity)` 和 `close_button(ui, tooltip)`
- `src/ui/camera_overlay.rs` — 使用 `theme::floating_frame(opacity)` 替代手动 alpha 计算
- `src/ui/side_panel.rs` — 使用 `theme::floating_frame(1.0)` 和 `theme::close_button`，删除本地 `close_btn`
- `src/ui/log_panel.rs` — 从 `TopBottomPanel::bottom` 重写为 `egui::Area` 浮动面板，使用共享 frame/close
- `src/config.rs` — 新增 `log_panel_pos: Option<[f32; 2]>`
- `src/ui/mod.rs` — 更新 log_panel 调用传入 config

## Phase 2: 相机 reset 使用模型实际大小

### 变更

- `src/camera.rs` — `reset_view(Option<Bounds>)` 接受可选 bounds，有则用 `fit_bounds`
- `src/main.rs` — `RuntimeState` 新增 `current_bounds: Option<Bounds>`，在 `handle_render_finished` 中存储，ResetView dispatch 时传入

## Phase 3: font_probe 改写为测试 + 补充符号探测

### 变更

- `src/system_fonts.rs` — 新增 `pub has_glyph(font_data, ch)` 函数 + `probe_glyph_coverage_in_system_fonts` 测试
- `src/bin/font_probe.rs` — 删除本地 `has_glyph`，复用 `system_fonts::has_glyph`

## Phase 4: 回归测试

- `cargo test` 全部通过（86 tests）
- `cargo check` 无新增 warning
