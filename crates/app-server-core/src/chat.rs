use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use app_server_protocol::{
    AgentId, AgentSearchSource, BoundAgentModel, ChatAckResponse, ChatArchivedResponse,
    ChatCreatedResponse, ChatHistoryResponse, ChatListResponse, ChatMessageRecord, ChatRole,
    ChatSessionId, ChatSessionSummary, ChatToolCallRecord, ChatToolResultRecord, PathHandle,
    ProtocolError, ProtocolErrorCode,
};
use serde::{Deserialize, Serialize};
use tokio::{fs, io::AsyncWriteExt};

const CHAT_INDEX_FILE: &str = "chats.json";
const CHAT_INDEX_VERSION: u32 = 1;
static CHAT_STORE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
    OnceLock::new();
static CHAT_INDEX_LISTENERS: OnceLock<Mutex<HashMap<PathBuf, HashMap<u64, ChatIndexListener>>>> =
    OnceLock::new();
static NEXT_CHAT_INDEX_LISTENER_ID: AtomicU64 = AtomicU64::new(1);

type ChatIndexListener = Arc<dyn Fn() + Send + Sync>;

pub struct ChatIndexListenerRegistration {
    workspace_root: PathBuf,
    id: u64,
}

impl Drop for ChatIndexListenerRegistration {
    fn drop(&mut self) {
        let Some(listeners) = CHAT_INDEX_LISTENERS.get() else {
            return;
        };
        let Ok(mut listeners) = listeners.lock() else {
            return;
        };
        if let Some(workspace_listeners) = listeners.get_mut(&self.workspace_root) {
            workspace_listeners.remove(&self.id);
            if workspace_listeners.is_empty() {
                listeners.remove(&self.workspace_root);
            }
        }
    }
}

