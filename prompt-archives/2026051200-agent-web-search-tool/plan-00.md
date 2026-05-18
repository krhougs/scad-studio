# Agent Web Search Function Tool

## Context

产品 Agent 的 web search 依赖 provider-hosted 能力（Anthropic `server_tool_use`、OpenAI Responses `web_search`），但实际使用的代理 provider（DeepSeek、MiMo）均不支持 server-side 执行。需要实现自有的 web search function tool，通过外部搜索 API 为 Agent 提供真实搜索能力。

同时实现 `fetch_url` 工具，让模型可以抓取搜索结果中的网页正文进行深度阅读。引用指令通过 system prompt 实现，不作为 function tool。

## 强制约束

### Tool API

1. **`web_search(query, top_k, filters)`**
   - 返回候选网页列表，每项包含：`title`、`url`、`snippet`、`date`（可选）、`source`（可选）
   - `filters`：保留字段，暂不实现

2. **`fetch_url(url)`**
   - 抓取指定网页正文，返回 markdown/text
   - 使用 Rust 方案做 HTML → text 转换

3. **引用指令**
   - 通过 system prompt 指导模型基于搜索证据回答时引用来源 URL
   - 不作为 function tool

### 配置格式（agents.toml）

```toml
active_web_search = "provider-id"

[[web_search_providers]]
id               = "searxng-local"
endpoint         = "http://localhost:8080/search"
method           = "GET"                    # GET | POST，默认 GET
query_key        = "q"
top_k_param      = "num_results"            # 可选，top_k 映射到的请求参数名
# api_key        = "..."                    # 可选，无认证时不填
# auth_header    = "Authorization"          # 认证方式 1：HTTP header
# auth_prefix    = "Bearer "                # header 值前缀
# auth_query_param = "api_key"              # 认证方式 2：query param
# max_result_chars = 8000                   # 可选，截断原始结果的字符上限

[web_search_providers.params]               # 额外固定参数
format  = "json"
engines = "google,bing,duckduckgo"

[web_search_providers.result_map]           # 响应字段映射
results = "results"                         # dot-path 到结果数组
title   = "title"
url     = "url"
snippet = "content"
# date  = "publishedDate"                  # 可选
# source = "engine"                        # 可选
```

- `api_key` 直接写配置，不读环境变量
- `auth_header` 与 `auth_query_param` 互斥
- `params` 在 resolve 时预序列化：GET → `Vec<(String, String)>`，POST → `serde_json::Map`
- `result_map` 必填（`date` 和 `source` 映射可选），用于从 provider 响应中提取标准化字段

### Rust 数据结构

TOML 反序列化层：

```rust
// AgentsConfigFile 新增
active_web_search: Option<String>,
#[serde(default)]
web_search_providers: Vec<WebSearchProviderFile>,

#[derive(Deserialize)]
struct WebSearchProviderFile {
    id: String,
    endpoint: String,
    method: Option<String>,
    query_key: String,
    top_k_param: Option<String>,
    api_key: Option<String>,
    auth_header: Option<String>,
    auth_prefix: Option<String>,
    auth_query_param: Option<String>,
    max_result_chars: Option<usize>,
    #[serde(default)]
    params: HashMap<String, serde_json::Value>,
    result_map: WebSearchResultMapFile,
}

#[derive(Deserialize)]
struct WebSearchResultMapFile {
    results: String,
    title: String,
    url: String,
    snippet: String,
    date: Option<String>,
    source: Option<String>,
}
```

运行时层：

```rust
pub enum WebSearchHttpMethod { Get, Post }

pub enum WebSearchAuth {
    None,
    Header { header: String, prefix: Option<String>, api_key: String },
    QueryParam { param: String, api_key: String },
}

pub enum WebSearchParams {
    QueryPairs(Vec<(String, String)>),
    JsonObject(serde_json::Map<String, serde_json::Value>),
}

pub struct WebSearchResultMap {
    pub results_path: String,
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub date: Option<String>,
    pub source: Option<String>,
}

pub struct ResolvedWebSearchProvider {
    pub id: String,
    pub endpoint: String,
    pub method: WebSearchHttpMethod,
    pub query_key: String,
    pub top_k_param: Option<String>,
    pub auth: WebSearchAuth,
    pub max_result_chars: Option<usize>,
    pub params: WebSearchParams,
    pub result_map: WebSearchResultMap,
}

// AgentProviderRegistry 新增
pub active_web_search_id: Option<String>,
pub web_search_providers: Vec<ResolvedWebSearchProvider>,
```

### 校验规则

