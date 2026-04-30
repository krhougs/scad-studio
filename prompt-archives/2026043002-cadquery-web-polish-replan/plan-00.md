# CadQuery Web Polish Replan

## 背景

本计划用于重新组织 CadQuery Web 预览、Ref 选择、文件路由、Agent 输出体验和真实网页验收工作。当前工作树已有一批未完成改动，因此第一目标不是继续叠加功能，而是先审计现状、清理具体建模 case 对产品代码的污染、保护已完成能力，再按可验证顺序完成剩余目标。

必须保护的前序成果：

- Web Chat 能从“我想做一个放在车里的无线充电板上的给 AirPods 用的垫子”触发 Agent 使用 CadQuery 建模。
- CadQuery runner 能写入 `.py` 并导出 `.step`，失败、超时或取消时不污染真实 workspace。
- CadQuery `.py` 已能进入模型预览。
- Viewer 能选择 face / edge / vertex，并把 Ref 写入 selection 和 Chat context。
- LLM reasoning 在前端以 `Thinking` 显示最新思考内容。
- `AGENTS.md` 已明确禁止测试场景污染产品代码。

## 待确定项检查

无待确定项。本计划按用户已经明确的语义执行：

- 选择模式不是 `select / preview` 两档；模式集合必须是一个独立预览模式，加多个按 protocol RefKind 划分的选择模式。
- RefKind 选择模式覆盖 MVP 用户可见层级：component / part / assembly、instance、feature、face、edge、vertex。root 只是树根展示节点，不是用户可选 Ref。
- 预览模式保留 axis、底板、gizmo、灯光、相机和渲染设置，只隐藏选择线框、anchor、hover/selected 高亮、选择 dock/status 和选择交互。
- 具体 AirPods 垫子语义只能作为真实用户输入、prompt archive、测试 fixture、测试断言、真实验收记录或生成 workspace 模型存在；不得进入前端、后端、`app-server`、protocol、tool schema 或产品 prompt 的通用实现。

## 全局执行协议

以下规则适用于 Phase -1 到 Phase 6：

1. 每个 Phase 开始前重新读取 `plan-prompt.md`、本计划和当前 `plan-00-result.md`，确认没有新增待确定项、未解决阻塞项或与前序成果冲突的改动。
2. 每个 Phase 先做只读审计，识别当前 diff 中属于用户已有改动、上一轮有效成果、本轮半成品和需要修正的边界问题；不得覆盖或回退用户已有改动。
3. 每个 Phase 执行时必须保护本计划中列出的前序成果和该 Phase 的“前序目标保护”。
4. 每个 Phase 实现完成后，必须启动独立 reviewer 审查该 Phase。review 输入必须包含：完整 `plan-00.md`、当前 Phase 目标与验收标准、前序目标保护、本 Phase diff 或涉及文件清单、已运行验证命令和结果。
5. 若 Phase review 发现阻塞项或高风险问题，必须修复、补充验证并再次发起独立 review；只有 review 无阻塞项后才能进入下一个 Phase。
6. 每个 Phase 通过后，必须立即更新 `plan-00-result.md`，记录完成情况、变更摘要、验证证据、review 结论和遗留风险。
7. 所有 Phase 完成后，必须执行 Plan 级独立 review。该 review 覆盖完整需求矩阵、每个 Phase 验收、Phase 之间是否冲突、前序成果是否被破坏、测试覆盖、真实网页证据和结果文档准确性。若发现阻塞项，回到对应 Phase 修复并重新进行 Phase review 与 Plan 级 review。

## 完整需求覆盖清单

Phase 6 必须至少覆盖以下需求，不得只按“10 条”粗略归类：