pub fn register_chat_index_listener(
    workspace_root: &Path,
    listener: ChatIndexListener,
) -> ChatIndexListenerRegistration {
    let id = NEXT_CHAT_INDEX_LISTENER_ID.fetch_add(1, Ordering::SeqCst);
    let listeners = CHAT_INDEX_LISTENERS.get_or_init(|| Mutex::new(HashMap::new()));
    listeners
        .lock()
        .expect("Chat index listener map lock should not be poisoned")
        .entry(workspace_root.to_path_buf())
        .or_default()
        .insert(id, listener);
    ChatIndexListenerRegistration {
        workspace_root: workspace_root.to_path_buf(),
        id,
    }
}

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
    #[serde(default)]
    client_request_id: Option<String>,
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
    search_sources: Vec<AgentSearchSource>,
    #[serde(default)]
    run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatIndex {
    version: u32,
    active_chat_id: Option<ChatSessionId>,
    chats: Vec<ChatIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatIndexEntry {
    chat_id: ChatSessionId,
    agent_id: AgentId,
    create_request_id: Option<String>,
    title: String,
    goal: Option<String>,
    summary: Option<String>,
    open_questions: Vec<String>,
    archived: bool,
    created_at_ms: u64,
    updated_at_ms: u64,
    related_files: Vec<PathHandle>,
    messages_path: String,
    events_path: String,
    bound_model: Option<BoundAgentModel>,
}

struct ChatIndexCreateResult {
    entry: ChatIndexEntry,
    created_now: bool,
}

impl ChatStore {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    pub async fn create(
        &self,
        title: &str,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
    ) -> Result<ChatCreatedResponse, ProtocolError> {
        let created = self
            .create_indexed(title.to_owned(), goal, related_files, None, None, None)
            .await?;
        let entry = created.entry;
        Ok(ChatCreatedResponse {
            session_id: entry.chat_id,
            agent_id: entry.agent_id,
            title: entry.title,
            initial_turn: None,
        })
    }

    pub async fn create_with_client_request_id(
        &self,
        client_request_id: &str,
        title: &str,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
    ) -> Result<ChatCreatedResponse, ProtocolError> {
        self.create_with_client_request_id_and_initial_message(
            client_request_id,
            title,
            goal,
            related_files,
            None,
        )
        .await
    }

    pub async fn create_with_client_request_id_and_initial_message(
        &self,
        client_request_id: &str,
        title: &str,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
        initial_user_message: Option<String>,
    ) -> Result<ChatCreatedResponse, ProtocolError> {
        let created = self
            .create_indexed(
                title.to_owned(),
                goal,
                related_files,
                Some(client_request_id.to_owned()),
                initial_user_message,
                None,
            )
            .await?;
        let entry = created.entry;
        Ok(ChatCreatedResponse {
            session_id: entry.chat_id,
            agent_id: entry.agent_id,
            title: entry.title,
            initial_turn: None,
        })
    }

    pub async fn create_with_client_request_id_initial_message_and_model(
        &self,
        client_request_id: &str,
        title: &str,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
        initial_user_message: impl Into<String>,
        bound_model: Option<BoundAgentModel>,
    ) -> Result<ChatCreatedResponse, ProtocolError> {
        let (response, _) = self
            .create_with_client_request_id_initial_message_and_model_outcome(
                client_request_id,
                title,
                goal,
                related_files,
                initial_user_message,
                bound_model,
            )
            .await?;
        Ok(response)
    }

    pub async fn create_with_client_request_id_initial_message_and_model_outcome(
        &self,
        client_request_id: &str,
        title: &str,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
        initial_user_message: impl Into<String>,
        bound_model: Option<BoundAgentModel>,
    ) -> Result<(ChatCreatedResponse, bool), ProtocolError> {
        let created = self
            .create_indexed(
                title.to_owned(),
                goal,
                related_files,
                Some(client_request_id.to_owned()),
                Some(initial_user_message.into()),
                bound_model,
            )
            .await?;
        let created_now = created.created_now;
        let entry = created.entry;
        Ok((
            ChatCreatedResponse {
                session_id: entry.chat_id,
                agent_id: entry.agent_id,
                title: entry.title,
                initial_turn: None,
            },
            created_now,
        ))
    }

    pub async fn create_owned(
        self,
        title: String,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
    ) -> Result<ChatCreatedResponse, ProtocolError> {
        self.create(&title, goal, related_files).await
    }

    pub async fn list(&self, include_archived: bool) -> Result<ChatListResponse, ProtocolError> {
        let index = self.load_or_migrate_index().await?;
        let mut sessions = Vec::new();
        for entry in index
            .chats
            .iter()
            .filter(|entry| include_archived || !entry.archived)
        {
            sessions.push(self.summary_from_index_entry(entry).await?);
        }
        Ok(ChatListResponse {
            sessions,
            active_chat_id: index.active_chat_id,
        })
    }

    pub async fn list_owned(
        self,
        include_archived: bool,
    ) -> Result<ChatListResponse, ProtocolError> {
        self.list(include_archived).await
    }

    pub async fn append_message(
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
            None,
        )
        .await
    }

    pub async fn append_message_owned(
        self,
        session_id: ChatSessionId,
        role: ChatRole,
        content: String,
        related_files: Vec<PathHandle>,
        tool_call_id: Option<String>,
        client_request_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        self.append_message_with_run_id(
            &session_id,
            role,
            &content,
            related_files,
            tool_call_id,
            None,
            client_request_id,
        )
        .await
    }

    pub async fn append_message_with_run_id(
        &self,
        session_id: &ChatSessionId,
        role: ChatRole,
        content: &str,
        related_files: Vec<PathHandle>,
        tool_call_id: Option<String>,
        run_id: Option<String>,
        client_request_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        if client_request_id.is_some() {
            let write_lock = workspace_write_lock(&self.workspace_root)?;
            let _guard = write_lock.lock().await;
            return self
                .append_message_with_run_id_without_lock(
                    session_id,
                    role,
                    content,
                    related_files,
                    tool_call_id,
                    run_id,
                    client_request_id,
                )
                .await;
        }

        self.append_message_with_run_id_without_lock(
            session_id,
            role,
            content,
            related_files,
            tool_call_id,
            run_id,
            client_request_id,
        )
        .await
    }

    async fn append_message_with_run_id_without_lock(
        &self,
        session_id: &ChatSessionId,
        role: ChatRole,
        content: &str,
        related_files: Vec<PathHandle>,
        tool_call_id: Option<String>,
        run_id: Option<String>,
        client_request_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        validate_session_id(session_id)?;
        let path = if client_request_id.is_some() {
            self.session_path_without_lock(session_id).await?
        } else {
            self.session_path(session_id).await?
        };
        let messages = read_messages(&self.workspace_root, &path).await?;
        if let Some(request_id) = client_request_id.as_deref() {
            if let Some(existing) = messages
                .iter()
                .find(|message| message.client_request_id.as_deref() == Some(request_id))
            {
                return Ok(ChatAckResponse {
                    session_id: session_id.clone(),
                    message_id: existing.message_id.clone(),
                });
            }
        }
        let message_count = messages.len();
        let message_id = format!("msg-{}", message_count.saturating_add(1));
        let message = JsonlMessage::new(
            &message_id,
            role,
            content.to_owned(),
            related_files,
            tool_call_id,
        )
        .with_run_id(run_id)
        .with_client_request_id(client_request_id);
        append_jsonl(&self.workspace_root, &path, &message).await?;
        Ok(ChatAckResponse {
            session_id: session_id.clone(),
            message_id,
        })
    }

    pub async fn append_tool_call(
        &self,
        session_id: &ChatSessionId,
        content: &str,
        tool_call: ChatToolCallRecord,
    ) -> Result<ChatAckResponse, ProtocolError> {
        self.append_tool_call_with_run_id(session_id, content, tool_call, None)
            .await
    }

    pub async fn append_tool_call_with_run_id(
        &self,
        session_id: &ChatSessionId,
        content: &str,
        tool_call: ChatToolCallRecord,
        run_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        let mut message = self
            .next_message_with_run_id(session_id, ChatRole::Assistant, content, run_id)
            .await?;
        message.tool_call_id = Some(tool_call.tool_call_id.clone());
        message.tool_calls.push(tool_call);
        self.append_record(session_id, message).await
    }

    pub async fn append_tool_result(
        &self,
        session_id: &ChatSessionId,
        content: &str,
        tool_result: ChatToolResultRecord,
        mesh_result: Option<app_server_protocol::CadQueryResultReady>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        self.append_tool_result_with_run_id(session_id, content, tool_result, mesh_result, None)
            .await
    }

    pub async fn append_tool_result_with_run_id(
        &self,
        session_id: &ChatSessionId,
        content: &str,
        tool_result: ChatToolResultRecord,
        mesh_result: Option<app_server_protocol::CadQueryResultReady>,
        run_id: Option<String>,
    ) -> Result<ChatAckResponse, ProtocolError> {
        let mut message = self
            .next_message_with_run_id(session_id, ChatRole::Tool, content, run_id)
            .await?;
        message.tool_call_id = Some(tool_result.tool_call_id.clone());
        message.tool_result = Some(tool_result);
        message.mesh_result = mesh_result;
        self.append_record(session_id, message).await
    }

    pub async fn update_summary(
        &self,
        session_id: &ChatSessionId,
        update: ChatSummaryUpdate,
    ) -> Result<ChatAckResponse, ProtocolError> {
        let write_lock = workspace_write_lock(&self.workspace_root)?;
        let _guard = write_lock.lock().await;
        validate_session_id(session_id)?;
        let mut index = self.load_or_migrate_index_without_lock().await?;
        let entry = index
            .chats
            .iter_mut()
            .find(|entry| entry.chat_id == *session_id)
            .ok_or_else(|| not_found("Chat session 不存在"))?;
        entry.summary = Some(update.summary);
        entry.goal = Some(update.goal);
        entry.related_files = update.related_files;
        entry.open_questions = update.open_questions;
        entry.updated_at_ms = now_ms();
        self.write_index(&index).await?;
        notify_chat_index_changed(&self.workspace_root);
        Ok(ChatAckResponse {
            session_id: session_id.clone(),
            message_id: "metadata".into(),
        })
    }

    pub async fn history(
        &self,
        session_id: &ChatSessionId,
        limit: Option<u32>,
    ) -> Result<ChatHistoryResponse, ProtocolError> {
        validate_session_id(session_id)?;
        let path = self.session_path(session_id).await?;
        let mut messages = read_messages(&self.workspace_root, &path).await?;
        if let Some(limit) = limit {
            trim_to_limit(&mut messages, limit as usize);
        }
        Ok(ChatHistoryResponse {
            session_id: session_id.clone(),
            messages: messages.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn select(&self, session_id: &ChatSessionId) -> Result<(), ProtocolError> {
        let write_lock = workspace_write_lock(&self.workspace_root)?;
        let _guard = write_lock.lock().await;
        validate_session_id(session_id)?;
        let mut index = self.load_or_migrate_index_without_lock().await?;
        let entry = index
            .chats
            .iter_mut()
            .find(|entry| entry.chat_id == *session_id && !entry.archived)
            .ok_or_else(|| not_found("Chat session 不存在"))?;
        entry.updated_at_ms = now_ms();
        index.active_chat_id = Some(session_id.clone());
        self.write_index(&index).await
    }

    pub async fn history_owned(
        self,
        session_id: ChatSessionId,
        limit: Option<u32>,
    ) -> Result<ChatHistoryResponse, ProtocolError> {
        self.history(&session_id, limit).await
    }

    pub async fn archive(
        &self,
        session_id: &ChatSessionId,
    ) -> Result<ChatArchivedResponse, ProtocolError> {
        let write_lock = workspace_write_lock(&self.workspace_root)?;
        let _guard = write_lock.lock().await;
        validate_session_id(session_id)?;
        let mut index = self.load_or_migrate_index_without_lock().await?;
        let entry = index
            .chats
            .iter_mut()
            .find(|entry| entry.chat_id == *session_id)
            .ok_or_else(|| not_found("Chat session 不存在"))?;
        ensure_existing_jsonl_file(
            &self.workspace_root,
            &self.relative_path(&entry.messages_path)?,
        )
        .await?;
        entry.archived = true;
        entry.updated_at_ms = now_ms();
        if index.active_chat_id.as_ref() == Some(session_id) {
            index.active_chat_id = index
                .chats
                .iter()
                .find(|entry| !entry.archived)
                .map(|entry| entry.chat_id.clone());
        }
        self.write_index(&index).await?;
        Ok(ChatArchivedResponse {
            session_id: session_id.clone(),
        })
    }

    pub async fn archive_owned(
        self,
        session_id: ChatSessionId,
    ) -> Result<ChatArchivedResponse, ProtocolError> {
        self.archive(&session_id).await
    }

    async fn session_path(&self, session_id: &ChatSessionId) -> Result<PathBuf, ProtocolError> {
        validate_session_id(session_id)?;
        let index = self.load_or_migrate_index().await?;
        self.session_path_from_index(session_id, &index).await
    }

    async fn session_path_without_lock(
        &self,
        session_id: &ChatSessionId,
    ) -> Result<PathBuf, ProtocolError> {
        validate_session_id(session_id)?;
        let index = self.load_or_migrate_index_without_lock().await?;
        self.session_path_from_index(session_id, &index).await
    }

    async fn session_path_from_index(
        &self,
        session_id: &ChatSessionId,
        index: &ChatIndex,
    ) -> Result<PathBuf, ProtocolError> {
        let entry = index
            .chats
            .iter()
            .find(|entry| entry.chat_id == *session_id)
            .ok_or_else(|| not_found("Chat session 不存在"))?;
        let path = self.relative_path(&entry.messages_path)?;
        ensure_existing_jsonl_file(&self.workspace_root, &path).await?;
        Ok(path)
    }

    async fn create_indexed(
        &self,
        title: String,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
        create_request_id: Option<String>,
        initial_user_message: Option<String>,
        bound_model: Option<BoundAgentModel>,
    ) -> Result<ChatIndexCreateResult, ProtocolError> {
        let write_lock = workspace_write_lock(&self.workspace_root)?;
        let _guard = write_lock.lock().await;
        let mut index = self.load_or_migrate_index_without_lock().await?;
        if let Some(request_id) = create_request_id.as_deref() {
            if let Some(entry) = find_create_request(&index, request_id) {
                return Ok(ChatIndexCreateResult {
                    entry: entry.clone(),
                    created_now: false,
                });
            }
        }
        let entry =
            self.new_index_entry(title, goal, related_files, create_request_id, bound_model)?;
        let create_result = async {
            let path = self.relative_path(&entry.messages_path)?;
            let content = entry
                .goal
                .clone()
                .unwrap_or_else(|| format!("chat:{}", entry.title));
            let meta = JsonlMessage::new(
                "meta-1",
                ChatRole::Meta,
                content,
                entry.related_files.clone(),
                None,
            );
            append_jsonl(&self.workspace_root, &path, &meta).await?;
            if let Some(content) = initial_user_message {
                let user_message =
                    JsonlMessage::new("msg-2", ChatRole::User, content, Vec::new(), None)
                        .with_client_request_id(entry.create_request_id.clone());
                append_jsonl(&self.workspace_root, &path, &user_message).await?;
            }
            create_event_log_file(
                &self.workspace_root,
                &self.relative_path(&entry.events_path)?,
            )
            .await?;
            index.active_chat_id = Some(entry.chat_id.clone());
            index.chats.push(entry.clone());
            self.write_index(&index).await
        }
        .await;
        if let Err(error) = create_result {
            self.cleanup_created_entry_files(&entry).await;
            return Err(error);
        }
        Ok(ChatIndexCreateResult {
            entry,
            created_now: true,
        })
    }

    fn new_index_entry(
        &self,
        title: String,
        goal: Option<String>,
        related_files: Vec<PathHandle>,
        create_request_id: Option<String>,
        bound_model: Option<BoundAgentModel>,
    ) -> Result<ChatIndexEntry, ProtocolError> {
        let chat_id = random_identifier("chat")?;
        let agent_id = random_identifier("agent")?;
        let now = now_ms();
        Ok(ChatIndexEntry {
            chat_id: ChatSessionId(chat_id.clone()),
            agent_id: AgentId(agent_id.clone()),
            create_request_id,
            title,
            goal,
            summary: None,
            open_questions: Vec::new(),
            archived: false,
            created_at_ms: now,
            updated_at_ms: now,
            related_files,
            messages_path: format!("chats/{chat_id}.jsonl"),
            events_path: format!("agent-events/{agent_id}.jsonl"),
            bound_model,
        })
    }

    async fn load_or_migrate_index(&self) -> Result<ChatIndex, ProtocolError> {
        let path = self.index_path();
        match read_index_file(&path).await? {
            Some(content) => parse_index(&content),
            None => {
                let write_lock = workspace_write_lock(&self.workspace_root)?;
                let _guard = write_lock.lock().await;
                self.load_or_migrate_index_without_lock().await
            }
        }
    }

    async fn load_or_migrate_index_without_lock(&self) -> Result<ChatIndex, ProtocolError> {
        let path = self.index_path();
        match read_index_file(&path).await? {
            Some(content) => parse_index(&content),
            None => {
                let index = self.migrate_legacy_index().await?;
                self.ensure_event_log_files(&index).await?;
                self.write_index(&index).await?;
                Ok(index)
            }
        }
    }

    async fn migrate_legacy_index(&self) -> Result<ChatIndex, ProtocolError> {
        let mut chats = Vec::new();
        chats.extend(self.migrate_legacy_dir(&self.chats_dir(), false).await?);
        chats.extend(self.migrate_legacy_dir(&self.archive_dir(), true).await?);
        Ok(ChatIndex {
            version: CHAT_INDEX_VERSION,
            active_chat_id: chats
                .iter()
                .find(|entry| !entry.archived)
                .map(|entry| entry.chat_id.clone()),
            chats,
        })
    }

    async fn migrate_legacy_dir(
        &self,
        dir: &Path,
        archived: bool,
    ) -> Result<Vec<ChatIndexEntry>, ProtocolError> {
        if !path_exists(dir).await {
            return Ok(Vec::new());
        }
        ensure_safe_dir(&self.workspace_root, dir).await?;
        let mut entries = Vec::new();
        let mut read_dir = fs::read_dir(dir)
            .await
            .map_err(|error| internal_error(error.to_string()))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|error| internal_error(error.to_string()))?
        {
            if legacy_chat_file(&entry.path()) {
                entries.push(self.legacy_entry(entry.path(), archived).await?);
            }
        }
        Ok(entries)
    }

    async fn legacy_entry(
        &self,
        path: PathBuf,
        archived: bool,
    ) -> Result<ChatIndexEntry, ProtocolError> {
        ensure_existing_jsonl_file(&self.workspace_root, &path).await?;
        let title = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| internal_error("Chat session 文件名无效"))?
            .to_owned();
        let messages = read_messages(&self.workspace_root, &path).await?;
        let now = now_ms();
        let agent_id = random_identifier("agent")?;
        Ok(ChatIndexEntry {
            chat_id: ChatSessionId(random_identifier("chat")?),
            agent_id: AgentId(agent_id.clone()),
            create_request_id: None,
            title,
            goal: None,
            summary: None,
            open_questions: Vec::new(),
            archived,
            created_at_ms: now,
            updated_at_ms: now,
            related_files: latest_related_files(&messages),
            messages_path: self.relative_string(&path)?,
            events_path: format!("agent-events/{agent_id}.jsonl"),
            bound_model: None,
        })
    }

    pub async fn agent_id_for_session(
        &self,
        session_id: &ChatSessionId,
    ) -> Result<AgentId, ProtocolError> {
        let index = self.load_or_migrate_index().await?;
        index
            .chats
            .iter()
            .find(|entry| entry.chat_id == *session_id)
            .map(|entry| entry.agent_id.clone())
            .ok_or_else(|| not_found("Chat session 不存在"))
    }

    pub async fn bound_model_for_session(
        &self,
        session_id: &ChatSessionId,
    ) -> Result<Option<BoundAgentModel>, ProtocolError> {
        let index = self.load_or_migrate_index().await?;
        index
            .chats
            .iter()
            .find(|entry| entry.chat_id == *session_id)
            .map(|entry| entry.bound_model.clone())
            .ok_or_else(|| not_found("Chat session 不存在"))
    }

    pub async fn session_id_for_agent(
        &self,
        agent_id: &AgentId,
    ) -> Result<ChatSessionId, ProtocolError> {
        let index = self.load_or_migrate_index().await?;
        index
            .chats
            .iter()
            .find(|entry| entry.agent_id == *agent_id)
            .map(|entry| entry.chat_id.clone())
            .ok_or_else(|| not_found("Agent 不存在"))
    }

    pub async fn has_create_request_id(
        &self,
        client_request_id: &str,
    ) -> Result<bool, ProtocolError> {
        let index = self.load_or_migrate_index().await?;
        Ok(find_create_request(&index, client_request_id).is_some())
    }

    pub async fn bound_model_for_agent(
        &self,
        agent_id: &AgentId,
    ) -> Result<Option<BoundAgentModel>, ProtocolError> {
        let index = self.load_or_migrate_index().await?;
        index
            .chats
            .iter()
            .find(|entry| entry.agent_id == *agent_id)
            .map(|entry| entry.bound_model.clone())
            .ok_or_else(|| not_found("Agent 不存在"))
    }

    async fn summary_from_index_entry(
        &self,
        entry: &ChatIndexEntry,
    ) -> Result<ChatSessionSummary, ProtocolError> {
        let path = self.relative_path(&entry.messages_path)?;
        let message_count = read_messages(&self.workspace_root, &path).await?.len() as u32;
        Ok(ChatSessionSummary {
            session_id: entry.chat_id.clone(),
            title: entry.title.clone(),
            archived: entry.archived,
            message_count,
            agent_id: entry.agent_id.clone(),
            related_files: entry.related_files.clone(),
            bound_model: entry.bound_model.clone(),
        })
    }

    async fn write_index(&self, index: &ChatIndex) -> Result<(), ProtocolError> {
        let path = self.index_path();
        ensure_index_file_writable(&path).await?;
        let tmp = self.index_tmp_path();
        ensure_index_tmp_file_writable(&tmp).await?;
        let content = serde_json::to_string_pretty(index)
            .map_err(|error| internal_error(format!("序列化 chats.json 失败: {error}")))?;
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .await
                .map_err(|error| {
                    internal_error(format!("创建 chats.json 临时文件失败: {error}"))
                })?;
            file.write_all(content.as_bytes()).await.map_err(|error| {
                internal_error(format!("写入 chats.json 临时文件失败: {error}"))
            })?;
            file.flush().await.map_err(|error| {
                internal_error(format!("刷新 chats.json 临时文件失败: {error}"))
            })?;
        }
        fs::rename(&tmp, &path)
            .await
            .map_err(|error| internal_error(format!("提交 chats.json 失败: {error}")))
    }

    fn relative_path(&self, relative: &str) -> Result<PathBuf, ProtocolError> {
        validate_relative_storage_path(relative)?;
        Ok(self.workspace_root.join(relative))
    }

    fn relative_string(&self, path: &Path) -> Result<String, ProtocolError> {
        let relative = path
            .strip_prefix(&self.workspace_root)
            .map_err(|_| invalid_path("Chat 路径不在 workspace 内"))?;
        relative_path_to_string(relative)
    }

    fn index_path(&self) -> PathBuf {
        self.workspace_root.join(CHAT_INDEX_FILE)
    }

    fn index_tmp_path(&self) -> PathBuf {
        self.workspace_root.join(format!("{CHAT_INDEX_FILE}.tmp"))
    }

    fn chats_dir(&self) -> PathBuf {
        self.workspace_root.join("chats")
    }

    fn archive_dir(&self) -> PathBuf {
        self.chats_dir().join("archived")
    }

    async fn ensure_event_log_files(&self, index: &ChatIndex) -> Result<(), ProtocolError> {
        for entry in &index.chats {
            create_event_log_file(
                &self.workspace_root,
                &self.relative_path(&entry.events_path)?,
            )
            .await?;
        }
        Ok(())
    }

    async fn cleanup_created_entry_files(&self, entry: &ChatIndexEntry) {
        if let Ok(path) = self.relative_path(&entry.messages_path) {
            let _ = remove_regular_file_if_exists(&path).await;
        }
        if let Ok(path) = self.relative_path(&entry.events_path) {
            let _ = remove_regular_file_if_exists(&path).await;
        }
    }

    async fn next_message_with_run_id(
        &self,
        session_id: &ChatSessionId,
        role: ChatRole,
        content: &str,
        run_id: Option<String>,
    ) -> Result<JsonlMessage, ProtocolError> {
        validate_session_id(session_id)?;
        let path = self.session_path(session_id).await?;
        let message_count = read_messages(&self.workspace_root, &path).await?.len();
        Ok(JsonlMessage::new(
            &format!("msg-{}", message_count.saturating_add(1)),
            role,
            content.to_owned(),
            Vec::new(),
            None,
        )
        .with_run_id(run_id))
    }

    async fn append_record(
        &self,
        session_id: &ChatSessionId,
        message: JsonlMessage,
    ) -> Result<ChatAckResponse, ProtocolError> {
        let path = self.session_path(session_id).await?;
        let message_id = message.message_id.clone();
        append_jsonl(&self.workspace_root, &path, &message).await?;
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
            client_request_id: None,
            ts_ms: now_ms(),
            role,
            content,
            related_files,
            tool_call_id,
            tool_calls: Vec::new(),
            tool_result: None,
            mesh_result: None,
            search_sources: Vec::new(),
            run_id: None,
        }
    }

    fn with_run_id(mut self, run_id: Option<String>) -> Self {
        self.run_id = run_id;
        self
    }

    fn with_client_request_id(mut self, client_request_id: Option<String>) -> Self {
        self.client_request_id = client_request_id;
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
            search_sources: value.search_sources,
            run_id: value.run_id,
        }
    }
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

