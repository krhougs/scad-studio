# Competitive Research Report: AI-Powered CAD Generation Landscape
# 竞品调研报告：AI 驱动的 CAD 生成技术全景

---

## Executive Summary / 摘要

**English**: The AI-CAD space has reached an inflection point in 2025-2026. earthtojake/text-to-cad (3000+ stars, MIT license) validates the "agent-writes-Python-CAD-code" paradigm that budn' already implements. Key findings: (1) build123d is emerging as the preferred LLM target over CadQuery due to higher Pass@1 rates, but CadQuery retains the richest research ecosystem; (2) the biggest gap in budn' vs. text-to-cad is the lack of a standard parts catalog, web-based explorer UX, and structured repair loop; (3) existing benchmarks (BenchCAD 17,900 parts, CadEval, CADSmith) provide ready-made evaluation infrastructure; (4) the "text-to-CAD is BS" skepticism on HN is real and addressable through budn's B-rep + parametric + selection pipeline.

**中文**：AI CAD 领域在 2025-2026 年已到达转折点。earthtojake/text-to-cad（3000+ stars，MIT 许可证）验证了"Agent 编写 Python CAD 代码"的范式——这也正是 budn' 已在实现的路径。核心发现：(1) build123d 因更高的 LLM 代码生成成功率正在取代 CadQuery 成为首选目标，但 CadQuery 仍保有最丰富的研究生态；(2) budn' 相比 text-to-cad 最大的差距在于缺少标准件目录、Web 端 Explorer 体验和结构化修复循环；(3) 现有基准测试（BenchCAD 17,900 零件、CadEval、CADSmith）提供了现成的评估基础设施；(4) HN 上"text-to-CAD 是不是在画饼"的质疑确实存在，但 budn' 的 B-rep + 参数化 + 精细选择管道恰好能回应这些质疑。

---

## 1. Programmatic CAD Solutions Comparison / 编程式 CAD 方案对比

### 1.1 Viable Candidates / 可行候选方案

Only **CadQuery** and **build123d** meet all hard requirements: open source, real B-rep geometry, full STEP export, Python API suitable for LLM generation. Everything else is disqualified.

满足全部硬性需求（开源、真实 B-rep 几何、完整 STEP 导出、适合 LLM 生成的 Python API）的方案只有 **CadQuery** 和 **build123d**。其余所有方案均不达标。

| Solution / 方案 | STEP | B-rep | LLM Pass@1 | Features / 功能丰富度 | Research Ecosystem / 研究生态 |
|---|---|---|---|---|---|
| **CadQuery** | Yes | Yes (OCCT) | 0.50 | Excellent / 优秀 | Richest (BenchCAD, Text-to-CadQuery 170k pairs) / 最丰富 |
| **build123d** | Yes | Yes (OCCT) | **0.59** | Excellent / 优秀 | Growing (text-to-cad harness community) / 快速成长中 |
| OpenCascade direct | Gold standard | Yes | Very low / 极低 | Maximum / 最强 | N/A (too low-level) |
| FreeCAD Python | Yes | Yes | Low / 低 | Very rich / 非常丰富 | None / 无 |
| Manifold | **No** | No (mesh) | N/A | Limited / 有限 | N/A |
| JSCAD | **No** | No (CSG) | N/A | Basic / 基础 | N/A |
| ImplicitCAD | **No** | No (SDF) | N/A | Basic / 基础 | N/A |
| Fornjot (Rust) | **No** | Partial | N/A | Minimal / 极少 | N/A |
| Truck (Rust) | Partial | Yes | Low / 低 | Limited / 有限 | N/A |

**Disqualification reasons / 淘汰原因：**
- **Manifold, JSCAD, ImplicitCAD**: No STEP export, no B-rep — fundamentally incompatible with engineering CAD / 无 STEP 导出，无 B-rep，与工程 CAD 根本不兼容
- **FreeCAD**: Non-pure Python, 500MB+ runtime, GUI entanglement, no LLM training data / 非纯 Python，500MB+ 运行时，与 GUI 耦合严重，无 LLM 训练数据
- **Fornjot**: Author's own advice: "wait a year or ten" / 作者本人建议"等一两年甚至十年"
- **Truck**: Japanese company, limited English docs, no LLM training data / 日本公司维护，英文文档有限，无 LLM 训练数据

### 1.2 CadQuery vs. build123d: The Real Decision / 真正的选择

**English:**

Both share the same OCCT kernel via OCP bindings and are interoperable at the object level. The decision is not either/or.

