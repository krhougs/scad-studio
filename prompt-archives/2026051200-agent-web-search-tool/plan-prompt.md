# Agent Web Search Function Tool

## 背景

当前产品 Agent 的 web search 依赖 provider-hosted 能力（Anthropic `server_tool_use`、OpenAI Responses `web_search`），但实际使用的代理 provider（DeepSeek、MiMo 等）均不支持 server-side 执行。已实现的 `HostedToolHook` 可以拦截未执行的 hosted tool call 并返回 fallback，但这意味着模型实际上无法搜索。

需要实现一个自有的 web search function tool，通过外部搜索 API（SearXNG、Tavily、Brave、Exa、SerpAPI 等）为 Agent 提供真实搜索能力。

## 已确认的设计决策

### 配置格式（agents.toml）

```toml
active_web_search = "searxng-local"

[[web_search_providers]]
id               = "searxng-local"
endpoint         = "http://localhost:8080/search"
method           = "GET"
query_key        = "q"
max_result_chars = 8000

[web_search_providers.params]
format  = "json"
engines = "google,bing,duckduckgo"
```

- `active_web_search`：指向当前激活的 web search provider id
- 支持多个 `[[web_search_providers]]` 配置，通过 `active_web_search` 切换
- 认证方式三选一（互斥）：`auth_header` + 可选 `auth_prefix`、`auth_query_param`、无认证
- `api_key` 直接写配置，不读环境变量
- `method`：`GET` 或 `POST`，默认 `GET`
- `params`：额外固定参数，GET 拼 query string，POST 放 JSON body

### Rust 数据结构

TOML 反序列化层：

```rust
#[derive(Deserialize)]
struct WebSearchProviderFile {
    id: String,
    endpoint: String,
    method: Option<String>,
    query_key: String,
    api_key: Option<String>,
    auth_header: Option<String>,
    auth_prefix: Option<String>,
    auth_query_param: Option<String>,
    max_result_chars: Option<usize>,
    #[serde(default)]
    params: HashMap<String, serde_json::Value>,
}
```

运行时层：

```rust
#[derive(Clone, Debug)]
pub enum WebSearchHttpMethod {
    Get,
    Post,
}

#[derive(Clone)]
pub enum WebSearchAuth {
    None,
    Header { header: String, prefix: Option<String>, api_key: String },
    QueryParam { param: String, api_key: String },
}

#[derive(Clone, Debug)]
pub enum WebSearchParams {
    QueryPairs(Vec<(String, String)>),
    JsonObject(serde_json::Map<String, serde_json::Value>),
}

#[derive(Clone)]
pub struct ResolvedWebSearchProvider {
    pub id: String,
    pub endpoint: String,
    pub method: WebSearchHttpMethod,
    pub query_key: String,
    pub auth: WebSearchAuth,
    pub max_result_chars: Option<usize>,
    pub params: WebSearchParams,
}
```

`AgentsConfigFile` 新增 `active_web_search: Option<String>` 和 `web_search_providers: Vec<WebSearchProviderFile>`。

`AgentProviderRegistry` 新增 `active_web_search_id: Option<String>` 和 `web_search_providers: Vec<ResolvedWebSearchProvider>`。

### 校验规则

1. `id` 不能为空，不能重复
2. `endpoint` 不能为空
3. `method` 只接受 `GET` / `POST`（不区分大小写），缺省为 `GET`
4. `auth_header` 和 `auth_query_param` 互斥
5. 设置了 `auth_header` 或 `auth_query_param` 但没有 `api_key` → 报错
6. `active_web_search` 非空时必须指向已存在的 provider id
7. `WebSearchAuth` 和 `ResolvedWebSearchProvider` 的 Debug impl 遮蔽 api_key

### 响应处理

- 搜索 API 原始 JSON 响应直接返回给 Agent，不做字段映射或后处理
- `max_result_chars` 可选截断，防止超大响应占用过多上下文

### 与现有 hosted web search 的关系

- 当 `active_web_search` 配置存在时，注册 `web_search` function tool，模型通过标准 tool call 调用
- `HostedToolHook` 继续保留，拦截 provider 尝试 server-side 执行的 hosted tool call
- `native_web_search` 配置继续保留，控制是否向 provider 请求 hosted web search
- 两者可以共存：hosted 优先由 provider 执行，如果 provider 未执行则 hook 拦截；function tool 始终可用

## 强制约束

- 不修改 `agent-system-prompt.md`
- 不读环境变量获取 API key
- 配置校验在启动时完成，缺失或错误配置必须返回明确错误
- 搜索结果不做字段映射，原始 JSON 直接返回
- 新增文件不超过 500 行，新增函数不超过 50 行
- 测试放在独立 `tests/` 目录
