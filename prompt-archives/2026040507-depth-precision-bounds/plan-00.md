## Phase 1: 建立裁剪面收紧测试

### 输入

- 当前相机固定使用极大的 near/far。

### 要保护的前序目标 / 边界

- 不改动前几轮透明/不透明分离与排序逻辑。

### 操作步骤

1. 在 `tests/camera_tests.rs` 新增测试。
2. 为给定 bounds 调用新 API，要求 near 明显大于 `0.01`，far 明显小于 `10000`。
3. 先运行测试，确认在现状下失败。

### 验收标准

- 新测试先失败。

## Phase 2: 实现基于 bounds 的裁剪面计算

### 输入

- Phase 1 的失败测试。

### 要保护的前序目标 / 边界

- 不改变当前相机位置与构图逻辑，只收紧深度范围。
- 渲染和交互必须使用同一套 view_proj。

### 操作步骤

1. 在 `src/camera.rs` 增加基于 bounds 的 near/far 计算和 `matrices_for_bounds`。
2. 在 `src/renderer.rs` 使用 `scene_bounds()` 生成收紧后的投影矩阵。
3. 在 `src/main.rs` 的 UI/截面交互路径中复用 `current_bounds`，保持与渲染一致。

### 验收标准

- Phase 1 测试转绿。
- `camera.matrices_for_bounds(Some(bounds))` 的裁剪面明显收紧。

## Phase 3: 回归验证

### 输入

- 已完成实现。

### 要保护的前序目标 / 边界

- 不破坏现有 `camera`、`mesh`、`pipeline`、`three_mf` 测试。

### 操作步骤

1. 运行格式化。
2. 运行相关测试。
3. 记录结果。

### 验收标准

- 相关测试通过。