1. `id` 非空且不重复
2. `endpoint` 非空
3. `method` 只接受 GET/POST（不区分大小写），缺省 GET
4. `auth_header` 与 `auth_query_param` 互斥
5. 设置了 `auth_header` 或 `auth_query_param` 但没有 `api_key` → 报错
6. `active_web_search` 非空时必须指向已存在的 provider id
7. `result_map.results`、`result_map.title`、`result_map.url`、`result_map.snippet` 非空
8. `WebSearchAuth` 和 `ResolvedWebSearchProvider` 的 Debug impl 遮蔽 api_key

## Phase 1 — Config 数据结构与解析

**目标**：在现有 `config.rs` 中新增 web search provider 配置的反序列化、校验和 resolve 逻辑。

**前序保护**：不修改现有 provider/model 配置链路，不改变 `RigAgentConfig` 构造。

**验收**：
- `cargo test -p app-server-core --test llm_tests` 不回归
- 新增配置解析测试覆盖：正常解析、校验失败（互斥 auth、缺 api_key、重复 id、无效 method）、无 web search 配置时的默认行为

## Phase 2 — Web Search 执行层

**目标**：实现通用 HTTP 搜索请求构造、发送和结果标准化提取。

**前序保护**：Phase 1 config 解析逻辑不被修改。

**新增依赖**：workspace `Cargo.toml` 声明 `reqwest = { version = "0.13", default-features = false, features = ["json", "rustls-tls"] }`，`app-server-core` 引用 workspace 依赖。与 rig-core 共享同一份 reqwest 0.13，不新增编译产物。

**输入**：`ResolvedWebSearchProvider` + query + top_k
**输出**：标准化结果列表 `Vec<{title, url, snippet, date?, source?}>` 的 JSON 字符串

**验收**：
- 请求构造单元测试：GET/POST 两种方法、三种认证模式、top_k 参数映射
- 结果提取单元测试：dot-path 解析、可选字段缺失、max_result_chars 截断
- `cargo check -p app-server-core` 通过

## Phase 3 — Fetch URL 执行层

**目标**：实现 URL 抓取和 HTML → markdown/text 转换。

**前序保护**：Phase 1、2 不被修改。

**新增依赖**：Rust HTML→text 转换 crate（如 `htmd` 或 `html2text`）

**输入**：URL 字符串
**输出**：网页正文的 markdown/text 字符串

**边界处理**：
- 非 HTML 内容（plain text、JSON）直接返回
- 超时、连接失败、HTTP 错误返回明确错误信息
- 响应体大小限制，防止抓取超大页面

**验收**：
- HTML→text 转换单元测试
- 错误处理单元测试
- `cargo check -p app-server-core` 通过

## Phase 4 — Tool 注册与集成

**目标**：将 `web_search` 和 `fetch_url` 注册为 agent function tool，接入现有工具执行链路。

**前序保护**：现有 18 个工具的注册、权限、执行不被修改。Phase 1-3 不被修改。

**关键集成点**：
- 工具注册：在 `agent_tool_specs()` 中新增两个工具定义（JSON schema、category、mode）
- 工具执行：在 `WorkspaceToolExecutor` 中新增 match 分支
- 配置传递：`ResolvedWebSearchProvider` 从 `AgentProviderRegistry` 传入 executor
- 条件注册：仅在 `active_web_search` 配置存在时注册 `web_search` 工具；`fetch_url` 始终注册

**验收**：
- `cargo test -p app-server-core --test agent_tool_tests` 不回归
- 新增测试：web_search 工具调用 dispatch、fetch_url 工具调用 dispatch
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests` 不回归

## Phase 5 — System Prompt 与 Turn Context

**目标**：在 system prompt 中添加引用指令；更新 turn context 反映 function tool web search 可用性。

**前序保护**：Phase 1-4 不被修改。system prompt 现有内容（Section 1-11）的语义不被改变。

**变更范围**：
- `docs/cadquery-mvp/agent-system-prompt.md`：在 tool calling 或 response rules 区域添加引用规则——当回答基于 web search 结果时，必须引用来源 URL
- `agent.rs` 的 `build_turn_context`：当 function tool web search 可用时，在 provider-native capabilities 或单独区域说明；不再仅依赖 `native_web_search_enabled` 标志

**验收**：
- 人工审查 system prompt 变更
- `cargo test -p app-server-core --test llm_tests` 不回归

## Phase 6 — 端到端验证

**验收命令**：
1. `cargo test -p app-server-core --test llm_tests`
2. `cargo test -p app-server-core --test agent_tool_tests`
3. `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`
4. `cargo test -p app-server-protocol`
5. `bun run --cwd packages/studio-web test:unit`
6. `git diff --check`
7. 功能性验证：配置 SearXNG（或其他可用 provider）后，Agent 在需要搜索时调用 `web_search` 工具，并在回答中引用来源