1. 自行启动本轮可控 Web dev server，并记录命令、端口和日志位置。
2. 在真实网页中新建 Chat，不复用旧 Chat。
3. 以“我想做一个放在车里的无线充电板上的给 AirPods 用的垫子”为真实用户输入，完成 CadQuery 建模。
4. 模型能从 Web 文件列表打开并预览。
5. 预览区域能交互选择 Ref，并将 selection 用于后续修改。
6. 调通过程中遇到前端问题、LLM stream 中断、tool call 出错等问题时，自行复现、定位、修复和验证。
7. 发现前端体验不佳之处时，在本计划范围内修复并验证。
8. LLM reasoning 在前端显示 `Thinking`，并显示最新一条思考过程。
9. 右侧 Inspector 提供类似 Photoshop 图层的 Ref 层级树，并支持自由多选任意用户可见 Ref。
10. `.py` 和 `.step` 从文件列表打开都路由到已生成模型预览。
11. Agent 写完模型后 `.py` 和 `.step` 保持同步，并由 app-server/protocol/manifest 显式关系表达。
12. 每个模型包含用途、细节说明和面向人类交互的稳定命名。
13. solid / wireframe / xray 渲染和切换正常。
14. 模型更新直接刷新当前 `.py` / `.step` tab，不打开新的临时 result tab。
15. LLM 输出结束只显示轻量 logo/icon，不显示大 done card。
16. Agent tool start / running / result 默认显示为单行状态，点击后用 modal 展开详情。
17. 同一 LLM stream 只在最上面显示一次 `ASSISTANT` 来源，用户输入不受影响。
18. `cadquery-select-dock` 位于预览区域底部正中间、status bar 上方。
19. 模式集合包含一个独立预览模式和多个按 protocol RefKind 划分的选择模式。
20. 预览模式保留 axis、底板等预览辅助，只隐藏选择线框和 anchor 等选择覆盖层。
21. 清理之前 Agent 把具体建模 case 和任务相关内容耦合进前端、后端、`app-server`、protocol、tool schema 或产品 prompt 的问题。

## Phase -1 — 清理具体建模 case 与任务耦合

输入：

- 当前未提交 diff。
- 前端、后端、`app-server`、protocol、tool schema、产品 prompt 和 docs 中与 CadQuery Web / Agent 相关的改动。
- `AGENTS.md` 中“禁止测试场景污染产品代码”的长期约束。

操作步骤：

1. 只读审计当前 diff，识别用户已有改动、上一轮有效成果、本轮半成品和可修改范围。
2. 全局检索当前验收 case 和任务语义相关词，包括但不限于 AirPods、车载无线充电板、无线充电、charging pad、airpods、具体垫子模型名和本轮 prompt 专用对象名。
3. 进行语义审计：检查新增默认 prompt、schema 示例、tool fallback、前端展示分支、后端路由、测试专用常量、模型结构默认值等是否只服务本轮验收，即使命中内容不包含上述关键词也要纳入判断。
4. 将命中结果分类为：产品代码路径、通用契约或 schema、产品 prompt / guidance、测试 fixture、prompt archive、真实验收记录、生成 workspace 模型。
5. 对产品代码路径、通用契约或 schema、产品 prompt / guidance 中的具体 case 语义进行清理，改为领域无关的产品契约、能力描述或通用示例。
6. 保留测试 fixture、prompt archive、真实验收记录和生成 workspace 模型中的具体 case，但确认它们不会被产品运行路径读取为默认逻辑或通用提示。
7. 为清理后的边界补充或调整验证，确保后续新增功能不能再次依赖具体 case 文案或对象名通过测试。

验收标准：

- 前端、后端、`app-server`、protocol、tool schema 和产品 prompt 中不存在当前验收 case 专属对象名、任务 prompt 专用语义或无关键词的一次性验收逻辑。
- 保留下来的具体 case 只存在于测试 fixture、prompt archive、真实验收记录或生成 workspace 模型中。
- 清理后不破坏已完成的 CadQuery 建模、预览、selection、reasoning 和 staging 语义。
- 已按“全局执行协议”完成本 Phase 独立 review、收敛和 `plan-00-result.md` 更新。

前序目标保护：

- 不删除真实验收所需的 prompt archive、测试 fixture 或生成模型记录。
- 不把领域无关清理扩大成无关重构。

## Phase 0 — 当前状态审计与基线固定

输入：

- Phase -1 清理后的当前未提交 diff。
- `prompt-archives/2026043000-cadquery-web-e2e-gapfill/` 与 `prompt-archives/2026043001-cadquery-preview-ref-layer-polish/`。
- 当前 dev server 与被中断的测试命令状态。

操作步骤：