**CadQuery advantages:**
- Largest AI-CAD research corpus: BenchCAD (17,900 industrial CadQuery programs, 106 families), Text-to-CadQuery (170k annotated pairs), CadQueryEval benchmark
- budn' already uses CadQuery (`budn_cad_runner`, agent system prompt, all tool schemas)
- Higher geometric accuracy (IoU 0.2827 vs build123d's 0.2617 in CADDesigner benchmark)
- Proven selector system that maps to budn's Ref model (`.faces(">Z")`, `.edges()`)

**build123d advantages:**
- Higher Pass@1 success rate (0.59 vs 0.50) — LLMs generate more executable code
- Context-manager syntax is more Pythonic, LLMs handle it more fluently
- `Select.LAST`/`Select.NEW` topology tracking — directly useful for budn's face/edge/vertex Ref model
- earthtojake/text-to-cad chose build123d, creating community momentum
- Algebraic boolean syntax (`+`, `-`, `&` operators) is more concise

**build123d risk:**
- The overloaded symbolic operators cause LLMs to produce code that *executes* but is *geometrically wrong* more often — hence the lower IoU despite higher Pass@1
- Smaller documentation corpus means models hallucinate APIs more

**中文：**

两者共享同一个 OCCT 内核（通过 OCP 绑定），对象级别可互操作。选择并非非此即彼。

**CadQuery 优势：**
- 最大的 AI-CAD 研究语料库：BenchCAD（17,900 个工业级 CadQuery 程序，106 个零件族）、Text-to-CadQuery（170k 标注对）、CadQueryEval 基准
- budn' 已经在使用 CadQuery（`budn_cad_runner`、Agent 系统提示词、所有工具模式定义）
- 更高的几何精度（IoU 0.2827 vs build123d 的 0.2617）
- 经过验证的选择器系统，直接映射到 budn' 的 Ref 模型

**build123d 优势：**
- 更高的 Pass@1 成功率（0.59 vs 0.50）——LLM 生成的代码更易执行
- 上下文管理器语法更符合 Python 习惯，LLM 处理更流畅
- `Select.LAST`/`Select.NEW` 拓扑追踪功能，对 budn' 的精细选择 Ref 模型直接有用
- earthtojake/text-to-cad 选择了 build123d，正在形成社区动量

**build123d 风险：**
- 符号运算符重载导致 LLM 生成的代码"能运行但几何结果不对"的概率更高——所以虽然 Pass@1 更高，但 IoU 反而更低
- 文档语料库更小，模型更容易产生 API 幻觉

### 1.3 Recommendation / 建议

**English:** Stay on CadQuery for now. The migration cost is low if we switch later (same kernel, interoperable objects). CadQuery gives budn' access to the richest evaluation infrastructure (BenchCAD, Text-to-CadQuery) and avoids the geometric accuracy penalty of build123d's operator overloading. Monitor build123d's ecosystem growth; if the `Select.NEW` topology tracking proves critical for Ref stability, consider a gradual migration or dual-target approach (agent generates build123d, runner accepts both).

**中文：** 当前继续使用 CadQuery。后续如有需要迁移成本很低（同内核，对象互通）。CadQuery 让 budn' 可以利用最丰富的评估基础设施（BenchCAD、Text-to-CadQuery），也能避免 build123d 运算符重载带来的几何精度损失。持续关注 build123d 生态增长；若其 `Select.NEW` 拓扑追踪功能对 Ref 稳定性至关重要，可考虑渐进迁移或双目标策略（Agent 生成 build123d 代码，Runner 同时接受两种格式）。

---

## 2. text-to-cad Analysis / text-to-cad 项目分析

### 2.1 Architecture / 架构

earthtojake/text-to-cad is a collection of **agent skills** (SKILL.md files) that extend coding agents (Claude Code, Codex, Gemini CLI). It is NOT a standalone application — it runs entirely inside the user's existing coding agent.

earthtojake/text-to-cad 是一套 **Agent 技能**（SKILL.md 文件），用于扩展编码 Agent（Claude Code、Codex、Gemini CLI）的能力。它不是独立应用——完全运行在用户现有的编码 Agent 内部。

**Pipeline / 管道:**
```
Text prompt → Agent reads SKILL.md → Agent writes build123d Python → scripts/step generates STEP
→ scripts/inspect validates geometry → CAD Explorer renders in browser → @cad handles for edits
```

**Seven bundled skills / 七个内置技能:**
1. **CAD** — Core build123d generation with STEP-first workflow, natural language specs, structured repair loop
2. **step.parts** — Standard parts catalog (12,000+ STEP files: screws, bearings, motors, connectors)
3. **CAD Explorer** — React/Three.js WebGL viewer with file browser, assembly inspection, picking
4. **URDF/SDF/SRDF** — Robotics (out of scope for budn')
5. **SendCutSend** — Fabrication service integration with DFM preflight

### 2.2 What text-to-cad Does Well / text-to-cad 的优势

**English:**

1. **STEP-first workflow**: STEP is the primary artifact, not meshes. This addresses the #1 criticism of text-to-CAD tools.
2. **Source-controlled parametric code**: Agent writes `.py` files, not opaque models. Users can diff, version, and refactor.
3. **Structured repair loop**: `repair-loop.md` classifies failures (syntax, invalid geometry, fillet failure, wrong scale) and prescribes minimal fixes. The agent iterates until validation passes.
4. **Standard parts catalog**: `step.parts` (12,000+ parts via API) lets agents compose models with real off-the-shelf components.
5. **Progressive reference loading**: Skills load reference docs only when triggered by task type, avoiding context window bloat.
6. **Natural language specs**: Explicitly converts prose to internal CAD brief — no JSON schemas exposed to users.
7. **Explorer integration**: Browser-based WebGL viewer with `@cad[...]` handles for geometry-aware re-editing.
8. **Multi-agent compatibility**: Works across Claude Code, Codex, Gemini CLI, OpenClaw via identical SKILL.md.

**中文：**

1. **STEP 优先工作流**：STEP 是主要产物，不是网格。这直接回应了对 text-to-CAD 工具的头号批评。
2. **版本可控的参数化代码**：Agent 写的是 `.py` 文件，不是不透明模型。用户可以 diff、版本管理和重构。
3. **结构化修复循环**：`repair-loop.md` 对失败进行分类（语法错误、无效几何、倒角失败、比例错误），并给出最小修复方案。Agent 会反复迭代直至验证通过。
4. **标准件目录**：`step.parts`（通过 API 提供 12,000+ 零件）让 Agent 可以用真实的现成标准件组装模型。
5. **渐进式参考文档加载**：技能仅在任务类型触发时才加载参考文档，避免上下文窗口膨胀。
6. **自然语言规格**：明确将自然语言转化为内部 CAD 简报，不向用户暴露 JSON 模式。
7. **Explorer 集成**：基于浏览器的 WebGL 查看器，配合 `@cad[...]` 句柄实现几何感知的再编辑。

### 2.3 Benchmarks / 基准测试

text-to-cad ships 10 progressively complex benchmarks:

text-to-cad 内置了 10 个难度递进的基准测试：

| # | Benchmark / 基准 | Difficulty / 难度 | Key Challenge / 核心挑战 |
|---|---|---|---|
| 1 | Rectangular calibration block / 矩形校准块 | Easy | Through-holes, chamfers / 通孔、倒角 |
| 2 | Circular flange / 圆形法兰 | Easy | Bolt patterns / 螺栓分布 |
| 3 | L-bracket / L 型支架 | Easy-Medium | Gussets / 加强筋 |
| 4 | Stepped shaft with keyway / 阶梯轴含键槽 | Medium | Multi-diameter, keyway / 多直径、键槽 |
| 5 | Open-top electronics enclosure / 开顶电子外壳 | Medium | Standoffs, blind holes, shell / 支撑柱、盲孔、壳体 |
| 6 | Aerospace clevis bracket / 航空叉式支架 | Hard | Lightening cutouts / 减重开口 |
| 7 | Radial engine cylinder / 径向发动机汽缸 | Hard | Cooling fins / 散热片 |
| 8 | Centrifugal impeller / 离心叶轮 | Hard | Curved blades / 曲面叶片 |
| 9 | Spiral staircase / 螺旋楼梯 | Expert | Helical handrail / 螺旋扶手 |
| 10 | Planetary gear stage / 行星齿轮组 | Expert | Multi-body assembly, gear teeth / 多体装配、齿轮齿形 |

Each benchmark has a natural language prompt and a deterministic test case table (dimensions, body count, feature presence, negative checks). This format is directly reusable.

每个基准都有自然语言提示词和确定性测试用例表（尺寸、实体数量、特征存在性、反向检查）。这种格式可以直接复用。

---

## 3. Gaps in budn' vs. text-to-cad / budn' 相比 text-to-cad 的差距

### 3.1 Critical Gaps / 关键差距

| Gap / 差距 | text-to-cad | budn' current / budn' 当前 | Priority / 优先级 |
|---|---|---|---|
| **Standard parts catalog** / 标准件目录 | step.parts (12,000+ parts via API) | None / 无 | **High / 高** |
| **Structured repair loop** / 结构化修复循环 | `repair-loop.md` with classified failures and minimal fix prescriptions | `cadquery_dry_run` exists, but no structured failure classification or iterative repair guidance in system prompt | **High / 高** |
| **Web-based model explorer** / Web 端模型浏览器 | CAD Explorer (React + Three.js + Vite, file browser, assembly inspection) | Existing desktop viewer only, no web explorer UX | **High / 高** |
| **Natural language spec conversion** / 自然语言规格转换 | `natural-language-specs.md` reference teaches agent to convert prose to internal CAD brief | Agent system prompt has general guidelines but no structured brief-writing patterns | **Medium / 中** |
| **Progressive reference loading** / 渐进式参考加载 | 8 reference docs loaded on-demand by trigger | All context loaded upfront in system prompt | **Medium / 中** |
| **DFM/fabrication preflight** / DFM 制造预检 | SendCutSend skill with material-specific validation | None / 无 | **Low / 低** (post-MVP) |
| **Multi-agent compatibility** / 多 Agent 兼容 | Works with Claude Code, Codex, Gemini CLI | Rust backend with own LLM abstraction (correct architectural choice) | N/A (different architecture) |

### 3.2 What budn' Already Does Better / budn' 已有的优势

| Capability / 能力 | budn' | text-to-cad |
|---|---|---|
| **Atomic staging** / 原子性暂存 | `.budn_staging` with conflict detection, never pollutes workspace | Agent writes files directly, no staging |
| **Structured tool calling** / 结构化工具调用 | 19 registered tools with mode/path permissions, execution scope validation | Agent calls Python scripts via CLI, no formal tool protocol |
| **Topology metadata** / 拓扑元数据 | Runner outputs face/edge/vertex topology, feature mapping, refs | `@cad[...]` handles exist but less structured |
| **Plan/Agent mode separation** / 计划/执行模式分离 | Formal workspace plan packages with YAML front matter, execution scope, plan-result tracking | No equivalent — agent is always in "do" mode |
| **Single-commit guard** / 单次提交保护 | AtomicBool prevents double-commit per run | No equivalent |
| **B-rep precision selection** / B-rep 精细选择 | MVP face/edge/vertex selection → Agent via Ref | Explorer has picking but simpler ref system |
| **Provider abstraction** / 供应商抽象 | Rust-native multi-provider (OpenAI Responses/Completions, Anthropic) with model discovery | Depends entirely on external coding agent |

### 3.3 Skills to Extract from text-to-cad / 从 text-to-cad 中提取的能力

**English:**

From the CAD SKILL.md and reference files, the following patterns should be adapted for budn':

1. **Failure classification taxonomy**: Repair-loop classifies errors into: syntax/import failure, invalid/missing geometry, fillet/chamfer failure, wrong scale/bounding box, boolean operation failure, assembly positioning error. budn's system prompt should include analogous classification to guide the agent's self-repair behavior.

2. **Natural language brief extraction**: The agent converts prose like "make an enclosure for my Raspberry Pi" into an internal structured brief with dimensions, features, coordinate convention, assumptions, output paths, and validation criteria — all without exposing JSON to the user. budn's system prompt has MODEL_DESCRIPTION/MODEL_DETAILS requirements but lacks explicit brief-writing guidance.

3. **"Plan before coding" workflow**: Text-to-cad explicitly requires "Define parameters, labels, source paths, expected bounding boxes, and any mating/positioning datums before editing." budn's Plan mode achieves this more formally, but the Agent mode should also enforce a brief planning step before `cadquery_execute`.

4. **Conditional render for validation**: "Use scripts/render only when visual ambiguity remains" — the agent renders images only when CLI inspection can't resolve uncertainty. budn' should adopt this pattern to avoid wasteful preview generation.

5. **Default assumptions catalog**: Wall thickness 2-3mm, fillet 1-3mm, M3/M4/M5 clearance holes 3.4/4.5/5.5mm. These engineering defaults dramatically reduce prompt ambiguity and should be encoded in budn's system prompt.

6. **Feature operation mapping**: The build123d modeling reference maps design intent to operations (sketch → extrude, revolve, sweep, loft; shell for hollowing; fillet/chamfer for edges; boolean for composition). Encoding these mappings helps the agent choose the right CadQuery operations.

**中文：**

从 CAD SKILL.md 和参考文档中，以下模式应适配到 budn'：

1. **失败分类体系**：修复循环将错误分为：语法/导入错误、无效/缺失几何、倒角失败、比例/包围盒错误、布尔运算失败、装配定位错误。budn' 的系统提示词应包含类似的分类，以引导 Agent 的自修复行为。

2. **自然语言简报提取**：Agent 将"给我的树莓派做个外壳"这样的口语转化为内部结构化简报（尺寸、特征、坐标约定、假设、输出路径、验证标准），全程不向用户暴露 JSON。budn' 有 MODEL_DESCRIPTION/MODEL_DETAILS 要求，但缺少明确的简报编写指导。

3. **"先规划再编码"工作流**：text-to-cad 明确要求"在编辑前先定义参数、标签、源文件路径、预期包围盒和配合基准"。budn' 的 Plan 模式实现得更正式，但 Agent 模式在执行 `cadquery_execute` 前也应强制一个简要规划步骤。

4. **条件渲染验证**：仅在 CLI 检查无法消除歧义时才渲染图像。budn' 应采纳此模式以避免不必要的预览生成。

5. **工程默认值目录**：壁厚 2-3mm、倒角 1-3mm、M3/M4/M5 通孔 3.4/4.5/5.5mm。这些工程默认值能大幅降低提示词歧义，应编入 budn' 的系统提示词。

6. **特征操作映射表**：将设计意图映射到操作（草图→拉伸/旋转/扫略/放样；抽壳→壳体；倒角→边缘；布尔→组合），帮助 Agent 选择正确的 CadQuery 操作。

---

## 4. Promotion Strategy for Out-of-Domain / Beginner Users / 面向域外用户和初学者的推广策略

### 4.1 Market Positioning Analysis / 市场定位分析

**English:**

The text-to-CAD space is polarized. The HN "Is text-to-CAD BS?" thread (May 2026) crystalizes the debate:

**Skeptics say:**
- Demos are cherry-picked; real parts break the tools
- Professional CAD users are faster in SolidWorks/Onshape than any text-to-CAD
- LLMs have weak spatial reasoning — text is a lossy compression of design intent
- Most tools output meshes, not real CAD

**Advocates say:**
- For CAD-illiterate software engineers, it's the only path to custom parts
- Source-controlled Python is a different kind of transparency than feature trees
- STEP-first tools (budn', text-to-cad, Zoo) address the mesh criticism

**budn's positioning opportunity:**

budn' can differentiate from both the "text-to-mesh" toys and the "black-box API" services by emphasizing:

1. **"Your engineer, not your oracle"** — budn' doesn't just generate parts, it collaborates through Plan→Agent workflow, understands your selection, and builds incrementally on your feedback
2. **B-rep precision selection** — the MVP face/edge/vertex selection is genuinely unique. No other open-source tool lets you click a face and have the agent understand the topological context
3. **Parametric transparency** — every output is a `.py` file with `MODEL_DESCRIPTION`, `MODEL_DETAILS`, and `REFS` — readable, versionable, modifiable
4. **Desktop-native + Web** — the Rust backend with protocol abstraction means budn' can run locally (zero latency, full privacy) or as a web service

**中文：**

text-to-CAD 领域高度两极化。HN 上"text-to-CAD 是不是在画饼"的讨论串（2026 年 5 月）集中体现了这场争论：

**质疑方认为：**
- 演示都是精心挑选的；真实零件会让工具崩溃
- 专业 CAD 用户在 SolidWorks/Onshape 里比任何 text-to-CAD 都快
- LLM 空间推理能力弱——文字是设计意图的有损压缩
- 大多数工具输出的是网格，不是真正的 CAD

**支持方认为：**
- 对不懂 CAD 的软件工程师来说，这是获得定制零件的唯一途径
- 版本可控的 Python 源码是一种与特征树不同但同样有效的透明度
- STEP 优先的工具（budn'、text-to-cad、Zoo）已经解决了网格批评

**budn' 的差异化机会：**

1. **"你的工程师伙伴，不是你的神谕"**——budn' 不只是生成零件，它通过计划→执行工作流协作，理解你的选择，并基于反馈逐步迭代
2. **B-rep 精细选择**——MVP 的面/边/顶点选择是真正独特的。没有其他开源工具能让你点击一个面，Agent 就理解拓扑上下文
3. **参数化透明度**——每个输出都是带有 `MODEL_DESCRIPTION`、`MODEL_DETAILS` 和 `REFS` 的 `.py` 文件——可读、可版本化、可修改
4. **桌面原生 + Web**——基于 Rust 后端和协议抽象，budn' 可以本地运行（零延迟、完全隐私）或作为 Web 服务

### 4.2 Target User Segments / 目标用户群

| Segment / 用户群 | Description / 描述 | Entry point / 切入点 |
|---|---|---|
| **Hardware-curious developers** / 对硬件感兴趣的开发者 | Software engineers who want custom enclosures/mounts for side projects / 想为业余项目做定制外壳/支架的软件工程师 | "Describe your Raspberry Pi project and get a printable enclosure" / "描述你的树莓派项目，获得可打印的外壳" |
| **Maker/3D printing hobbyists** / 创客/3D 打印爱好者 | People who use Thingiverse but want custom parametric parts / 使用 Thingiverse 但想要定制参数化零件的人 | Gridfinity bins, cable clips, phone stands with custom dimensions / 带自定义尺寸的收纳盒、线缆夹、手机架 |
| **Product designers (early stage)** / 产品设计师（早期阶段） | Designers who need quick concept models before SolidWorks / 在使用 SolidWorks 前需要快速概念模型的设计师 | "I need an enclosure for a PCB that's 85x56mm with USB-C and HDMI ports" / "我需要一个 85x56mm PCB 的外壳，带 USB-C 和 HDMI 接口" |
| **Mechanical engineers (augmented)** / 机械工程师（增强型） | Engineers who want AI help with repetitive parametric variants / 想要 AI 帮助处理重复参数变体的工程师 | "Generate the M3/M4/M5/M6 variants of this bracket" / "为这个支架生成 M3/M4/M5/M6 的变体" |

### 4.3 Promotion Channels and Messages / 推广渠道和信息

**English:**

1. **Show, don't tell**: Create a gallery of 20+ real-world parts (not abstract geometry) with before/after: text prompt → STEP file → 3D printed result. The #1 criticism is cherry-picked demos — counter by showing failures alongside successes with transparent metrics.

2. **"First enclosure in 5 minutes" tutorial**: A video/GIF showing a complete workflow from "I need a case for Arduino Nano" → Plan discussion → STEP generation → face selection → "add ventilation here" → export to STL → sliced in PrusaSlicer. This is the killer demo.

3. **Benchmark transparency**: Publish budn's scores on established benchmarks (BenchCAD subset, text-to-cad's 10 benchmarks). Being honest about failure modes builds credibility — "We pass 7/10 benchmarks; here's what we're working on for the other 3."

4. **Community templates**: Publish a library of "workspace templates" — pre-configured project structures for common tasks (electronics enclosure, 3D printer bracket, gear mechanism). Lowers the cold-start barrier.

5. **Integration with 3D printing workflow**: Partner with PrusaSlicer / OrcaSlicer for one-click export. The path from text → STEP → sliced G-code should be seamless.

**中文：**

1. **展示而非描述**：创建 20+ 个真实世界零件的展示（非抽象几何），配以全流程对比：文字提示 → STEP 文件 → 3D 打印成品。对"精心挑选演示"的批评，用同时展示成功和失败案例来回应，并附上透明的指标数据。

2. **"5 分钟做出你的第一个外壳"教程**：一个视频/GIF 展示完整工作流：从"我需要一个 Arduino Nano 的壳" → 计划讨论 → STEP 生成 → 面选择 → "在这里加通风孔" → 导出 STL → 在 PrusaSlicer 中切片。这是终极演示。

3. **基准测试透明度**：公开 budn' 在已有基准上的成绩（BenchCAD 子集、text-to-cad 的 10 个基准）。坦诚面对失败模式能建立可信度——"我们通过了 7/10 个基准；这是我们正在改进的另外 3 个。"

4. **社区模板**：发布"工作空间模板"库——针对常见任务（电子外壳、3D 打印支架、齿轮机构）的预配置项目结构。降低冷启动门槛。

5. **与 3D 打印工作流集成**：与 PrusaSlicer / OrcaSlicer 合作实现一键导出。从文字 → STEP → 切片 G-code 的路径应该无缝衔接。

---

## 5. Web App Experience Design (Explorer) / Web App 体验设计（Explorer）

### 5.1 text-to-cad Explorer Architecture / text-to-cad Explorer 架构

The CAD Explorer is a React + Three.js + Vite app with:

CAD Explorer 是一个 React + Three.js + Vite 应用：

- **Tech stack**: React 18, Three.js 0.160, Tailwind CSS 4, Radix UI, Lucide icons, Vite 7
- **Key components**: `CadWorkspace` (root), `CadRenderPane` (3D viewport), `FileExplorerSidebar` (file browser), `FloatingToolBar`, `ViewPlaneControl`, `LookSettingsPopover`, `StepAssemblyFileSheet` (assembly inspector), `StatusToast`, `DrawingToolbar`
- **Libraries**: `cadManifestStore` (file catalog), `cadRefs` (geometry handles), `perspective`/`lookSettings` (camera), `selectors/runtime` (picking), `assembly/meshData` (assembly graph), `dxf/parseDxf`, `urdf/*` (robotics, out of scope)
- **Server**: `ensure-dev.mjs` auto-discovers or starts a Vite dev server, scans workspace for CAD files, serves via HTTP

### 5.2 Proposed budn' Web Explorer Experience / budn' Web Explorer 体验设计

**English:**

budn's web explorer should be more integrated than text-to-cad's standalone viewer. Since budn' has a formal agent protocol, the explorer should be a first-class client of `app-server-protocol`, not a file-watching side tool.

**Core UX flow:**

```
1. Landing / Gallery
   ├── Browse community templates and example projects
   ├── "Start from scratch" → empty workspace
   └── "Describe your project" → onboarding chat

2. Workspace
   ├── [Left] File tree (components/, parts/, assemblies/, plans/, outputs/)
   ├── [Center] 3D Viewport (Three.js, STEP/STL/3MF rendering)
   │   ├── Face/edge/vertex picking → selection sent to Agent as Ref
   │   ├── Assembly explosion view
   │   ├── Measurement tools (distance, angle, volume)
   │   └── Section plane
   ├── [Right] Chat panel
   │   ├── Plan mode conversations
   │   ├── Agent mode with tool call visualization
   │   ├── Selection context display ("You selected @face[...] on parts/enclosure.py")
   │   └── Web search results inline
   └── [Bottom] Status bar (agent state, plan progress, export status)

3. Export
   ├── STEP download (primary)
   ├── STL/3MF for 3D printing
   ├── Share link (read-only viewer with orbit)
   └── "Open in FreeCAD/Fusion" protocol handler
```

**Key differentiators from text-to-cad Explorer:**

1. **Chat-integrated viewport**: text-to-cad Explorer is view-only; budn's explorer should let users select geometry and have the selection context flow directly into the chat as a Ref. This is budn's killer feature — no other web tool does this.

2. **Plan visualization**: Show workspace plan packages as a structured workflow, not just chat messages. Users should see the plan → execution → result lifecycle visually.

3. **Gallery / template browser**: text-to-cad has no gallery. budn' should launch with a curated gallery of 20+ real-world parts that users can fork and modify. This is the #1 onboarding hook.

4. **Progressive disclosure**: Beginners see "Describe your project" → chat. Advanced users see the full workspace with file tree, plan packages, and topology inspector. Don't overwhelm novices with the full CAD workflow.

5. **Export-first status**: Show STEP file health prominently — valid/invalid, manifold check, bounding box, feature count. Make the quality of the output visible, not hidden.

**中文：**

budn' 的 Web Explorer 应比 text-to-cad 的独立查看器更加集成。由于 budn' 有正式的 Agent 协议，Explorer 应该是 `app-server-protocol` 的一等客户端，而非文件监听式的附属工具。

**核心 UX 流程：**

```
1. 着陆页 / 展示廊
   ├── 浏览社区模板和示例项目
   ├── "从头开始" → 空工作空间
   └── "描述你的项目" → 引导对话

2. 工作空间
   ├── [左] 文件树 (components/, parts/, assemblies/, plans/, outputs/)
   ├── [中] 3D 视口 (Three.js，STEP/STL/3MF 渲染)
   │   ├── 面/边/顶点选取 → 选择作为 Ref 发送给 Agent
   │   ├── 装配爆炸图
   │   ├── 测量工具（距离、角度、体积）
   │   └── 剖切面
   ├── [右] 对话面板
   │   ├── Plan 模式对话
   │   ├── Agent 模式（工具调用可视化）
   │   ├── 选择上下文显示（"你选择了 parts/enclosure.py 上的 @face[...]"）
   │   └── Web 搜索结果内嵌显示
   └── [底] 状态栏（Agent 状态、计划进度、导出状态）

3. 导出
   ├── STEP 下载（主要）
   ├── STL/3MF 用于 3D 打印
   ├── 分享链接（只读查看器带轨道旋转）
   └── "在 FreeCAD/Fusion 中打开"协议处理
```

**与 text-to-cad Explorer 的关键差异：**

1. **对话集成视口**：text-to-cad 的 Explorer 是纯查看；budn' 应让用户选择几何体后，选择上下文作为 Ref 直接流入对话。这是 budn' 的杀手级功能。
2. **计划可视化**：将工作空间计划包展示为结构化工作流，而非仅仅是对话消息。
3. **展示廊 / 模板浏览器**：text-to-cad 没有展示廊。budn' 应以 20+ 个真实世界零件的展示廊上线。
4. **渐进式展示**：初学者看到"描述你的项目" → 对话。高级用户看到完整工作空间。不要用完整的 CAD 工作流吓到新手。
5. **导出优先状态**：醒目显示 STEP 文件健康度——有效/无效、流形检查、包围盒、特征数量。让输出质量可见。

### 5.3 Technical Implementation Notes / 技术实现备注

Based on text-to-cad Explorer's stack and budn's architecture constraints:

基于 text-to-cad Explorer 的技术栈和 budn' 的架构约束：

- **Framework**: React + Three.js (same as text-to-cad Explorer) is appropriate. budn' already has `scad-ui` (egui) for desktop and `studio-web` for web. The web explorer should live in `studio-web` and consume `app-server-protocol` via WebSocket transport.
- **STEP rendering**: Need OCCT-to-mesh conversion for browser display. Options: (a) server-side tessellation in runner (already exists — runner outputs mesh), (b) client-side via opencascade.js WASM. Server-side is simpler and consistent with budn's architecture.
- **File browser**: `app-server-protocol` already has `list_directory` and file read commands. The explorer sidebar should consume these, not scan the filesystem directly.
- **Selection pipeline**: Existing `scad-scene` + Three.js raycasting → map mesh face to topology face → resolve to Ref via `cadquery_resolve_selection` equivalent. This is the most technically challenging part and budn's core differentiator.

---

## 6. Benchmarking Scenarios / 基准测试场景

### 6.1 Established Benchmarks to Leverage / 可利用的现有基准

| Benchmark | Parts / 零件数 | Format | Relevance / 相关性 |
|---|---|---|---|
| **BenchCAD** | 17,900 CadQuery programs, 106 families | IoU, CD, Feature-F1, Code Edit accuracy | Directly usable, same CadQuery target / 直接可用，同为 CadQuery |
| **text-to-cad** | 10 progressive benchmarks | Natural language prompt + test case table | Directly reusable format / 格式直接可复用 |
| **CadEval** (Epoch AI) | Unknown count | Rendering success, CD, Hausdorff, volume | Methodology reference / 方法论参考 |
| **CadQueryEval** | 25 tasks | Geometric metrics via Docker sandbox | Directly executable / 可直接执行 |
| **CADSmith** | Multi-agent pipeline eval | Execution rate, IoU, CD, VLM judge | Architecture-aligned evaluation / 架构对齐的评估方法 |
| **Text-to-CadQuery** | 170k pairs | Exact match, CD, IR, F1, IoU | Fine-tuning data reference / 微调数据参考 |

### 6.2 budn'-Specific Benchmark Suite / budn' 专属基准测试套件

Organized by budn's three target domains, with difficulty tiers.

按 budn' 的三个目标领域组织，分难度等级。

#### Tier 1: Easy (1-3 features, ~20 lines CadQuery) / 简单

| # | Scenario / 场景 | Domain / 领域 | Key Test / 核心测试 |
|---|---|---|---|
| B01 | Solid block with through-holes and chamfers / 带通孔和倒角的实心块 | General | Dimensions, hole positions, chamfer scope |
| B02 | Circular plate with bolt pattern / 带螺栓孔分布的圆板 | Industrial | Bolt circle, hole count, symmetry |
| B03 | Rectangular tube with wall thickness / 矩形管材 | General | Shell, wall thickness, open ends |
| B04 | Cable clip for 8mm wire / 8mm 线缆夹 | Hobby 3D Print | Snap geometry, minimum wall for FDM |
| B05 | Simple phone stand (wedge shape) / 简单手机架 | Hobby 3D Print | Angle, stability, pocket for phone |

#### Tier 2: Medium (3-6 features, ~40-80 lines) / 中等

| # | Scenario / 场景 | Domain / 领域 | Key Test / 核心测试 |
|---|---|---|---|
| B06 | Open-top electronics enclosure with standoffs / 带支撑柱的开顶电子外壳 | Consumer Electronics | Shell, standoffs with blind holes, corner fillets |
| B07 | Parametric spur gear (module, tooth count) / 参数化直齿轮 | Industrial | Involute tooth profile, parametric dimensions |
| B08 | Raspberry Pi 4 case with port cutouts / 树莓派 4 外壳带接口开口 | Consumer Electronics | Multiple positioned cutouts, standoff alignment |
| B09 | L-bracket with gusset and mounting holes / 带加强筋和安装孔的 L 支架 | Industrial | Gusset geometry, hole pattern, fillet |
| B10 | Gridfinity 2x2 bin with dividers / Gridfinity 2x2 收纳盒带分隔 | Hobby 3D Print | Grid snap system, parametric divider count |
| B11 | Stepped shaft with keyway / 阶梯轴含键槽 | Industrial | Multi-diameter, keyway pocket, concentricity |

#### Tier 3: Hard (sweeps/lofts, 5-10 features, ~100-200 lines) / 困难

| # | Scenario / 场景 | Domain / 领域 | Key Test / 核心测试 |
|---|---|---|---|
| B12 | Two-part snap-fit enclosure with screw bosses / 双件卡扣外壳带螺丝柱 | Consumer Electronics | Snap-fit tolerance, mating surfaces, lid alignment |
| B13 | Ventilated enclosure with internal ribs / 带通风孔和内部加强筋的外壳 | Consumer Electronics | Rib pattern, ventilation slot array, shell consistency |
| B14 | Bearing pillow block (608 bearing) / 轴承座（608 轴承） | Industrial | Press-fit bore, mounting holes, fillets |
| B15 | Helical gear / 斜齿轮 | Industrial | Swept involute profile, helix angle |
| B16 | GoPro mount with ball joint socket / GoPro 支架带球形关节 | Hobby 3D Print | Spherical cavity, snap retention, print-friendly overhangs |
| B17 | Cable routing tray with clips / 走线槽带卡扣 | Consumer Electronics | Sweep along path, snap clips, mounting tabs |

#### Tier 4: Expert (assembly, manufacturing-aware, ~200+ lines) / 专家

| # | Scenario / 场景 | Domain / 领域 | Key Test / 核心测试 |
|---|---|---|---|
| B18 | Planetary gear stage (sun, 3 planets, ring, carrier) / 行星齿轮组 | Industrial | Multi-body, gear meshing, center distances |
| B19 | Multi-PCB stacked enclosure with connectors / 多 PCB 堆叠外壳带连接器 | Consumer Electronics | Inter-board spacing, connector alignment, EMI shield |
| B20 | Injection-moldable clip with draft angles / 注塑件卡扣带拔模角 | Industrial | Draft angle, parting line, ejector pin boss |
| B21 | 3D-printable hinge mechanism / 3D 打印铰链机构 | Hobby 3D Print | Print-in-place clearance, axis alignment, rotation range |
| B22 | USB hub enclosure (full product) / USB 集线器外壳（完整产品） | Consumer Electronics | Port cutouts from spec, LED light pipe, rubber foot recess, label pocket |

### 6.3 Evaluation Dimensions / 评估维度

| Dimension / 维度 | Metric / 指标 | Automation / 自动化程度 |
|---|---|---|
| **Execution validity** / 执行有效性 | Binary: does code run? Is output a valid Solid? | Fully automated / 全自动 |
| **Geometric correctness** / 几何正确性 | IoU, Chamfer Distance vs. reference | Automated with reference models / 有参考模型时全自动 |
| **Dimensional accuracy** / 尺寸精度 | Per-dimension error vs. spec (bounding box, feature measurements) | Automated via OCCT queries / 通过 OCCT 查询自动化 |
| **Feature completeness** / 特征完整性 | Feature recall (requested vs. present) | Semi-automated (OCCT count + VLM check) / 半自动 |
| **Manufacturing readiness** / 制造可行性 | DFM rule violation count | Semi-automated (wall thickness, overhang check) / 半自动 |
| **Code quality** / 代码质量 | Parametric variable count, AST depth, MODEL_DETAILS compliance | Automated AST analysis / 自动 AST 分析 |
| **Topology quality** / 拓扑质量 | Shape healing issues, face count, fillet/chamfer presence | Automated via OCCT shape analysis / 通过 OCCT 分析自动化 |
| **Prompt faithfulness** / 提示词忠实度 | VLM judge score (1-10) | Automated VLM evaluation / 自动 VLM 评估 |
| **Parametric differential** / 参数化差异测试 | Change one dimension → verify proportional output change | **Novel metric** — fully automated / **新指标**，全自动 |

The **parametric differential test** is the most novel contribution: modify one dimension in the prompt (e.g., "width=100mm" → "width=120mm"), re-generate, and verify the output changed proportionally. This tests true parametric behavior vs. memorized shapes. No existing benchmark includes this.

**参数化差异测试**是最具创新性的贡献：修改提示词中的一个尺寸（如"宽度=100mm"→"宽度=120mm"），重新生成，验证输出是否按比例变化。这测试的是真正的参数化行为，而非记忆形状。现有基准均未包含此测试。

---

## 7. Competitive Landscape Summary / 竞争格局总结

| Competitor / 竞品 | Approach / 方法 | Output | Pricing / 定价 | Threat to budn' / 对 budn' 的威胁 |
|---|---|---|---|---|
| **earthtojake/text-to-cad** | Agent skills for coding agents, build123d | STEP, STL, 3MF, GLB | Free (MIT) | Medium — different architecture (skill vs. product), validates our paradigm / 中等——架构不同，但验证了我们的范式 |
| **Zoo (KittyCAD)** | Proprietary ML model + geometry engine, KCL language | STEP, STL, OBJ, GLB | Free tier → $99/mo Pro | High — funded, production-ready, B-rep ML generation / 高——有资金、生产就绪、B-rep ML 生成 |
| **Adam (YC W25)** | Agent inside Onshape/Fusion feature tree | Native CAD models | Beta, $4.1M raised | Medium — different approach (augment existing CAD vs. standalone) / 中等——不同路径（增强现有 CAD vs. 独立产品） |
| **ChatToSTL** | OpenSCAD code generation, live render | STL, 3MF | Unknown | Low — mesh only, no B-rep / 低——仅网格，无 B-rep |
| **FusionMCP** | Claude controls Fusion 360 step-by-step | Native Fusion models | Free | Low — requires Fusion license / 低——需要 Fusion 许可证 |

---

## 8. Actionable Recommendations / 可执行建议

### Priority 1: Immediate (next 2-4 weeks) / 优先级 1：立即行动（2-4 周）

**English:**

1. **Add structured repair loop to agent system prompt**: Extract text-to-cad's failure classification (syntax, geometry, fillet, scale, boolean) and encode as repair guidance in `docs/cadquery-mvp/agent-system-prompt.md`. This single change likely has the highest impact on generation quality.

2. **Add engineering default values to system prompt**: Wall thickness 2-3mm, standard fillet 1-3mm, M3/M4/M5 clearance 3.4/4.5/5.5mm, coordinate conventions. Reduces prompt ambiguity significantly.

3. **Implement text-to-cad's 10 benchmarks as automated tests**: Port the benchmark prompts and test case tables into an automated evaluation harness. This gives an immediate quality baseline and regression detection.

4. **Add natural language brief extraction step**: Before `cadquery_execute`, the agent should internally convert the user's prose into a structured brief (dimensions, features, coordinate system, assumptions, validation criteria). Add this as guidance in the system prompt.

**中文：**

1. **在 Agent 系统提示词中添加结构化修复循环**：提取 text-to-cad 的失败分类体系（语法、几何、倒角、比例、布尔），编入 `docs/cadquery-mvp/agent-system-prompt.md`。这一项改动对生成质量的提升可能最大。

2. **在系统提示词中添加工程默认值**：壁厚 2-3mm、标准倒角 1-3mm、M3/M4/M5 通孔 3.4/4.5/5.5mm、坐标约定。能显著降低提示词歧义。

3. **将 text-to-cad 的 10 个基准实现为自动化测试**：将基准提示词和测试用例表移植到自动化评估框架中。这能提供即时的质量基线和回归检测。

4. **添加自然语言简报提取步骤**：在 `cadquery_execute` 之前，Agent 应在内部将用户的口语转化为结构化简报。在系统提示词中添加此指导。

### Priority 2: Near-term (1-2 months) / 优先级 2：近期（1-2 个月）

**English:**

5. **Build the 22-scenario benchmark suite (Section 6.2)**: Covering consumer electronics, industrial, and hobby 3D printing at 4 difficulty tiers. Include the novel parametric differential test.

6. **Integrate a standard parts catalog**: Either integrate with step.parts API (12,000+ parts, free, MIT) or build a minimal catalog of the most common fasteners and components. This dramatically reduces the complexity of assembly tasks.

7. **Ship the web explorer MVP**: React + Three.js viewer consuming `app-server-protocol` via WebSocket. Core features: 3D viewport with face/edge/vertex picking → Ref → chat integration. The gallery/template browser and measurement tools can come later.

8. **Progressive reference loading**: Instead of loading the entire system prompt upfront, structure agent context as progressive references loaded by task trigger (like text-to-cad's approach). This preserves context window budget.

**中文：**

5. **构建 22 场景基准测试套件（第 6.2 节）**：覆盖消费电子、工业和业余 3D 打印三个领域，4 个难度等级。包含新的参数化差异测试。

6. **集成标准件目录**：接入 step.parts API（12,000+ 零件，免费，MIT）或构建最常用紧固件和元器件的最小目录。这能大幅降低装配任务的复杂度。

7. **发布 Web Explorer MVP**：React + Three.js 查看器，通过 WebSocket 消费 `app-server-protocol`。核心功能：带面/边/顶点选取的 3D 视口 → Ref → 对话集成。展示廊和测量工具可后续添加。

8. **渐进式参考加载**：不再一次性加载完整系统提示词，而是将 Agent 上下文结构化为按任务触发的渐进引用（类似 text-to-cad 的方法）。节省上下文窗口预算。

### Priority 3: Medium-term (2-4 months) / 优先级 3：中期（2-4 个月）

**English:**

9. **Gallery and template system**: Curate 20+ real-world parts as workspace templates. Launch with a "fork and modify" UX for beginners.

10. **Evaluate build123d dual-target**: Run the benchmark suite on both CadQuery and build123d targets. If build123d shows meaningfully higher success rates with acceptable geometric accuracy, consider adding it as an alternative target.

11. **VLM judge for automated evaluation**: Use a separate VLM (Claude/GPT-4o) to score generated models against prompts on a 1-10 rubric. This catches semantic failures that geometric metrics miss.

12. **Publish benchmark results transparently**: Create a public benchmark page showing budn's scores over time, compared to baselines (frontier LLMs zero-shot, text-to-cad, Zoo). Transparency builds trust with the skeptical engineering audience.

**中文：**

9. **展示廊和模板系统**：精选 20+ 个真实世界零件作为工作空间模板。以"复制并修改"的体验面向初学者发布。

10. **评估 build123d 双目标**：在 CadQuery 和 build123d 两个目标上运行基准测试套件。如果 build123d 显示出明显更高的成功率且几何精度可接受，考虑将其作为替代目标添加。

11. **VLM 评判器用于自动化评估**：使用独立的 VLM（Claude/GPT-4o）按 1-10 量表对生成模型与提示词的匹配度评分。能捕捉几何指标遗漏的语义失败。

12. **透明公开基准测试结果**：创建公开的基准测试页面，展示 budn' 的历史评分，与基线（前沿 LLM 零样本、text-to-cad、Zoo）比较。透明度能在持怀疑态度的工程师群体中建立信任。

---

## Sources / 参考来源

- [earthtojake/text-to-cad (GitHub)](https://github.com/earthtojake/text-to-cad)
- [text-to-cad CAD Skills website](https://www.cadskills.xyz/)
- [Show HN: Open-sourcing our text-to-CAD app](https://news.ycombinator.com/item?id=45140921)
- [Ask HN: Honest question about text-to-CAD. Is it BS?](https://news.ycombinator.com/item?id=47894977)
- [Text-to-CadQuery: A New Paradigm for CAD Generation (arXiv 2505.06507)](https://arxiv.org/html/2505.06507v1)
- [Zero-to-CAD: Agentic Synthesis of Interpretable CAD Programs (arXiv 2604.24479)](https://arxiv.org/html/2604.24479)
- [BenchCAD: Comprehensive Industry-Standard Benchmark (arXiv 2605.10865)](https://arxiv.org/html/2605.10865)
- [Zoo Design Studio v1](https://zoo.dev/blog/zoo-design-studio-v1)
- [Zoo Text-to-CAD Introduction](https://zoo.dev/blog/introducing-text-to-cad)
- [Zoo ML API](https://zoo.dev/machine-learning-api)
- [CadQuery Documentation](https://cadquery.readthedocs.io/)
- [build123d Documentation](https://build123d.readthedocs.io/)
- [build123d GitHub](https://github.com/gumyr/build123d)
- [CadQueryEval (Inspect AI)](https://github.com/danwahl/cadqueryeval)
- [CadQuery awesome-cadquery](https://github.com/CadQuery/awesome-cadquery)
- [Svetlana-DAO-LLC/cad-agent (build123d + MCP)](https://github.com/Svetlana-DAO-LLC/cad-agent)
- [CADDesigner: Conceptual CAD Model Generation (ECIP benchmark)](https://arxiv.org/html/2508.01031)
- [EvoCAD: Evolutionary CAD Code Generation](https://arxiv.org/pdf/2510.11631)
- [CadEval Benchmark (Epoch AI)](https://epoch.ai/benchmarks/cad-eval)
- [step.parts catalog](https://www.step.parts/)
