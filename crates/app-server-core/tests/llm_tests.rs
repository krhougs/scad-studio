use app_server_core::llm::{
    LlmConfig, LlmMessage, LlmToolCall, LlmToolDefinition, build_model_string, build_request_body,
    extract_delta_content, read_sse_stream,
};
use app_server_core::{
    AgentCadQueryCodeInput, AgentExecutionScope, AgentTurnInput, build_execute_messages,
    build_turn_context, build_turn_messages, cadquery_agent_system_prompt, extract_cadquery_code,
};
use app_server_protocol::{
    AgentMode, CadQueryObjectKind, ChatMessageRecord, ChatRole, PathHandle, SelectionKind,
    SelectionRef, WorkspaceId,
};
use std::cell::RefCell;
use std::io::Cursor;

#[test]
fn extract_cadquery_code_from_python_fenced_block() {
    let response = "Here is the code:\n```python\nimport cadquery as cq\nresult = cq.Workplane().box(10, 10, 10)\n```\nDone.";
    let code = extract_cadquery_code(response);
    assert_eq!(
        code,
        "import cadquery as cq\nresult = cq.Workplane().box(10, 10, 10)"
    );
}

#[test]
fn cadquery_agent_prompt_requires_model_description_and_named_refs() {
    let prompt = cadquery_agent_system_prompt();
    assert!(prompt.contains("MODEL_DESCRIPTION"));
    assert!(prompt.contains("MODEL_DETAILS"));
    assert!(prompt.contains("key_dimensions"));
    assert!(prompt.contains("manufacturing_or_placement_constraints"));
    assert!(prompt.contains("REFS.features"));
    assert!(prompt.contains("export_formats"));
    assert!(prompt.contains("export_targets"));
}

#[test]
fn extract_cadquery_code_from_generic_fenced_block() {
    let response = "```\nimport cadquery as cq\nresult = cq.Workplane().box(5, 5, 5)\n```";
    let code = extract_cadquery_code(response);
    assert_eq!(
        code,
        "import cadquery as cq\nresult = cq.Workplane().box(5, 5, 5)"
    );
}

#[test]
fn extract_cadquery_code_returns_whole_response_when_no_fence() {
    let response = "import cadquery as cq\nresult = cq.Workplane().box(1, 1, 1)";
    let code = extract_cadquery_code(response);
    assert_eq!(code, response);
}

#[test]
fn extract_cadquery_code_prefers_python_fence_over_generic() {
    let response = "```\ngeneric\n```\n\n```python\npython_code\n```";
    let code = extract_cadquery_code(response);
    assert_eq!(code, "python_code");
}

#[test]
fn extract_cadquery_code_trims_whitespace_inside_fence() {
    let response = "```python\n  \n  code_here  \n  \n```";
    let code = extract_cadquery_code(response);
    assert_eq!(code, "code_here");
}

#[test]
fn extract_delta_content_parses_valid_sse_chunk() {
    let json = r#"{"choices":[{"delta":{"content":"hello"},"index":0}]}"#;
    assert_eq!(extract_delta_content(json), Some("hello".into()));
}

#[test]
fn extract_delta_content_returns_none_for_empty_content() {
    let json = r#"{"choices":[{"delta":{"content":""},"index":0}]}"#;
    assert_eq!(extract_delta_content(json), None);
}

#[test]
fn extract_delta_content_returns_none_for_no_content_key() {
    let json = r#"{"choices":[{"delta":{"role":"assistant"},"index":0}]}"#;
    assert_eq!(extract_delta_content(json), None);
}

#[test]
fn extract_delta_content_returns_none_for_invalid_json() {
    assert_eq!(extract_delta_content("not json"), None);
}

#[test]
fn extract_delta_content_returns_none_for_empty_choices() {
    let json = r#"{"choices":[]}"#;
    assert_eq!(extract_delta_content(json), None);
}