fn parse_index(content: &str) -> Result<ChatIndex, ProtocolError> {
    let index: ChatIndex = serde_json::from_str(content)
        .map_err(|error| internal_error(format!("解析 chats.json 失败: {error}")))?;
    if index.version != CHAT_INDEX_VERSION {
        return Err(internal_error(format!(
            "不支持的 chats.json version: {}",
            index.version
        )));
    }
    for entry in &index.chats {
        validate_session_id(&entry.chat_id)?;
        validate_relative_storage_path(&entry.messages_path)?;
        validate_relative_storage_path(&entry.events_path)?;
    }
    Ok(index)
}

fn workspace_write_lock(
    workspace_root: &Path,
) -> Result<Arc<tokio::sync::Mutex<()>>, ProtocolError> {
    let locks = CHAT_STORE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks
        .lock()
        .map_err(|_| internal_error("Chat store lock poisoned"))?;
    Ok(Arc::clone(
        locks
            .entry(workspace_root.to_path_buf())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
    ))
}

fn notify_chat_index_changed(workspace_root: &Path) {
    let Some(listeners) = CHAT_INDEX_LISTENERS.get() else {
        return;
    };
    let callbacks = listeners
        .lock()
        .expect("Chat index listener map lock should not be poisoned")
        .get(workspace_root)
        .map(|listeners| listeners.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for callback in callbacks {
        callback();
    }
}

fn find_create_request<'a>(index: &'a ChatIndex, request_id: &str) -> Option<&'a ChatIndexEntry> {
    index
        .chats
        .iter()
        .find(|entry| entry.create_request_id.as_deref() == Some(request_id))
}

