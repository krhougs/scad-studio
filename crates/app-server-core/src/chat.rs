use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use app_server_protocol::{
    ChatAckResponse, ChatArchivedResponse, ChatCreatedResponse, ChatHistoryResponse,
    ChatListResponse, ChatMessageRecord, ChatRole, ChatSessionId, ChatSessionSummary,
    ChatToolCallRecord, ChatToolResultRecord, PathHandle, ProtocolError, ProtocolErrorCode,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ChatStore {
    workspace_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummaryUpdate {
    pub summary: String,
    pub goal: String,
    pub related_files: Vec<PathHandle>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonlMessage {
    message_id: String,
    ts_ms: u64,
    role: ChatRole,
    content: String,
    related_files: Vec<PathHandle>,
    tool_call_id: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ChatToolCallRecord>,
    #[serde(default)]
    tool_result: Option<ChatToolResultRecord>,
    #[serde(default)]
    mesh_result: Option<app_server_protocol::CadQueryResultReady>,
    #[serde(default)]
    run_id: Option<String>,
}

impl ChatStore {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    pub fn create(
        &self,
        title: &str,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
    ) -> Result<ChatCreatedResponse, ProtocolError> {
        let session_id = self.unique_session_id(title)?;
        let path = self.active_path(&session_id);
        ensure_parent(&self.workspace_root, &path)?;
        let content = goal.unwrap_or_else(|| format!("chat:{title}"));
        let meta = JsonlMessage::new("meta-1", ChatRole::Meta, content, related_files, None);
        append_jsonl(&self.workspace_root, &path, &meta)?;
        Ok(ChatCreatedResponse {
            session_id,
            title: title.to_owned(),
        })
    }

    pub fn list(&self, include_archived: bool) -> Result<ChatListResponse, ProtocolError> {
        let mut sessions = self.list_dir(&self.chats_dir(), false)?;
        if include_archived {
            sessions.extend(self.list_dir(&self.archive_dir(), true)?);
        }
        sessions.sort_by(|left, right| left.title.cmp(&right.title));
        Ok(ChatListResponse { sessions })
    }

    pub fn append_message(
        &self,
        session_id: &ChatSessionId,
        role: ChatRole,
        content: &str,
        related_files: Vec<PathHandle>,
        tool_call_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        self.append_message_with_run_id(
            session_id,
            role,
            content,
            related_files,
            tool_call_id,
            None,
        )
    }

    pub fn append_message_with_run_id(
        &self,
        session_id: &ChatSessionId,
        role: ChatRole,
        content: &str,
        related_files: Vec<PathHandle>,
        tool_call_id: Option<String>,
        run_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        validate_session_id(session_id)?;
        let path = self.session_path(session_id)?;
        let message_count = read_messages(&self.workspace_root, &path)?.len();
        let message_id = format!("msg-{}", message_count.saturating_add(1));
        let message = JsonlMessage::new(
            &message_id,
            role,
            content.to_owned(),
            related_files,
            tool_call_id,
        )
        .with_run_id(run_id);
        append_jsonl(&self.workspace_root, &path, &message)?;
        Ok(ChatAckResponse {
            session_id: session_id.clone(),
            message_id,
        })
    }

    pub fn append_tool_call(
        &self,
        session_id: &ChatSessionId,
        content: &str,
        tool_call: ChatToolCallRecord,
    ) -> Result<ChatAckResponse, ProtocolError> {
        self.append_tool_call_with_run_id(session_id, content, tool_call, None)
    }

    pub fn append_tool_call_with_run_id(
        &self,
        session_id: &ChatSessionId,
        content: &str,
        tool_call: ChatToolCallRecord,
        run_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        let mut message =
            self.next_message_with_run_id(session_id, ChatRole::Assistant, content, run_id)?;
        message.tool_call_id = Some(tool_call.tool_call_id.clone());
        message.tool_calls.push(tool_call);
        self.append_record(session_id, message)
    }

    pub fn append_tool_result(
        &self,
        session_id: &ChatSessionId,
        content: &str,
        tool_result: ChatToolResultRecord,
        mesh_result: Option<app_server_protocol::CadQueryResultReady>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        self.append_tool_result_with_run_id(session_id, content, tool_result, mesh_result, None)
    }

    pub fn append_tool_result_with_run_id(
        &self,
        session_id: &ChatSessionId,
        content: &str,
        tool_result: ChatToolResultRecord,
        mesh_result: Option<app_server_protocol::CadQueryResultReady>,
        run_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        let mut message =
            self.next_message_with_run_id(session_id, ChatRole::Tool, content, run_id)?;
        message.tool_call_id = Some(tool_result.tool_call_id.clone());
        message.tool_result = Some(tool_result);
        message.mesh_result = mesh_result;
        self.append_record(session_id, message)
    }

    pub fn update_summary(
        &self,
        session_id: &ChatSessionId,
        update: ChatSummaryUpdate,
    ) -> Result<ChatAckResponse, ProtocolError> {
        let content = summary_update_content(&update)?;
        self.append_message(
            session_id,
            ChatRole::Meta,
            &content,
            update.related_files,
            None,
        )
    }

    pub fn history(
        &self,
        session_id: &ChatSessionId,
        limit: Option<u32>,
    ) -> Result<ChatHistoryResponse, ProtocolError> {
        validate_session_id(session_id)?;
        let path = self.session_path(session_id)?;
        let mut messages = read_messages(&self.workspace_root, &path)?;
        if let Some(limit) = limit {
            trim_to_limit(&mut messages, limit as usize);
        }
        Ok(ChatHistoryResponse {
            session_id: session_id.clone(),
            messages: messages.into_iter().map(Into::into).collect(),
        })
    }

    pub fn archive(
        &self,
        session_id: &ChatSessionId,
    ) -> Result<ChatArchivedResponse, ProtocolError> {
        validate_session_id(session_id)?;
        let source = self.active_path(session_id);
        if !path_exists(&source) {
            return Err(not_found("Chat session 不存在"));
        }
        ensure_existing_jsonl_file(&self.workspace_root, &source)?;
        let target = self.archived_path(session_id);
        ensure_jsonl_file_writable(&self.workspace_root, &target)?;
        fs::rename(source, target)
            .map_err(|error| internal_error(format!("归档 Chat session 失败: {error}")))?;
        Ok(ChatArchivedResponse {
            session_id: session_id.clone(),
        })
    }

    fn unique_session_id(&self, title: &str) -> Result<ChatSessionId, ProtocolError> {
        let base = sanitize_session_id(title);
        for index in 1..=999 {
            let candidate = if index == 1 {
                base.clone()
            } else {
                format!("{base}-{index}")
            };
            let session_id = ChatSessionId(candidate);
            if !path_exists(&self.active_path(&session_id))
                && !path_exists(&self.archived_path(&session_id))
            {
                return Ok(session_id);
            }
        }
        Err(internal_error("无法创建唯一 Chat session id"))
    }

    fn list_dir(
        &self,
        dir: &Path,
        archived: bool,
    ) -> Result<Vec<ChatSessionSummary>, ProtocolError> {
        if !path_exists(dir) {
            return Ok(Vec::new());
        }
        ensure_safe_dir(&self.workspace_root, dir)?;
        let mut sessions = Vec::new();
        for entry in fs::read_dir(dir).map_err(|error| internal_error(error.to_string()))? {
            let entry = entry.map_err(|error| internal_error(error.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
                sessions.push(summary_from_path(&self.workspace_root, &path, archived)?);
            }
        }
        Ok(sessions)
    }

    fn session_path(&self, session_id: &ChatSessionId) -> Result<PathBuf, ProtocolError> {
        validate_session_id(session_id)?;
        let active = self.active_path(session_id);
        if path_exists(&active) {
            ensure_existing_jsonl_file(&self.workspace_root, &active)?;
            return Ok(active);
        }
        let archived = self.archived_path(session_id);
        if path_exists(&archived) {
            ensure_existing_jsonl_file(&self.workspace_root, &archived)?;
            return Ok(archived);
        }
        Err(not_found("Chat session 不存在"))
    }

    fn active_path(&self, session_id: &ChatSessionId) -> PathBuf {
        self.chats_dir().join(format!("{}.jsonl", session_id.0))
    }

    fn archived_path(&self, session_id: &ChatSessionId) -> PathBuf {
        self.archive_dir().join(format!("{}.jsonl", session_id.0))
    }

    fn chats_dir(&self) -> PathBuf {
        self.workspace_root.join("chats")
    }

    fn archive_dir(&self) -> PathBuf {
        self.chats_dir().join("archived")
    }

    fn next_message(
        &self,
        session_id: &ChatSessionId,
        role: ChatRole,
        content: &str,
    ) -> Result<JsonlMessage, ProtocolError> {
        self.next_message_with_run_id(session_id, role, content, None)
    }

    fn next_message_with_run_id(
        &self,
        session_id: &ChatSessionId,
        role: ChatRole,
        content: &str,
        run_id: Option<String>,
    ) -> Result<JsonlMessage, ProtocolError> {
        validate_session_id(session_id)?;
        let path = self.session_path(session_id)?;
        let message_count = read_messages(&self.workspace_root, &path)?.len();
        Ok(JsonlMessage::new(
            &format!("msg-{}", message_count.saturating_add(1)),
            role,
            content.to_owned(),
            Vec::new(),
            None,
        )
        .with_run_id(run_id))
    }

    fn append_record(
        &self,
        session_id: &ChatSessionId,
        message: JsonlMessage,
    ) -> Result<ChatAckResponse, ProtocolError> {
        let path = self.session_path(session_id)?;
        let message_id = message.message_id.clone();
        append_jsonl(&self.workspace_root, &path, &message)?;
        Ok(ChatAckResponse {
            session_id: session_id.clone(),
            message_id,
        })
    }
}

impl JsonlMessage {
    fn new(
        message_id: &str,
        role: ChatRole,
        content: String,
        related_files: Vec<PathHandle>,
        tool_call_id: Option<String>,
    ) -> Self {
        Self {
            message_id: message_id.to_owned(),
            ts_ms: now_ms(),
            role,
            content,
            related_files,
            tool_call_id,
            tool_calls: Vec::new(),
            tool_result: None,
            mesh_result: None,
            run_id: None,
        }
    }

    fn with_run_id(mut self, run_id: Option<String>) -> Self {
        self.run_id = run_id;
        self
    }
}

impl From<JsonlMessage> for ChatMessageRecord {
    fn from(value: JsonlMessage) -> Self {
        Self {
            message_id: value.message_id,
            ts_ms: value.ts_ms,
            role: value.role,
            content: value.content,
            related_files: value.related_files,
            tool_call_id: value.tool_call_id,
            tool_calls: value.tool_calls,
            tool_result: value.tool_result,
            mesh_result: value.mesh_result,
            run_id: value.run_id,
        }
    }
}

fn summary_from_path(
    workspace_root: &Path,
    path: &Path,
    archived: bool,
) -> Result<ChatSessionSummary, ProtocolError> {
    let messages = read_messages(workspace_root, path)?;
    let session_id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| internal_error("Chat session 文件名无效"))?
        .to_owned();
    let related_files = latest_related_files(&messages);
    Ok(ChatSessionSummary {
        session_id: ChatSessionId(session_id.clone()),
        title: session_id,
        archived,
        message_count: messages.len() as u32,
        related_files,
    })
}

fn latest_related_files(messages: &[JsonlMessage]) -> Vec<PathHandle> {
    if let Some(message) = messages
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::Meta && is_chat_summary_meta(message))
    {
        return message.related_files.clone();
    }
    messages
        .iter()
        .find(|message| !message.related_files.is_empty())
        .map(|message| message.related_files.clone())
        .unwrap_or_default()
}

fn is_chat_summary_meta(message: &JsonlMessage) -> bool {
    serde_json::from_str::<serde_json::Value>(&message.content)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(|meta_type| meta_type == "chat_summary")
        })
        .unwrap_or(false)
}

