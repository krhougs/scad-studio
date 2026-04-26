# Prompt

用户要求先提交上一轮点光源强度持久化改动，然后排查新的 web 模型预览渲染问题：

> 在放大缩小的过程中，斜着的平面或者其边缘会出现动态变化的 glitch 纹路，摄像机特别近和特别远的时候不会触发。

## 当前状态

- 上一轮功能已提交：`cde55a8 feat: persist preview point light intensity`。
- 本轮先以 root cause investigation 为目标，不直接改渲染策略。
- 重点怀疑方向包括：相机 near/far 动态变化导致的深度精度问题、build plate / grid 与 mesh 或边缘的深度竞争、shadow bias 与接收面交互、透明 / xray depth 设置。

## 排查约束

- 先复现与收集证据，再提出或实现修复。
- 若确认存在无法在本轮解决但会影响后续判断的问题，需要更新 `docs/known_issues.md`。