fn legacy_chat_file(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("jsonl")
}

fn random_identifier(prefix: &str) -> Result<String, ProtocolError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| internal_error(format!("生成随机 id 失败: {error}")))?;
    let mut output = String::with_capacity(prefix.len() + 1 + bytes.len() * 2);
    output.push_str(prefix);
    output.push('-');
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}")
            .map_err(|error| internal_error(format!("格式化随机 id 失败: {error}")))?;
    }
    Ok(output)
}

fn validate_relative_storage_path(path: &str) -> Result<(), ProtocolError> {
    if path.is_empty() {
        return Err(invalid_path("Chat 存储路径不能为空"));
    }
    let relative = Path::new(path);
    if relative.is_absolute() {
        return Err(invalid_path("Chat 存储路径不能是绝对路径"));
    }
    for component in relative.components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(invalid_path("Chat 存储路径不能逃逸 workspace")),
        }
    }
    Ok(())
}

fn relative_path_to_string(path: &Path) -> Result<String, ProtocolError> {
    let mut segments = Vec::new();
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return Err(invalid_path("Chat 存储路径不能逃逸 workspace"));
        };
        segments.push(
            segment
                .to_str()
                .ok_or_else(|| invalid_path("Chat 存储路径必须是 UTF-8"))?
                .to_owned(),
        );
    }
    Ok(segments.join("/"))
}