#[derive(Serialize)]
struct ChatSummaryMeta<'a> {
    #[serde(rename = "type")]
    meta_type: &'static str,
    summary: &'a str,
    goal: &'a str,
    related_files: Vec<String>,
    open_questions: &'a [String],
}

fn summary_update_content(update: &ChatSummaryUpdate) -> Result<String, ProtocolError> {
    let related_files = update
        .related_files
        .iter()
        .map(|path| path.display_path().to_owned())
        .collect::<Vec<_>>();
    serde_json::to_string(&ChatSummaryMeta {
        meta_type: "chat_summary",
        summary: update.summary.as_str(),
        goal: update.goal.as_str(),
        related_files,
        open_questions: update.open_questions.as_slice(),
    })
    .map_err(|error| internal_error(format!("序列化 Chat summary 失败: {error}")))
}

fn append_jsonl(
    workspace_root: &Path,
    path: &Path,
    message: &JsonlMessage,
) -> Result<(), ProtocolError> {
    ensure_jsonl_file_writable(workspace_root, path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| internal_error(format!("打开 Chat JSONL 失败: {error}")))?;
    let line = serde_json::to_string(message)
        .map_err(|error| internal_error(format!("序列化 Chat JSONL 失败: {error}")))?;
    writeln!(file, "{line}")
        .map_err(|error| internal_error(format!("写入 Chat JSONL 失败: {error}")))
}

