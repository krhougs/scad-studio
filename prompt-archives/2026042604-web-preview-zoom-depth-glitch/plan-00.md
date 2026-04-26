# Web Preview Zoom Depth Glitch Investigation Plan

## 背景

用户反馈 web 模型预览在缩放过程中，斜着的平面或边缘会出现动态变化的 glitch 纹路；摄像机特别近和特别远时不触发。该现象通常需要优先区分是深度精度、几何重叠、shadow acne、透明材质排序，还是测试模型本身有共面面片。

## Phase 1: 复现路径与现有渲染状态确认

### 输入

- 当前 Three.js viewer 实现。
- 已有 Playwright canvas interaction 测试。
- 用户描述的触发条件：缩放过程中、中间距离、斜面或边缘。

### 操作步骤

1. 检查 `mesh-three.ts` 中相机 near/far 更新、材质 depth 设置、build plate、grid、shadow 设置。
2. 检查相关测试模型和现有 Playwright 是否能驱动 wheel zoom。
3. 判断是否已有 canvas dataset 可暴露深度精度相关状态；缺失时只先记录缺口，不直接改生产代码。

### 验收标准

- 能给出至少一个可验证的根因假设。
- 明确哪些代码路径与该现象相关，哪些路径可以先排除。

## Phase 2: 最小证据收集

### 输入

- Phase 1 的根因假设。

### 操作步骤

1. 通过代码和测试观察相机 distance、near、far 的关系。
2. 对照 Three.js / 本项目已有模式确认深度精度风险。
3. 如需要，新增只读 dataset 或测试辅助断言，但不先改变视觉结果。

### 验收标准

- 能解释为什么问题在中间缩放距离触发，而特别近和特别远不触发。
- 能说明是否需要修改 near/far、helper 深度关系、shadow 或材质设置。

## Phase 3: 修复方案判断

### 输入

- Phase 2 的证据。

### 操作步骤

1. 若根因明确且修复范围小，按 TDD 增加回归断言后实现。
2. 若根因仍不充分，停止在调查结论，不做猜测性修复。
3. 更新执行结果。

### 验收标准

- 不引入与点光源、per file 持久化或 preview request 去重无关的改动。
- 若实施修复，相关 typecheck / unit / Playwright 验证通过。
