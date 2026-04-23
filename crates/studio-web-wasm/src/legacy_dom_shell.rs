#![cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]

use crate::{
    FakeChatState, MessageRole,
    preview_canvas::{
        PREVIEW_MESH_CANVAS_ID, PreviewCanvasState, render_mesh_preview,
        set_preview_canvas_dom_state,
    },
    transport_port::WebSocketAppServerTransportPort,
};
use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform, CommandSuccess,
    PathHandle, PreviewArtifact, PreviewRequest, PreviewRequestKind, ProtocolVersionRange,
    ServerPushEvent, WorkspaceEntry, WorkspaceEntryKind, WorkspaceListRequest,
    web_file_read_capability,
};
use scad_scene::MeshData;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use studio_common::{
    AppServerClient, AppServerClientEvent, DirectoryWatchLifecycle, PreviewState,
    WatchLifecycleRequest,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::spawn_local;
use web_sys::{
    Document, Event, HtmlCanvasElement, HtmlElement, HtmlTextAreaElement, KeyboardEvent, Window,
};

thread_local! {
    static APP_INSTANCE: RefCell<Option<Rc<RefCell<StudioWebApp>>>> = RefCell::new(None);
}

pub fn boot_studio_web(url: &str) -> Result<(), JsValue> {
    let app = Rc::new(RefCell::new(StudioWebApp::new(url)?));
    APP_INSTANCE.with(|slot| {
        if let Some(previous) = slot.borrow_mut().replace(Rc::clone(&app)) {
            if let Ok(mut previous) = previous.try_borrow_mut() {
                previous.shutdown();
            }
        }
    });
    app.borrow_mut().render()?;
    install_tick_loop(&app)?;
    Ok(())
}

pub fn boot_studio_web_from_window() -> Result<(), JsValue> {
    let window = window()?;
    let search = window.location().search()?;
    let url = query_param(&search, "ws").unwrap_or_else(|| "ws://127.0.0.1:39180".into());
    boot_studio_web(&url)
}

struct TreeDirInfo {
    dir_path: Option<PathHandle>,
}

struct StudioWebApp {
    client: AppServerClient<WebSocketAppServerTransportPort>,
    root: HtmlElement,
    render_callbacks: Vec<Closure<dyn FnMut(Event)>>,
    tick_callback: Option<Closure<dyn FnMut()>>,
    interval_id: Option<i32>,
    chat: FakeChatState,
    status: String,
    workspace_title: String,
    current_directory: Option<PathHandle>,
    tree_children: HashMap<Option<PathHandle>, Vec<WorkspaceEntry>>,
    expanded_dirs: HashSet<Option<PathHandle>>,
    tree_dir_infos: Vec<TreeDirInfo>,
    files: Vec<WorkspaceEntry>,
    watch: DirectoryWatchLifecycle,
    preview: PreviewState,
    preview_canvas: PreviewCanvasState,
    handshake_started: bool,
    auto_preview_started: bool,
}

impl StudioWebApp {
    fn new(url: &str) -> Result<Self, JsValue> {
        let root = ensure_root(&document()?)?;
        let transport = WebSocketAppServerTransportPort::connect(url)?;
        Ok(Self {
            client: AppServerClient::new(transport),
            root,
            render_callbacks: Vec::new(),
            tick_callback: None,
            interval_id: None,
            chat: FakeChatState::default(),
            status: format!("connecting to {url}"),
            workspace_title: "workspace pending".into(),
            current_directory: None,
            tree_children: HashMap::new(),
            expanded_dirs: HashSet::new(),
            tree_dir_infos: Vec::new(),
            files: Vec::new(),
            watch: DirectoryWatchLifecycle::default(),
            preview: PreviewState::default(),
            preview_canvas: PreviewCanvasState::default(),
            handshake_started: false,
            auto_preview_started: false,
        })
    }

    fn shutdown(&mut self) {
        if let Some(interval_id) = self.interval_id.take() {
            if let Ok(window) = window() {
                window.clear_interval_with_handle(interval_id);
            }
        }
        self.tick_callback = None;
        self.render_callbacks.clear();
    }

    fn render(&mut self) -> Result<(), JsValue> {
        let render_job = self.preview_canvas.next_render();
        self.render_callbacks.clear();
        let html = self.shell_html();
        self.root.set_inner_html(&html);
        let app = APP_INSTANCE
            .with(|slot| slot.borrow().as_ref().cloned())
            .ok_or_else(|| JsValue::from_str("studio-web app instance missing during render"))?;
        self.attach_handlers(&app)?;
        sync_chat_scroll(&document()?)?;
        if let Some((render_id, mesh)) = render_job {
            schedule_preview_render(render_id, mesh)?;
        }
        Ok(())
    }

    fn shell_html(&mut self) -> String {
        format!(
            r#"
            <div class="studio-web-shell">
              <style>
                body {{ font-family: -apple-system, BlinkMacSystemFont, sans-serif; margin: 0; background: #111827; color: #e5e7eb; }}
                .studio-web-shell {{ display: grid; gap: 16px; padding: 16px; }}
                .studio-web-grid {{ display: grid; gap: 16px; grid-template-columns: minmax(260px, 1fr) minmax(280px, 1fr); }}
                .studio-web-card {{ background: #1f2937; border: 1px solid #374151; border-radius: 12px; padding: 16px; }}
                .studio-web-card h2 {{ margin: 0 0 8px; font-size: 16px; }}
                .studio-web-card p {{ margin: 0 0 8px; color: #9ca3af; }}
                .studio-web-list {{ display: flex; flex-direction: column; gap: 8px; margin-top: 12px; }}
                .studio-web-list button {{ background: #111827; color: #e5e7eb; border: 1px solid #4b5563; border-radius: 8px; padding: 8px 10px; text-align: left; cursor: pointer; }}
                .studio-web-tree {{ display: flex; flex-direction: column; gap: 2px; margin-top: 12px; }}
                .studio-web-tree-node {{ display: flex; align-items: center; gap: 4px; }}
                .studio-web-tree-toggle {{ background: none; border: none; color: #9ca3af; cursor: pointer; padding: 4px; font-size: 12px; line-height: 1; min-width: 20px; }}
                .studio-web-tree-dir {{ background: #111827; color: #e5e7eb; border: 1px solid #4b5563; border-radius: 6px; padding: 6px 10px; text-align: left; cursor: pointer; }}
                .studio-web-actions {{ display: flex; gap: 8px; margin-top: 12px; }}
                .studio-web-chat-messages {{ max-height: 220px; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; margin-bottom: 12px; }}
                .studio-web-message {{ border-radius: 10px; padding: 10px; }}
                .studio-web-message-user {{ background: #2563eb; color: white; }}
                .studio-web-message-assistant {{ background: #374151; }}
                .studio-web-empty {{ color: #6b7280; font-style: italic; }}
                .studio-web-chat-input {{ width: 100%; min-height: 72px; resize: vertical; background: #111827; color: #e5e7eb; border: 1px solid #4b5563; border-radius: 8px; padding: 8px; box-sizing: border-box; }}
                .studio-web-preview-surface {{ display: flex; flex-direction: column; gap: 8px; margin-top: 12px; }}
                .studio-web-preview-canvas {{ width: 100%; min-height: 240px; background: #0b1220; border: 1px solid #374151; border-radius: 12px; }}
                .studio-web-preview-status {{ margin: 0; color: #9ca3af; font-size: 13px; }}
              </style>
              <div class="studio-web-card">
                <h1>{}</h1>
                <p id="connection-status">{}</p>
              </div>
              <div class="studio-web-grid">
                <section class="studio-web-card">
                  <h2>workspace</h2>
                  <p>{}</p>
                  <div class="studio-web-actions">
                    <button id="workspace-root-button">Back to root</button>
                  </div>
                  <div class="studio-web-tree" id="workspace-tree">{}</div>
                  <div class="studio-web-list" id="file-list">{}</div>
                </section>
                <section class="studio-web-card">
                  <h2>preview</h2>
                  <p>{}</p>
                  <p id="preview-summary">{}</p>
                  {}
                </section>
              </div>
              <section class="studio-web-card">
                <h2>fake chat</h2>
                <div class="studio-web-chat-messages" id="chat-messages">{}</div>
                <textarea id="chat-input" class="studio-web-chat-input" placeholder="Type a fake message and press Enter">{}</textarea>
                <div class="studio-web-actions">
                  <button id="chat-send-button">Send</button>
                  <button id="chat-clear-button">Clear</button>
                </div>
              </section>
            </div>
            "#,
            escape_html(crate::web_shell_label()),
            escape_html(&self.status),
            escape_html(&self.workspace_location_label()),
            self.workspace_tree_html(),
            self.file_buttons_html(),
            escape_html(self.preview.status_label()),
            escape_html(&self.preview.summary_label()),
            self.preview_canvas.html(),
            self.chat_messages_html(),
            escape_html(self.chat.draft()),
        )
    }

    fn workspace_location_label(&self) -> String {
        match self.current_directory.as_ref() {
            Some(handle) if !handle.display_path().is_empty() => {
                format!("{} / {}", self.workspace_title, handle.display_path())
            }
            _ => self.workspace_title.clone(),
        }
    }

    fn workspace_tree_html(&mut self) -> String {
        self.tree_dir_infos.clear();
        let (html, infos) = build_tree_html(&self.tree_children, &self.expanded_dirs);
        self.tree_dir_infos = infos;
        html
    }

    fn file_buttons_html(&self) -> String {
        if self.files.is_empty() {
            return "<p class=\"studio-web-empty\">No files in this directory.</p>".into();
        }
        self.files
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let label = if previewable(entry) {
                    "Preview"
                } else {
                    "File"
                };
                format!(
                    "<button id=\"workspace-file-{index}\">🧾 {} · {}</button>",
                    escape_html(&display_name(entry)),
                    label
                )
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn chat_messages_html(&self) -> String {
        if self.chat.messages().is_empty() {
            return "<p class=\"studio-web-empty\">No fake chat messages yet.</p>".into();
        }
        self.chat
            .messages()
            .iter()
            .map(|message| {
                let class_name = match message.role {
                    MessageRole::User => "studio-web-message studio-web-message-user",
                    MessageRole::Assistant => "studio-web-message studio-web-message-assistant",
                };
                format!(
                    "<div class=\"{class_name}\">{}</div>",
                    escape_html(&message.content)
                )
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn attach_handlers(&mut self, app: &Rc<RefCell<Self>>) -> Result<(), JsValue> {
        let document = document()?;
        self.attach_click(&document, "workspace-root-button", {
            let app = Rc::clone(app);
            move |_| {
                let _ = list_directory(&app, None);
            }
        })?;
        let tree_infos: Vec<(usize, Option<PathHandle>)> = self
            .tree_dir_infos
            .iter()
            .enumerate()
            .map(|(i, info)| (i, info.dir_path.clone()))
            .collect();
        for (index, dir_path) in tree_infos {
            self.attach_click(&document, &format!("tree-toggle-{index}"), {
                let app = Rc::clone(app);
                let dir_path = dir_path.clone();
                move |_| {
                    let mut app_ref = app.borrow_mut();
                    if !app_ref.tree_children.contains_key(&dir_path) {
                        return;
                    }
                    if app_ref.expanded_dirs.contains(&dir_path) {
                        app_ref.expanded_dirs.remove(&dir_path);
                    } else {
                        app_ref.expanded_dirs.insert(dir_path.clone());
                    }
                    let _ = app_ref.render();
                }
            })?;
            self.attach_click(&document, &format!("tree-dir-{index}"), {
                let app = Rc::clone(app);
                move |_| {
                    let _ = list_directory(&app, dir_path.clone());
                }
            })?;
        }
        for index in 0..self.files.len() {
            self.attach_click(&document, &format!("workspace-file-{index}"), {
                let app = Rc::clone(app);
                move |_| {
                    let handle = app.borrow().files[index].path.clone();
                    let _ = request_preview(&app, handle);
                }
            })?;
        }
        self.attach_click(&document, "chat-send-button", {
            let app = Rc::clone(app);
            move |_| {
                let _ = send_chat_message(&app);
            }
        })?;
        self.attach_click(&document, "chat-clear-button", {
            let app = Rc::clone(app);
            move |_| {
                app.borrow_mut().chat.clear_messages();
                let _ = app.borrow_mut().render();
            }
        })?;

        let input = document
            .get_element_by_id("chat-input")
            .ok_or_else(|| JsValue::from_str("chat input missing"))?
            .dyn_into::<HtmlTextAreaElement>()?;
        let on_input = {
            let app = Rc::clone(app);
            Closure::<dyn FnMut(Event)>::wrap(Box::new(move |event: Event| {
                if let Some(target) = event.target()
                    && let Ok(input) = target.dyn_into::<HtmlTextAreaElement>()
                {
                    app.borrow_mut().chat.set_draft(input.value());
                }
            }))
        };
        input.add_event_listener_with_callback("input", on_input.as_ref().unchecked_ref())?;
        self.render_callbacks.push(on_input);

        let on_keydown = {
            let app = Rc::clone(app);
            Closure::<dyn FnMut(Event)>::wrap(Box::new(move |event: Event| {
                let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                    return;
                };
                if event.key() == "Enter" && !event.shift_key() {
                    event.prevent_default();
                    let _ = send_chat_message(&app);
                }
            }))
        };
        input.add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref())?;
        self.render_callbacks.push(on_keydown);
        Ok(())
    }

    fn attach_click<F>(
        &mut self,
        document: &Document,
        element_id: &str,
        f: F,
    ) -> Result<(), JsValue>
    where
        F: FnMut(Event) + 'static,
    {
        let Some(element) = document.get_element_by_id(element_id) else {
            return Ok(());
        };
        let closure = Closure::<dyn FnMut(Event)>::wrap(Box::new(f));
        element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        self.render_callbacks.push(closure);
        Ok(())
    }

    fn sync_watch_directory(&mut self, directory: Option<PathHandle>) -> Result<(), JsValue> {
        if let Some(request) = self.watch.enter_directory(directory) {
            self.send_watch_request(request)?;
        }
        Ok(())
    }

    fn advance_watch_subscribed(
        &mut self,
        request_id: app_server_protocol::RequestId,
        ack: app_server_protocol::WatchSubscriptionAck,
    ) -> Result<(), JsValue> {
        if let Some(request) = self.watch.handle_watch_subscribed(request_id, ack) {
            self.send_watch_request(request)?;
        }
        Ok(())
    }

    fn advance_watch_unsubscribed(
        &mut self,
        request_id: app_server_protocol::RequestId,
        ack: app_server_protocol::WatchSubscriptionAck,
    ) -> Result<(), JsValue> {
        if let Some(request) = self.watch.handle_watch_unsubscribed(request_id, ack) {
            self.send_watch_request(request)?;
        }
        Ok(())
    }

    fn send_watch_request(&mut self, request: WatchLifecycleRequest) -> Result<(), JsValue> {
        let request_id = match &request {
            WatchLifecycleRequest::Subscribe(request) => {
                self.client.subscribe(request.clone()).map_err(js_error)?
            }
            WatchLifecycleRequest::Unsubscribe(request) => {
                self.client.unsubscribe(request.clone()).map_err(js_error)?
            }
        };
        self.watch.record_sent_request(request, request_id);
        Ok(())
    }
}

fn install_tick_loop(app: &Rc<RefCell<StudioWebApp>>) -> Result<(), JsValue> {
    let window = window()?;
    let app_for_tick = Rc::clone(app);
    let callback = Closure::<dyn FnMut()>::wrap(Box::new(move || {
        let _ = tick(&app_for_tick);
    }));
    let interval_id = window.set_interval_with_callback_and_timeout_and_arguments_0(
        callback.as_ref().unchecked_ref(),
        50,
    )?;
    let mut app = app.borrow_mut();
    app.interval_id = Some(interval_id);
    app.tick_callback = Some(callback);
    Ok(())
}

fn tick(app: &Rc<RefCell<StudioWebApp>>) -> Result<(), JsValue> {
    {
        let mut app = app.borrow_mut();
        if !app.handshake_started && app.client.transport().is_open() {
            app.client
                .begin_handshake(CapabilityHandshakeRequest {
                    capabilities: ClientCapabilities {
                        client_name: "studio-web".into(),
                        platform: ClientPlatform::Web,
                        protocol_version: ProtocolVersionRange::new(1, 1),
                        file_read: web_file_read_capability(),
                        supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
                    },
                })
                .map_err(js_error)?;
            app.handshake_started = true;
            app.status = "websocket open; handshake sent".into();
            app.render()?;
        }
    }

    loop {
        let event = app.borrow_mut().client.poll().map_err(js_error)?;
        let Some(event) = event else {
            break;
        };
        handle_client_event(app, event)?;
    }
    Ok(())
}

fn handle_client_event(
    app: &Rc<RefCell<StudioWebApp>>,
    event: AppServerClientEvent,
) -> Result<(), JsValue> {
    let mut app_state = app.borrow_mut();
    match event {
        AppServerClientEvent::HandshakeAccepted(response) => {
            app_state.status = format!(
                "connected · session {} · protocol {}",
                response.session_token.0, response.negotiated_version
            );
            app_state
                .client
                .send_command(ClientCommand::WorkspaceCurrent)
                .map_err(js_error)?;
        }
        AppServerClientEvent::Response(response) => {
            let request_id = response.request_id;
            match response.result {
                Ok(CommandSuccess::WorkspaceCurrent(current)) => {
                    app_state.workspace_title = current.root_name;
                    app_state.current_directory = None;
                    app_state
                        .client
                        .send_command(ClientCommand::WorkspaceList(WorkspaceListRequest {
                            directory: None,
                        }))
                        .map_err(js_error)?;
                }
                Ok(CommandSuccess::WorkspaceList(list)) => {
                    let listed_dir = list.directory.clone();
                    app_state.current_directory = listed_dir.clone();
                    let (_, files) = split_entries(list.entries.clone());
                    app_state
                        .tree_children
                        .insert(listed_dir.clone(), list.entries);
                    app_state.expanded_dirs.insert(listed_dir);
                    app_state.files = files;
                    app_state.status = "workspace listing loaded".into();
                    app_state.sync_watch_directory(list.directory)?;
                    if !app_state.auto_preview_started {
                        if let Some(handle) = app_state
                            .files
                            .iter()
                            .find(|entry| previewable(entry))
                            .map(|entry| entry.path.clone())
                        {
                            app_state.auto_preview_started = true;
                            drop(app_state);
                            request_preview(app, handle)?;
                            return Ok(());
                        }
                        app_state
                            .preview
                            .skip("No previewable file found in current directory.");
                    }
                }
                Ok(CommandSuccess::WatchSubscribed(ack)) => {
                    app_state.advance_watch_subscribed(request_id, ack)?;
                }
                Ok(CommandSuccess::WatchUnsubscribed(ack)) => {
                    app_state.advance_watch_unsubscribed(request_id, ack)?;
                }
                Ok(CommandSuccess::PreviewReady(ready)) => {
                    match &ready.artifact {
                        PreviewArtifact::Mesh(mesh) => {
                            let scene_mesh = mesh_from_preview_payload(mesh).map_err(js_error)?;
                            app_state.preview_canvas.set_mesh(scene_mesh);
                        }
                        _ => app_state.preview_canvas.clear(),
                    }
                    app_state.preview.complete(ready);
                }
                Ok(other) => {
                    app_state.status = format!("ignored protocol response: {other:?}");
                }
                Err(error) => {
                    let message = format!("{:?}: {}", error.code, error.message);
                    if app_state.preview.current_request().is_some() {
                        app_state.preview.fail(message.clone());
                    }
                    app_state.status = message;
                }
            }
        }
        AppServerClientEvent::Push(push) => match push.event {
            ServerPushEvent::WatchChanged(event) => {
                if let Some(directory) = app_state.watch.refresh_directory_for(&event) {
                    app_state.status = "watch change detected; refreshing listing".into();
                    drop(app_state);
                    list_directory(app, directory)?;
                    return Ok(());
                }
                app_state.status = "ignored watch change push".into();
            }
            ServerPushEvent::WatchError(event) => {
                app_state.status = format!("watch error: {}", event.message);
            }
        },
        AppServerClientEvent::TransportError(message) => {
            app_state.status = format!("transport error: {message}");
        }
        AppServerClientEvent::Closed => {
            app_state.status = "connection closed".into();
        }
    }
    app_state.render()
}

fn list_directory(
    app: &Rc<RefCell<StudioWebApp>>,
    directory: Option<PathHandle>,
) -> Result<(), JsValue> {
    let mut app = app.borrow_mut();
    app.client
        .send_command(ClientCommand::WorkspaceList(WorkspaceListRequest {
            directory,
        }))
        .map_err(js_error)?;
    app.status = "workspace.list sent".into();
    app.render()
}

fn request_preview(app: &Rc<RefCell<StudioWebApp>>, source: PathHandle) -> Result<(), JsValue> {
    let mut app = app.borrow_mut();
    let request = PreviewRequest {
        source,
        defines: Vec::new(),
        kind: PreviewRequestKind::GeometryArtifact,
        configured_openscad_path: None,
    };
    app.client
        .send_command(ClientCommand::PreviewRequest(request.clone()))
        .map_err(js_error)?;
    app.preview_canvas.clear();
    app.preview.start(request);
    app.render()
}

fn schedule_preview_render(render_id: u64, mesh: MeshData) -> Result<(), JsValue> {
    let dom = document()?;
    let Some(canvas) = dom.get_element_by_id(PREVIEW_MESH_CANVAS_ID) else {
        return Ok(());
    };
    let canvas = canvas.dyn_into::<HtmlCanvasElement>()?;
    set_preview_canvas_dom_state(&dom, "rendering", "mesh render running via scad-scene")?;

    spawn_local(async move {
        let result = render_mesh_preview(canvas, mesh).await;
        let Some(app) = APP_INSTANCE.with(|slot| slot.borrow().as_ref().cloned()) else {
            return;
        };
        if !app.borrow().preview_canvas.matches_render(render_id) {
            return;
        }
        let Ok(document) = document() else {
            return;
        };
        match result {
            Ok(()) => {
                let _ = set_preview_canvas_dom_state(&document, "ready", "mesh render ready");
            }
            Err(error) => {
                let _ = set_preview_canvas_dom_state(&document, "error", &error);
            }
        }
    });
    Ok(())
}

fn send_chat_message(app: &Rc<RefCell<StudioWebApp>>) -> Result<(), JsValue> {
    let mut app = app.borrow_mut();
    app.chat.send_draft();
    app.render()
}

fn split_entries(entries: Vec<WorkspaceEntry>) -> (Vec<WorkspaceEntry>, Vec<WorkspaceEntry>) {
    let mut directories = Vec::new();
    let mut files = Vec::new();
    for entry in entries {
        match entry.kind {
            WorkspaceEntryKind::Directory => directories.push(entry),
            WorkspaceEntryKind::File => files.push(entry),
        }
    }
    directories.sort_by(|left, right| display_name(left).cmp(&display_name(right)));
    files.sort_by(|left, right| display_name(left).cmp(&display_name(right)));
    (directories, files)
}

fn build_tree_html(
    tree_children: &HashMap<Option<PathHandle>, Vec<WorkspaceEntry>>,
    expanded_dirs: &HashSet<Option<PathHandle>>,
) -> (String, Vec<TreeDirInfo>) {
    let mut infos = Vec::new();
    let html = render_tree_level(tree_children, expanded_dirs, None, 0, &mut infos);
    (html, infos)
}

fn render_tree_level(
    tree_children: &HashMap<Option<PathHandle>, Vec<WorkspaceEntry>>,
    expanded_dirs: &HashSet<Option<PathHandle>>,
    parent: Option<PathHandle>,
    depth: usize,
    infos: &mut Vec<TreeDirInfo>,
) -> String {
    let Some(entries) = tree_children.get(&parent) else {
        if depth == 0 {
            return "<p class=\"studio-web-empty\">No child directories.</p>".into();
        }
        return String::new();
    };
    let dirs: Vec<&WorkspaceEntry> = entries
        .iter()
        .filter(|e| matches!(e.kind, WorkspaceEntryKind::Directory))
        .collect();
    if dirs.is_empty() && depth == 0 {
        return "<p class=\"studio-web-empty\">No child directories.</p>".into();
    }
    let mut html = String::new();
    for dir in dirs {
        let index = infos.len();
        let dir_path: Option<PathHandle> = Some(dir.path.clone());
        let is_expanded = expanded_dirs.contains(&dir_path);
        let has_cached = tree_children.contains_key(&dir_path);
        let toggle_char = if is_expanded { '\u{25BC}' } else { '\u{25B6}' };

        html.push_str(&format!(
            "<div class=\"studio-web-tree-node\" style=\"padding-left:{}px\">",
            depth * 20
        ));
        html.push_str(&format!(
            "<button class=\"studio-web-tree-toggle\" id=\"tree-toggle-{index}\">{toggle_char}</button>"
        ));
        html.push_str(&format!(
            "<button class=\"studio-web-tree-dir\" id=\"tree-dir-{index}\">\u{1F4C1} {}</button>",
            escape_html(&display_name(dir))
        ));

        if is_expanded && has_cached {
            let child_html = render_tree_level(
                tree_children,
                expanded_dirs,
                dir_path.clone(),
                depth + 1,
                infos,
            );
            html.push_str(&child_html);
        }

        html.push_str("</div>");

        infos.push(TreeDirInfo { dir_path });
    }
    html
}

fn previewable(entry: &WorkspaceEntry) -> bool {
    matches!(entry.kind, WorkspaceEntryKind::File)
        && matches!(
            entry.path.display_path().rsplit('.').next(),
            Some("stl" | "3mf")
        )
}

fn display_name(entry: &WorkspaceEntry) -> String {
    entry
        .path
        .path_segments()
        .last()
        .cloned()
        .unwrap_or_else(|| entry.path.display_path())
}

fn mesh_from_preview_payload(
    payload: &app_server_protocol::PreviewMeshPayload,
) -> Result<MeshData, String> {
    MeshData::from_indexed_buffers(
        payload.positions.clone(),
        payload.normals.clone(),
        payload.vertex_colors.clone(),
        payload.indices.clone(),
    )
    .map_err(|error| error.to_string())
}

fn ensure_root(document: &Document) -> Result<HtmlElement, JsValue> {
    if let Some(existing) = document.get_element_by_id("studio-web-root") {
        return Ok(existing.dyn_into::<HtmlElement>()?);
    }
    let root = document.create_element("div")?;
    root.set_attribute("id", "studio-web-root")?;
    document
        .body()
        .ok_or_else(|| JsValue::from_str("document body missing"))?
        .append_child(&root)?;
    Ok(root.dyn_into::<HtmlElement>()?)
}

fn sync_chat_scroll(document: &Document) -> Result<(), JsValue> {
    let Some(messages) = document.get_element_by_id("chat-messages") else {
        return Ok(());
    };
    let messages = messages.dyn_into::<HtmlElement>()?;
    messages.set_scroll_top(messages.scroll_height());
    Ok(())
}

fn query_param(search: &str, key: &str) -> Option<String> {
    search
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
        .find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            (name == key).then(|| value.to_string())
        })
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn js_error(message: impl ToString) -> JsValue {
    JsValue::from_str(&message.to_string())
}

fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| JsValue::from_str("window missing"))
}

fn document() -> Result<Document, JsValue> {
    window()?
        .document()
        .ok_or_else(|| JsValue::from_str("document missing"))
}