1. 审计当前工作树，把上一轮已完成能力、本轮半成品和需要回退的错误边界分开记录。
2. 确认后台 dev server 和测试进程状态；若存在来源不明或不可控进程，记录并停止或避开，后续 Phase 5 必须启动本轮可控 dev server。
3. 确认被中断的 Playwright 进程状态，记录已完成的基线测试结果。
4. 复核 `app-server`、protocol、tool schema、system prompt、前端产品代码和后端产品代码中是否仍存在验收场景专有命名。

验收标准：

- 能明确列出当前保留、修正和继续实现的范围。
- 无后台测试进程处于不明状态。
- 前端、后端、`app-server` 和通用 prompt / schema 不包含当前验收对象专有命名。
- 已按“全局执行协议”完成本 Phase 独立 review、收敛和 `plan-00-result.md` 更新。

前序目标保护：

- 不回退 reasoning event、CadQuery source preview、Ref selection 和 staging 执行语义。

## Phase 1 — Ref 图层树、预览模式与 RefKind 选择模式

输入：

- CadQuery scene payload、topology、feature map、当前 selection snapshot。
- 当前 Canvas / Inspector / CadQuery Viewer 交互。
- GUI 共享边界约束。

操作步骤：

1. 先判断 Ref tree、mode control、select dock、status、toolbar 等改动属于共享基础层还是 Web 壳层；可复用的基础呈现优先进入共享 UI 层，状态与行为优先由共享状态层承接，Web 壳层只保留浏览器接线和页面编排。
2. 在右侧 Inspector 提供 Ref 层级树 section，呈现 root 展示节点和 protocol 暴露的用户可见 RefKind：component / part / assembly、instance、feature、face、edge、vertex。
3. Ref tree 支持自由多选用户可见 Ref，并通过现有 selection update protocol 同步。
4. Viewer canvas 选择状态与 Ref tree 选择状态互相同步。
5. 模式控件提供一个独立 preview mode，以及按 protocol RefKind 划分的多个 selection mode；不得实现成 `select / preview` 两档。
6. 如果当前模型没有某类 Ref，该 mode 可以禁用或隐藏，但不得删除仍可用的既有选择能力。
7. 预览模式只关闭选择交互和选择覆盖层；保留 axis、底板、gizmo、灯光、相机和渲染设置。
8. `cadquery-select-dock` 固定在预览区域底部正中间、status bar 上方。

验收标准：

- Inspector 中能看到 Ref tree，并能多选任意用户可见 Ref。
- 多选后 Chat context 和 Viewer selection 与 Ref tree 一致。
- Canvas 点击选择后 Ref tree 能反映当前 selection。
- RefKind selection mode 显示选择 dock 和选择辅助；preview mode 隐藏选择线框、anchor、hover/selected 高亮和选择 UI，但 axis、底板、gizmo 等预览外观仍可见。
- 模式控件能在 preview mode 与多个 RefKind selection mode 之间切换。
- 已记录 GUI 边界判断，未在 Web 壳层复制可共享基础组件或状态机。
- 已按“全局执行协议”完成本 Phase 独立 review、收敛和 `plan-00-result.md` 更新。

前序目标保护：

- 前端只消费 protocol 暴露的 Ref / topology / feature map 数据，不自行推断 Ref。
- 不减少已有选择能力。

## Phase 2 — 文件列表路由、artifact relation 与模型更新刷新

输入：

- 文件列表打开 `.py` / `.step` 的行为。
- CadQuery Agent 执行完成事件。
- app-server 维护并通过 protocol 暴露的 CadQuery manifest / artifact relation。
- 当前 tab 与 preview refresh 状态。

操作步骤：

1. 文件列表打开 `.py` 时进入对应 CadQuery 模型预览。
2. 文件列表打开生成的 `.step` 时，只能通过 app-server/protocol/manifest 暴露的 artifact relation 找到对应 CadQuery preview target；前端不得通过路径、文件名、扩展名、instance path 或 runner 输出自行建立源文件映射或 Ref 映射。
3. 无显式 artifact relation 的普通 STEP 文件走既有普通 STEP 打开路径或明确 unsupported 状态，不强行进入 CadQuery Ref 预览。
4. Agent 成功执行 CadQuery 后，若当前 tab 是对应 `.py` 或 `.step` 模型预览，则通过 manifest / artifact relation 刷新当前 tab，不创建新的临时 result tab。
5. Agent 写模型时必须通过 CadQuery tool 同步 `.py` 和 `.step`，并由 manifest / artifact relation 表达一次成功执行对应的一组产物。