#[test]
fn build_request_body_includes_all_fields() {
    let config = LlmConfig {
        base_url: "https://example.com/v1".into(),
        api_key: "test-key".into(),
        model: "gpt-4o".into(),
        timeout_secs: 60,
        max_tokens: 2048,
        temperature: 0.5,
    };
    let messages = vec![
        LlmMessage::new("system", "You are helpful."),
        LlmMessage::new("user", "Hello"),
    ];

    let body = build_request_body(&config, &messages, &[], true);

    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 2048);
    assert_eq!(body["temperature"], 0.5);
    let msgs = body["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[0]["content"], "You are helpful.");
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "Hello");
}

#[test]
fn build_request_body_respects_stream_false() {
    let config = LlmConfig {
        base_url: String::new(),
        api_key: String::new(),
        model: "test".into(),
        timeout_secs: 30,
        max_tokens: 1024,
        temperature: 0.0,
    };
    let body = build_request_body(&config, &[], &[], false);
    assert_eq!(body["stream"], false);
}

#[test]
fn read_sse_stream_collects_tokens_and_calls_back() {
    let sse_data = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"},\"index\":0}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\" world\"},\"index\":0}]}\n\n\
                    data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let tokens = RefCell::new(Vec::new());
    let result = read_sse_stream(std::io::BufReader::new(reader), &|token| {
        tokens.borrow_mut().push(token.to_owned());
        true
    });

    let response = result.unwrap();
    assert_eq!(response.content, "Hello world");
    assert!(response.tool_calls.is_empty());
    assert_eq!(tokens.into_inner(), vec!["Hello", " world"]);
}

#[test]
fn read_sse_stream_collects_reasoning_content_without_emitting_tokens() {
    let sse_data = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Need context. \"},\"index\":0}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Read file.\"},\"index\":0}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"Done\"},\"index\":0}]}\n\n\
                    data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let tokens = RefCell::new(Vec::new());
    let result = read_sse_stream(std::io::BufReader::new(reader), &|token| {
        tokens.borrow_mut().push(token.to_owned());
        true
    });

    let response = result.unwrap();
    assert_eq!(
        response.reasoning_content.as_deref(),
        Some("Need context. Read file.")
    );
    assert_eq!(response.content, "Done");
    assert_eq!(tokens.into_inner(), vec!["Done"]);
}

#[test]
fn read_sse_stream_forwards_reasoning_deltas_to_callback() {
    let sse_data = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Thinking \"},\"index\":0}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"through dimensions.\"},\"index\":0}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"Done\"},\"index\":0}]}\n\n\
                    data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let reasoning = RefCell::new(Vec::new());
    let result = app_server_core::llm::read_sse_stream_with_reasoning(
        std::io::BufReader::new(reader),
        &|_| true,
        &|delta| {
            reasoning.borrow_mut().push(delta.to_owned());
            true
        },
    );

    let response = result.unwrap();
    assert_eq!(response.content, "Done");
    assert_eq!(
        response.reasoning_content.as_deref(),
        Some("Thinking through dimensions.")
    );
    assert_eq!(
        reasoning.into_inner(),
        vec!["Thinking ", "through dimensions."]
    );
}

#[test]
fn read_sse_stream_stops_when_callback_returns_false() {
    let sse_data = "data: {\"choices\":[{\"delta\":{\"content\":\"first\"},\"index\":0}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"second\"},\"index\":0}]}\n\n\
                    data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let result = read_sse_stream(std::io::BufReader::new(reader), &|_token| false);

    assert_eq!(result.unwrap().content, "first");
}

#[test]
fn read_sse_stream_skips_non_data_lines() {
    let sse_data = ": comment\n\
                    event: message\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"index\":0}]}\n\n\
                    data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let result = read_sse_stream(std::io::BufReader::new(reader), &|_| true);
    assert_eq!(result.unwrap().content, "ok");
}