fn read_messages(workspace_root: &Path, path: &Path) -> Result<Vec<JsonlMessage>, ProtocolError> {
    ensure_existing_jsonl_file(workspace_root, path)?;
    let file = File::open(path)
        .map_err(|error| internal_error(format!("读取 Chat JSONL 失败: {error}")))?;
    let mut messages = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| internal_error(error.to_string()))?;
        if !line.trim().is_empty() {
            messages.push(
                serde_json::from_str(&line).map_err(|error| internal_error(error.to_string()))?,
            );
        }
    }
    Ok(messages)
}

fn trim_to_limit(messages: &mut Vec<JsonlMessage>, limit: usize) {
    if messages.len() > limit {
        messages.drain(0..messages.len() - limit);
    }
}

fn sanitize_session_id(title: &str) -> String {
    let mut output = String::new();
    for character in title.chars().flat_map(|value| value.to_lowercase()) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    output.trim_matches('-').to_owned().if_empty("chat")
}

fn validate_session_id(session_id: &ChatSessionId) -> Result<(), ProtocolError> {
    let value = session_id.0.as_str();
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(ProtocolError::new(
            ProtocolErrorCode::InvalidCommand,
            "Chat session id 无效",
        ))
    }
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_owned()
        } else {
            self
        }
    }
}