验收标准：

- 文件列表分别点击 `.py` 和带显式 artifact relation 的 `.step`，都进入同一 CadQuery 模型预览链路。
- 无显式 artifact relation 的 STEP 不产生 CadQuery Ref 或源模型推断。
- Agent 后续修改模型后，当前 `.py` / `.step` tab 直接刷新。
- 不出现新的临时 CadQuery result tab 抢占当前模型 tab。
- `.py`、`.step`、preview target、topology、feature map 和 selection Ref 的关系均来自 app-server/protocol/manifest 显式数据。
- 已按“全局执行协议”完成本 Phase 独立 review、收敛和 `plan-00-result.md` 更新。

前序目标保护：

- `.py` 模型不得被普通文件写入工具改写。
- staging 成功后再回写的原子性语义不变。
- 不为 `.step` 预览引入前端 Ref 推断路径。

## Phase 3 — Agent 模型产物契约

输入：

- CadQuery Agent system prompt。
- CadQuery tool schema、contract check、runtime warning。
- 真实 Web Chat 生成或修改的模型源码。

操作步骤：

1. 要求新建或修改的 CadQuery 模型包含用途说明、关键尺寸、使用场景、假设、交互注意事项和制造或放置约束。
2. 要求 `REFS.features` 使用稳定、可读、语义化命名。`face1`、`top`、`base` 等只能作为“含义不足的反例”，不得变成固定校验特例或通用默认命名。
3. 要求 Agent 使用 `cadquery_execute` 时声明 `.step` 导出目标，保持 `.py` 与 `.step` 同步，并让 app-server 记录 artifact relation。
4. 检查通用提示、tool schema 和 warning 示例保持领域无关，不含当前验收对象专有命名。

验收标准：

- 新生成模型源码能直接读到模型用途和关键细节。
- Ref 名称面向后续选择和修改，而不是只描述几何位置或序号。
- 通用 app-server、tool schema 和产品 prompt 不含当前验收场景专有对象名。
- 已按“全局执行协议”完成本 Phase 独立 review、收敛和 `plan-00-result.md` 更新。

前序目标保护：

- 不把模型说明要求变成会阻断现有旧模型预览的硬失败，除非执行时已有对应迁移策略。
- 不降低 CadQuery 安全导入、unsafe 调用和 execution scope 校验。

## Phase 4 — 渲染模式与聊天流 UI

输入：

- Viewer toolbar。
- Three.js CadQuery mesh 渲染状态。
- Chat message、Agent event、tool event、done event 和 reasoning event。
- GUI 共享边界约束。

操作步骤：

1. 先判断 toolbar、dock、modal、status bar、chat event row 等改动属于共享基础层还是 Web 壳层；可复用的基础呈现优先进入共享 UI 层，状态与行为优先由共享状态层承接。
2. 验证 solid / wireframe / xray 对 CadQuery mesh 生效，并能从 DOM 状态和截图或像素变化中确认。
3. done event 改为轻量 logo/icon 标识，不渲染大 card。
4. tool start / running / result 默认只显示单行状态。
5. 点击 tool 单行状态时，用 modal 展开完整细节。
6. 同一段连续 Assistant 输出只在第一条显示 `ASSISTANT` 来源。
7. 用户消息来源显示不受影响。

验收标准：

- 三种渲染模式可切换且状态与视觉结果一致。
- done event 不渲染大 card，只保留单个轻量 logo/icon 标识。
- tool 详细内容仍可查看，但默认不占据大块聊天空间。
- `Thinking` 最新思考内容继续显示。
- 已记录 GUI 边界判断，未在 Web 壳层复制可共享基础组件或状态机。
- 已按“全局执行协议”完成本 Phase 独立 review、收敛和 `plan-00-result.md` 更新。

前序目标保护：

- 不丢失 Agent event 的详细信息，只改变默认呈现方式。
- 不隐藏 reasoning 的 `Thinking`。