#[test]
fn read_sse_stream_returns_empty_for_no_content_chunks() {
    let sse_data = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"},\"index\":0}]}\n\n\
                    data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let result = read_sse_stream(std::io::BufReader::new(reader), &|_| true);
    assert_eq!(result.unwrap().content, "");
}

#[test]
fn build_turn_messages_includes_system_prompt_and_history() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "explain fillet".into(),
        history: vec![chat_msg("user", "hi"), chat_msg("assistant", "hello")],
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    };
    let messages = build_turn_messages(&input);
    assert_eq!(messages[0].role, "system");
    assert!(messages[0].content.contains("Role"));
    assert_eq!(messages[1].role, "user");
    assert_eq!(messages[1].content, "hi");
    assert_eq!(messages[2].role, "assistant");
    assert_eq!(messages[2].content, "hello");
    assert_eq!(messages[3].role, "user");
    assert!(messages[3].content.contains("explain fillet"));
}

#[test]
fn build_turn_messages_skips_empty_and_tool_history() {
    let input = AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "design a lid".into(),
        history: vec![
            chat_msg("user", "initial"),
            empty_msg("assistant"),
            tool_msg("tool result"),
        ],
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    };
    let messages = build_turn_messages(&input);
    let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
    assert_eq!(roles, vec!["system", "user", "user"]);
}

#[test]
fn build_turn_context_includes_mode_plan_ref_and_selection() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: vec![test_selection()],
        active_selection_index: Some(0),
        plan_ref: Some(
            PathHandle::new(WorkspaceId::new("ws"), ["plans", "2026050100-lid"]).unwrap(),
        ),
        context_refs: Vec::new(),
        execution_scope: Some(AgentExecutionScope::for_plan(
            "plans/2026050100-lid",
            "plans/2026050100-lid/plan-result.md",
            "parts/lid.py",
            CadQueryObjectKind::Part,
            vec!["parts/lid.py".into()],
            Vec::new(),
            vec!["outputs/lid.step".into()],
        )),
    };
    let context = build_turn_context(&input);
    assert!(context.contains("Mode: Agent"));
    assert!(context.contains("Plan ref: plans/2026050100-lid"));
    assert!(context.contains("Execution scope"));
    assert!(context.contains("target_path=parts/lid.py"));
    assert!(context.contains("plan_result_path=plans/2026050100-lid/plan-result.md"));
    assert!(context.contains("Viewer selection"));
}

#[test]
fn build_turn_context_omits_selection_when_empty() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(context.contains("Mode: Agent"));
    assert!(!context.contains("Viewer selection"));
    assert!(!context.contains("Plan ref"));
}

#[test]
fn build_turn_context_includes_context_refs() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: vec!["@face[top_lid:f_0]".into(), "@part[bottom_case]".into()],
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(context.contains("context refs"));
    assert!(context.contains("@face[top_lid:f_0]"));
    assert!(context.contains("@part[bottom_case]"));
}

#[test]
fn build_execute_messages_includes_structured_context() {
    let input = AgentCadQueryCodeInput {
        prompt: "make it taller".into(),
        history: vec![chat_msg("user", "previous request")],
        selections: vec![test_selection()],
        active_selection_index: Some(0),
        target_display_path: "parts/box.py".into(),
        target_type: CadQueryObjectKind::Part,
        execution_scope: None,
    };
    let messages = build_execute_messages(&input);
    assert_eq!(messages[0].role, "system");
    assert_eq!(messages[1].role, "user");
    assert_eq!(messages[1].content, "previous request");
    let last = &messages[2];
    assert_eq!(last.role, "user");
    assert!(last.content.contains("Mode: Agent"));
    assert!(last.content.contains("Target path: parts/box.py"));
    assert!(last.content.contains("Target type: part"));
    assert!(last.content.contains("```python"));
}