async fn ensure_index_file_writable(path: &Path) -> Result<(), ProtocolError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid_path("chats.json 不能是符号链接"))
        }
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(invalid_path("chats.json 路径不能是目录")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(internal_error(format!("读取 chats.json 状态失败: {error}"))),
    }
}

async fn read_index_file(path: &Path) -> Result<Option<String>, ProtocolError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid_path("chats.json 不能是符号链接"))
        }
        Ok(metadata) if metadata.is_file() => fs::read_to_string(path)
            .await
            .map(Some)
            .map_err(|error| internal_error(format!("读取 chats.json 失败: {error}"))),
        Ok(_) => Err(invalid_path("chats.json 路径不能是目录")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(internal_error(format!("读取 chats.json 状态失败: {error}"))),
    }
}

async fn ensure_index_tmp_file_writable(path: &Path) -> Result<(), ProtocolError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid_path("chats.json 临时文件不能是符号链接"))
        }
        Ok(metadata) if metadata.is_file() => fs::remove_file(path)
            .await
            .map_err(|error| internal_error(format!("清理 chats.json 临时文件失败: {error}"))),
        Ok(_) => Err(invalid_path("chats.json 临时文件路径不能是目录")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(internal_error(format!(
            "读取 chats.json 临时文件状态失败: {error}"
        ))),
    }
}

