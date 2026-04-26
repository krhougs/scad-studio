# Web Preview Point Light Intensity Result

## 进度

- Phase 1 已完成：`PreviewAppearance` 新增 `pointLightIntensity`，默认值为 `1.6`；读取旧配置时补默认值，非法数值按 `0..5` 归一化；`preset-io` 单元测试已覆盖默认值、读取、写入和非法值。
- Phase 2 已完成：Appearance 右侧栏新增点光源强度数值控件；Three.js 运行时点光源强度改为 `lightingIntensity * pointLightIntensity`；canvas dataset 暴露配置强度和运行时强度；Playwright 已覆盖持久化、切换文件恢复和运行时强度。
- Phase 3 验证已完成：
  - `bun --cwd packages/studio-web typecheck` 通过。
  - `bun --cwd packages/studio-web test:unit -- tests/unit/preset-io.test.ts tests/unit/mesh-render-metrics.test.ts` 通过，24 个测试通过。
  - `bun --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts` 通过，15 个测试通过。
- 独立 review 已启动，等待结果。
- 独立 review 返回两个 Important 问题：
  - `parameters-presets.spec.ts` 的全量持久化断言缺少 `pointLightIntensity`。
  - `preview-request-dedup.spec.ts` 未覆盖点光源强度不会触发 OpenSCAD 重新渲染。
- 两个问题均已修复。
- 补充验证：
  - `bun --cwd packages/studio-web test:e2e tests/playwright/parameters-presets.spec.ts -g "save, load, delete round-trip"` 通过。
  - `bun --cwd packages/studio-web test:e2e tests/playwright/preview-request-dedup.spec.ts -g "appearance changes do not emit preview request"` 通过。
  - `bun --cwd packages/studio-web typecheck` 通过。
  - `bun --cwd packages/studio-web test:unit -- tests/unit/preset-io.test.ts tests/unit/mesh-render-metrics.test.ts` 通过。
  - `bun --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/preview-request-dedup.spec.ts` 中 26 个测试通过，`@preview-dedup scad refresh emits one equivalent preview request` 失败。该失败已单独复现并记录到 `docs/known_issues.md`，当前判断为 Web `.scad` 外部刷新链路问题，不作为本轮点光源强度配置的验收依据。