#[test]
fn llm_config_debug_hides_api_key() {
    let config = LlmConfig {
        base_url: "https://example.com".into(),
        api_key: "sk-secret-key-12345".into(),
        model: "gpt-4o".into(),
        timeout_secs: 60,
        max_tokens: 4096,
        temperature: 0.7,
    };
    let debug = format!("{:?}", config);
    assert!(!debug.contains("sk-secret-key-12345"));
    assert!(debug.contains("***"));
    assert!(debug.contains("example.com"));
}

fn chat_msg(role: &str, content: &str) -> ChatMessageRecord {
    ChatMessageRecord {
        message_id: "m1".into(),
        ts_ms: 1,
        role: match role {
            "assistant" => ChatRole::Assistant,
            _ => ChatRole::User,
        },
        content: content.into(),
        related_files: Vec::new(),
        tool_call_id: None,
        tool_calls: Vec::new(),
        tool_result: None,
        mesh_result: None,
        run_id: None,
    }
}

fn empty_msg(role: &str) -> ChatMessageRecord {
    chat_msg(role, "")
}

fn tool_msg(content: &str) -> ChatMessageRecord {
    ChatMessageRecord {
        message_id: "t1".into(),
        ts_ms: 1,
        role: ChatRole::Tool,
        content: content.into(),
        related_files: Vec::new(),
        tool_call_id: None,
        tool_calls: Vec::new(),
        tool_result: None,
        mesh_result: None,
        run_id: None,
    }
}

fn test_selection() -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_0]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.lid_alignment_surface]".into()),
        build_id: Some("sha256:test".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }
}

#[test]
fn build_model_string_base_only() {
    assert_eq!(build_model_string("gpt-5.5", None, None), "gpt-5.5");
}

#[test]
fn build_model_string_with_variant() {
    assert_eq!(
        build_model_string("gpt-5.5", Some("mini"), None),
        "gpt-5.5:mini"
    );
}

#[test]
fn build_model_string_with_service_level() {
    assert_eq!(
        build_model_string("gpt-5.5", None, Some("high")),
        "gpt-5.5:high"
    );
}

#[test]
fn build_model_string_with_both() {
    assert_eq!(
        build_model_string("gpt-5.5", Some("mini"), Some("low")),
        "gpt-5.5:mini:low"
    );
}

#[test]
fn build_model_string_empty_variant_ignored() {
    assert_eq!(build_model_string("gpt-5.5", Some(""), None), "gpt-5.5");
}

#[test]
fn read_sse_stream_parses_tool_calls() {
    let sse_data = "\
        data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":null,\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"\"}}]},\"index\":0}]}\n\n\
        data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"pa\"}}]},\"index\":0}]}\n\n\
        data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"th\\\": \\\"parts/lid.py\\\"}\"}}]},\"index\":0}]}\n\n\
        data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let response = read_sse_stream(std::io::BufReader::new(reader), &|_| true).unwrap();
    assert!(response.content.is_empty());
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].id, "call_1");
    assert_eq!(response.tool_calls[0].function_name, "read_file");
    assert_eq!(
        response.tool_calls[0].arguments,
        "{\"path\": \"parts/lid.py\"}"
    );
}

#[test]
fn read_sse_stream_parses_multiple_tool_calls() {
    let sse_data = "\
        data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_a\",\"type\":\"function\",\"function\":{\"name\":\"list_directory\",\"arguments\":\"{\\\"path\\\": \\\"\\\"}\"}}]},\"index\":0}]}\n\n\
        data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_b\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\": \\\"README.md\\\"}\"}}]},\"index\":0}]}\n\n\
        data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let response = read_sse_stream(std::io::BufReader::new(reader), &|_| true).unwrap();
    assert_eq!(response.tool_calls.len(), 2);
    assert_eq!(response.tool_calls[0].function_name, "list_directory");
    assert_eq!(response.tool_calls[1].function_name, "read_file");
}