async fn create_event_log_file(workspace_root: &Path, path: &Path) -> Result<(), ProtocolError> {
    ensure_jsonl_file_writable(workspace_root, path).await?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .map(|_| ())
        .map_err(|error| internal_error(format!("创建 Agent event JSONL 失败: {error}")))
}

async fn remove_regular_file_if_exists(path: &Path) -> Result<(), ProtocolError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => Ok(()),
        Ok(metadata) if metadata.is_file() => fs::remove_file(path)
            .await
            .map_err(|error| internal_error(format!("清理 Chat 创建文件失败: {error}"))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(internal_error(format!(
            "读取 Chat 创建文件状态失败: {error}"
        ))),
    }
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

async fn append_jsonl(
    workspace_root: &Path,
    path: &Path,
    message: &JsonlMessage,
) -> Result<(), ProtocolError> {
    ensure_jsonl_file_writable(workspace_root, path).await?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .map_err(|error| internal_error(format!("打开 Chat JSONL 失败: {error}")))?;
    let line = serde_json::to_string(message)
        .map_err(|error| internal_error(format!("序列化 Chat JSONL 失败: {error}")))?;
    file.write_all(format!("{line}\n").as_bytes())
        .await
        .map_err(|error| internal_error(format!("写入 Chat JSONL 失败: {error}")))
}