fn ensure_parent(workspace_root: &Path, path: &Path) -> Result<(), ProtocolError> {
    let Some(parent) = path.parent() else {
        return Err(internal_error("路径缺少父目录"));
    };
    ensure_safe_dir(workspace_root, parent)
}

fn ensure_safe_dir(workspace_root: &Path, path: &Path) -> Result<(), ProtocolError> {
    let relative = path
        .strip_prefix(workspace_root)
        .map_err(|_| invalid_path("Chat 路径不在 workspace 内"))?;
    let mut current = workspace_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(invalid_path("Chat 路径不能逃逸 workspace"));
        };
        current.push(segment);
        ensure_safe_dir_component(&current)?;
    }
    Ok(())
}

fn ensure_safe_dir_component(path: &Path) -> Result<(), ProtocolError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid_path("Chat 目录不能是符号链接"))
        }
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(invalid_path("Chat 目录路径不能是普通文件")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match fs::create_dir(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                ensure_safe_dir_component(path)
            }
            Err(error) => Err(internal_error(format!("创建 Chat 目录失败: {error}"))),
        },
        Err(error) => Err(internal_error(format!("读取 Chat 目录失败: {error}"))),
    }
}

fn ensure_jsonl_file_writable(workspace_root: &Path, path: &Path) -> Result<(), ProtocolError> {
    ensure_parent(workspace_root, path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid_path("Chat JSONL 文件不能是符号链接"))
        }
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(invalid_path("Chat JSONL 路径不能是目录")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(internal_error(format!("读取 Chat JSONL 状态失败: {error}"))),
    }
}

fn ensure_existing_jsonl_file(workspace_root: &Path, path: &Path) -> Result<(), ProtocolError> {
    ensure_parent(workspace_root, path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid_path("Chat JSONL 文件不能是符号链接"))
        }
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(invalid_path("Chat JSONL 路径不能是目录")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(not_found("Chat session 不存在"))
        }
        Err(error) => Err(internal_error(format!("读取 Chat JSONL 状态失败: {error}"))),
    }
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn not_found(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::NotFound, message)
}

fn invalid_path(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidPathHandle, message)
}

fn internal_error(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::Internal, message)
}
