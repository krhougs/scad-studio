# Web Preview Zoom Depth Glitch Investigation Result

## 进度

- 已提交上一轮点光源强度改动：`cde55a8 feat: persist preview point light intensity`。
- 已确认根因方向：`clippingPlanesForBounds` 在相机缩放到模型附近但仍在模型外时，会把 near 退到 `0.01`，同时 far 至少 `1000`，导致 WebGL 深度范围比例过大，容易在斜面和边缘出现动态深度纹路。
- 已按 TDD 增加单元测试：`keeps clipping planes tight while dollying near the mesh`。该测试在旧实现下失败，失败点为 `near = 0.01`。
- 已修复 clipping plane 计算：根据模型半径和相机到 bounds center 的距离收紧 near/far；相机进入或贴近模型时保留较小 near，避免近距离裁切。
- 已通过验证：
  - `bun --cwd packages/studio-web test:unit -- tests/unit/mesh-render-metrics.test.ts` 通过，10 个测试通过。
  - `bun --cwd packages/studio-web typecheck` 通过。
  - `bun --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts` 通过，15 个测试通过。
- 独立 review 已启动，等待结果。
- 第一轮独立 review 发现两个必须处理的风险：
  - 小模型的 build plate/grid helper 可能被过小的 far 裁切。
  - pan 后 near 仍按 bounds center 欧氏距离收紧，可能裁掉画面边缘的 mesh。
- 已补充两个失败测试覆盖上述风险，并修复为按 camera forward 投影计算深度范围：
  - mesh 最近投影深度决定 near。
  - mesh 与 build plate 的最远投影深度共同决定 far。
  - 贴近或进入 mesh 时 near 保守降低。
- 修复后再次验证通过：
  - `bun --cwd packages/studio-web test:unit -- tests/unit/mesh-render-metrics.test.ts` 通过，12 个测试通过。
  - `bun --cwd packages/studio-web typecheck` 通过。
  - `bun --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts` 通过，15 个测试通过。
- 第二轮独立 review 已启动，等待结果。
- 第二轮独立 review 未发现必须修复的问题。
- 非阻断观察：axes helper 当前只缩放、不跟随 mesh center/bottom；如果模型坐标远离世界原点，axes helper 理论上可能被当前 clipping 范围裁切。该问题不影响 mesh 与 build plate，也不是本轮斜面/边缘动态纹路的修复目标，本轮不扩大范围处理。