## Phase 5 — 真实 Web Playwright 调试循环

输入：

- 本轮可控 Web dev server。
- 真实 Web 页面。
- CadQuery runner 与 LLM 配置。
- Phase 1 到 Phase 4 已通过 review 的功能。

操作步骤：

1. 启动本轮可控 Web dev server，记录命令、端口和日志位置；不得依赖来源不明的旧 server。
2. 在真实网页中新建 Chat，不复用旧 Chat。
3. 将“我想做一个放在车里的无线充电板上的给 AirPods 用的垫子”作为真实用户输入提交。该具体 prompt 只能作为用户输入和验收记录，不得写入产品默认 prompt、schema、fixture loader 或运行时分支。
4. 通过网页与 Agent 对话完成 CadQuery 建模；过程中遇到 LLM stream 中断、tool call 出错、前端错误或交互问题时，按复现、定位根因、补验证、修复的顺序处理。
5. 在预览区域选择至少一个用户可见 Ref，并发起一次基于该 selection context 的后续 Agent 修改。
6. 验证后续修改直接刷新当前 `.py` / `.step` tab，不打开新的临时 result tab。
7. 从文件列表分别打开 `.py` 和带 manifest / artifact relation 的 `.step`。
8. 验证预览、Ref tree 多选、RefKind selection mode、preview mode、solid / wireframe / xray、Chat context、tool modal、done 标识和 Assistant 来源显示。

验收标准：

- 真实网页路径完成新建 Chat → 原始用户 prompt → CadQuery 建模 → 文件列表打开 → Ref 选择 → 基于 selection 后续修改 → 当前 tab 刷新的完整链路。
- Playwright 验收过程有可复述证据，包括命令、端口、页面行为、关键断言和必要截图或 trace。
- 具体 AirPods prompt 未进入产品代码、通用 prompt、schema 或运行时默认分支。
- 已按“全局执行协议”完成本 Phase 独立 review、收敛和 `plan-00-result.md` 更新。

前序目标保护：

- 不为了通过 E2E 而在产品代码中加入测试专用分支。
- 不绕过真实 Web Chat 与 Agent tool 链路。

## Phase 6 — 最终验证、覆盖矩阵与结果归档

输入：

- `plan-prompt.md` 中记录的原始需求、用户澄清和补充。
- 本计划的“完整需求覆盖清单”。
- 所有代码与文档改动。
- 本轮测试结果。
- 真实网页验收记录。

操作步骤：

1. 生成最终需求覆盖矩阵，逐项覆盖本计划“完整需求覆盖清单”中的全部需求。
2. 为每一项标注覆盖方式：Web 单元测试、Rust / protocol / app-server 测试、Playwright 自动化测试、真实网页操作证据，或阻塞原因与恢复条件。
3. 运行相关 Web 单元测试、类型检查和 Playwright 测试。
4. 运行相关 Rust / protocol / app-server 测试。
5. 对照覆盖矩阵逐条确认测试或真实网页证据是否覆盖需求；任何未覆盖项都是执行阻断项，不得带缺口宣称计划完成。
6. 清理本轮产生的无关缓存和临时产物。
7. 更新 `plan-00-result.md`，记录完成情况、逐项需求覆盖矩阵、验证证据、Phase review 结论、Plan 级 review 结论和遗留风险。
8. 启动 Plan 级独立 review。若 review 发现阻塞项或高风险问题，回到对应 Phase 修复，重新完成该 Phase review，再重新执行 Plan 级 review。

验收标准：

- 验证命令给出明确结果。
- 完整需求覆盖清单中的每一项都有自动化测试或真实网页验收证据；无法验证时只允许记录阻塞原因和恢复条件，不能宣称完成。
- Plan 级独立 review 覆盖所有 Phase 验收、Phase 间冲突、前序成果保护、测试覆盖、真实网页证据和结果文档准确性，且无阻塞项。
- 无无关缓存或临时产物混入交付 diff。
- `plan-00-result.md` 能让后续会话无上下文继续判断当前状态。

前序目标保护：

- 不清理用户已有改动。
- 不隐藏未解决问题；若有本轮无法解决但影响后续判断的问题，更新 `docs/known_issues.md`。