async fn read_messages(
    workspace_root: &Path,
    path: &Path,
) -> Result<Vec<JsonlMessage>, ProtocolError> {
    ensure_existing_jsonl_file(workspace_root, path).await?;
    let contents = fs::read_to_string(path)
        .await
        .map_err(|error| internal_error(format!("读取 Chat JSONL 失败: {error}")))?;
    let mut messages = Vec::new();
    for line in contents.lines() {
        if !line.trim().is_empty() {
            messages.push(
                serde_json::from_str(line).map_err(|error| internal_error(error.to_string()))?,
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

async fn ensure_parent(workspace_root: &Path, path: &Path) -> Result<(), ProtocolError> {
    let Some(parent) = path.parent() else {
        return Err(internal_error("路径缺少父目录"));
    };
    ensure_safe_dir(workspace_root, parent).await
}

async fn ensure_safe_dir(workspace_root: &Path, path: &Path) -> Result<(), ProtocolError> {
    let relative = path
        .strip_prefix(workspace_root)
        .map_err(|_| invalid_path("Chat 路径不在 workspace 内"))?;
    let mut current = workspace_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(invalid_path("Chat 路径不能逃逸 workspace"));
        };
        current.push(segment);
        ensure_safe_dir_component(&current).await?;
    }
    Ok(())
}

async fn ensure_safe_dir_component(path: &Path) -> Result<(), ProtocolError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) => validate_safe_dir_metadata(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match fs::create_dir(path).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(path)
                        .await
                        .map_err(|error| internal_error(format!("读取 Chat 目录失败: {error}")))?;
                    validate_safe_dir_metadata(metadata)
                }
                Err(error) => Err(internal_error(format!("创建 Chat 目录失败: {error}"))),
            }
        }
        Err(error) => Err(internal_error(format!("读取 Chat 目录失败: {error}"))),
    }
}

fn validate_safe_dir_metadata(metadata: std::fs::Metadata) -> Result<(), ProtocolError> {
    if metadata.file_type().is_symlink() {
        Err(invalid_path("Chat 目录不能是符号链接"))
    } else if metadata.is_dir() {
        Ok(())
    } else {
        Err(invalid_path("Chat 目录路径不能是普通文件"))
    }
}

async fn ensure_jsonl_file_writable(
    workspace_root: &Path,
    path: &Path,
) -> Result<(), ProtocolError> {
    ensure_parent(workspace_root, path).await?;
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid_path("Chat JSONL 文件不能是符号链接"))
        }
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(invalid_path("Chat JSONL 路径不能是目录")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(internal_error(format!("读取 Chat JSONL 状态失败: {error}"))),
    }
}

async fn ensure_existing_jsonl_file(
    workspace_root: &Path,
    path: &Path,
) -> Result<(), ProtocolError> {
    ensure_parent(workspace_root, path).await?;
    match fs::symlink_metadata(path).await {
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

async fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).await.is_ok()
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