#[test]
fn read_sse_stream_handles_content_and_tool_calls_together() {
    let sse_data = "\
        data: {\"choices\":[{\"delta\":{\"content\":\"Let me check.\"},\"index\":0}]}\n\n\
        data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_x\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\": \\\"a.py\\\"}\"}}]},\"index\":0}]}\n\n\
        data: [DONE]\n\n";
    let reader = Cursor::new(sse_data.as_bytes());

    let response = read_sse_stream(std::io::BufReader::new(reader), &|_| true).unwrap();
    assert_eq!(response.content, "Let me check.");
    assert_eq!(response.tool_calls.len(), 1);
}

#[test]
fn build_request_body_includes_tools_when_provided() {
    let config = LlmConfig {
        base_url: String::new(),
        api_key: String::new(),
        model: "test".into(),
        timeout_secs: 30,
        max_tokens: 1024,
        temperature: 0.0,
    };
    let tools = vec![LlmToolDefinition {
        name: "read_file".into(),
        description: "Read a file".into(),
        parameters: serde_json::json!({"type": "object", "properties": {}}),
    }];
    let body = build_request_body(&config, &[], &tools, true);
    let tools_arr = body["tools"].as_array().unwrap();
    assert_eq!(tools_arr.len(), 1);
    assert_eq!(tools_arr[0]["type"], "function");
    assert_eq!(tools_arr[0]["function"]["name"], "read_file");
}

#[test]
fn build_request_body_omits_tools_when_empty() {
    let config = LlmConfig {
        base_url: String::new(),
        api_key: String::new(),
        model: "test".into(),
        timeout_secs: 30,
        max_tokens: 1024,
        temperature: 0.0,
    };
    let body = build_request_body(&config, &[], &[], true);
    assert!(body.get("tools").is_none());
}

#[test]
fn build_request_body_serializes_tool_message() {
    let config = LlmConfig {
        base_url: String::new(),
        api_key: String::new(),
        model: "test".into(),
        timeout_secs: 30,
        max_tokens: 1024,
        temperature: 0.0,
    };
    let messages = vec![
        LlmMessage::assistant_with_tool_calls(
            String::new(),
            vec![LlmToolCall {
                id: "call_1".into(),
                function_name: "read_file".into(),
                arguments: "{\"path\": \"a.py\"}".into(),
            }],
        ),
        LlmMessage::tool_result("call_1".into(), "file content".into()),
    ];
    let body = build_request_body(&config, &messages, &[], true);
    let msgs = body["messages"].as_array().unwrap();
    assert_eq!(msgs[0]["role"], "assistant");
    assert!(msgs[0]["content"].is_null());
    assert_eq!(msgs[0]["tool_calls"][0]["id"], "call_1");
    assert_eq!(msgs[1]["role"], "tool");
    assert_eq!(msgs[1]["tool_call_id"], "call_1");
    assert_eq!(msgs[1]["content"], "file content");
}

#[test]
fn build_request_body_serializes_reasoning_content_for_assistant_tool_call() {
    let config = LlmConfig {
        base_url: String::new(),
        api_key: String::new(),
        model: "test".into(),
        timeout_secs: 30,
        max_tokens: 1024,
        temperature: 0.0,
    };
    let messages = vec![LlmMessage::assistant_with_reasoning_and_tool_calls(
        "checking".into(),
        "Need the current file contents.".into(),
        vec![LlmToolCall {
            id: "call_1".into(),
            function_name: "read_file".into(),
            arguments: "{\"path\": \"a.py\"}".into(),
        }],
    )];

    let body = build_request_body(&config, &messages, &[], true);
    let msgs = body["messages"].as_array().unwrap();
    assert_eq!(msgs[0]["role"], "assistant");
    assert_eq!(msgs[0]["content"], "checking");
    assert_eq!(
        msgs[0]["reasoning_content"],
        "Need the current file contents."
    );
    assert_eq!(msgs[0]["tool_calls"][0]["id"], "call_1");
}
