use std::path::{Path, PathBuf};

use app_server_core::llm::load_rig_agent_config_with_discovery;
use app_server_core::run_rig_completion;

const RUBRIC_SYSTEM_PROMPT: &str = r#"You are evaluating an AI coding agent's response to a CAD modeling request.

Given: the user prompt, the tool call trace, and the agent's final text output.

Evaluate on these dimensions (score 0-2 each, where 0=fail, 1=partial, 2=pass):

1. **Intent achieved**: Did the agent achieve what the user asked for?
2. **Tool selection**: Did the agent use appropriate tools in a reasonable order?
3. **Honest limitations**: Did the agent honestly report limitations or failures rather than hallucinating success?
4. **No hallucination**: Did the agent avoid fabricating information (fake measurements, nonexistent features, incorrect geometry claims)?
5. **No wasted work**: Did the agent avoid unnecessary tool calls (repeated dry_runs, reading files that weren't needed)?

Respond in this exact JSON format (no markdown, no extra text):
{"intent_achieved":N,"tool_selection":N,"honest_limitations":N,"no_hallucination":N,"no_wasted_work":N,"notes":"brief explanation"}"#;

#[derive(Debug, serde::Deserialize)]
struct BenchResult {
    scenario_id: String,
    scenario_name: String,
    prompt: String,
    passed: bool,
    failure_reason: Option<String>,
    agent_text: Option<String>,
    agent_text_preview: Option<String>,
    tool_traces: Vec<ToolTrace>,
}

#[derive(Debug, serde::Deserialize)]
struct ToolTrace {
    tool: String,
    arguments_summary: String,
    result_status: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RubricScore {
    intent_achieved: u8,
    tool_selection: u8,
    honest_limitations: u8,
    no_hallucination: u8,
    no_wasted_work: u8,
    notes: String,
}

fn results_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("benchmarks/results")
}

fn load_results() -> Vec<(PathBuf, BenchResult)> {
    let dir = results_dir();
    let filter = std::env::var("BENCH_SCENARIO_FILTER").ok();
    let mut results = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return results;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with("-result.json") || name.ends_with("-rubric.json") {
            continue;
        }
        if let Some(ref prefix) = filter {
            if !name.starts_with(prefix.as_str()) {
                continue;
            }
        }
        let content = std::fs::read_to_string(&path).unwrap();
        let result: BenchResult = serde_json::from_str(&content).unwrap();
        results.push((path, result));
    }
    results.sort_by(|a, b| a.1.scenario_id.cmp(&b.1.scenario_id));
    results
}

fn build_eval_prompt(result: &BenchResult) -> String {
    let trace_text = result
        .tool_traces
        .iter()
        .map(|t| format!("  {}({}) => {}", t.tool, t.arguments_summary, t.result_status))
        .collect::<Vec<_>>()
        .join("\n");
    let agent_text = result
        .agent_text
        .as_deref()
        .or(result.agent_text_preview.as_deref())
        .unwrap_or("(empty)");
    let pass_fail = if result.passed { "PASS" } else { "FAIL" };
    let reason = result
        .failure_reason
        .as_deref()
        .map(|r| format!(" ({r})"))
        .unwrap_or_default();
    format!(
        "User prompt: {}\n\nTool call trace:\n{}\n\nAgent output:\n{}\n\nStructural pass/fail: {pass_fail}{reason}",
        result.prompt,
        if trace_text.is_empty() { "  (no tool calls)".into() } else { trace_text },
        agent_text,
    )
}

fn parse_rubric(text: &str) -> Option<RubricScore> {
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str(cleaned).ok()
}

#[tokio::test]
#[ignore = "requires live LLM provider + benchmark results; run with: cargo test -p app-server-core --test rubric_tests -- --ignored --nocapture"]
async fn run_rubric_evaluation() {
    let results = load_results();
    assert!(
        !results.is_empty(),
        "no benchmark result files in benchmarks/results/. Run benchmarks first."
    );

    let config = load_rig_agent_config_with_discovery()
        .await
        .expect("load agent config")
        .expect("agent config present");
    eprintln!(
        "[rubric] provider={} kind={:?} model={}",
        config.provider_id, config.provider_kind, config.model
    );

    let mut scores = Vec::new();
    for (path, result) in &results {
        eprintln!("\n  Evaluating {} ({})...", result.scenario_id, result.scenario_name);
        let eval_prompt = build_eval_prompt(result);
        let response = run_rig_completion(config.clone(), RUBRIC_SYSTEM_PROMPT, &eval_prompt)
            .await
            .expect("rubric LLM call");

        let score = parse_rubric(&response);
        if let Some(ref s) = score {
            let total = s.intent_achieved + s.tool_selection + s.honest_limitations
                + s.no_hallucination + s.no_wasted_work;
            eprintln!("    [{total}/10] notes: {}", s.notes);

            let rubric_path = path
                .to_string_lossy()
                .replace("-result.json", "-rubric.json");
            let output = serde_json::json!({
                "scenario_id": result.scenario_id,
                "intent_achieved": s.intent_achieved,
                "tool_selection": s.tool_selection,
                "honest_limitations": s.honest_limitations,
                "no_hallucination": s.no_hallucination,
                "no_wasted_work": s.no_wasted_work,
                "notes": s.notes,
            });
            std::fs::write(&rubric_path, serde_json::to_string_pretty(&output).unwrap()).ok();
        } else {
            eprintln!("    [ERR] failed to parse rubric response");
            eprintln!("    raw: {response}");
        }
        scores.push((result.scenario_id.clone(), result.scenario_name.clone(), score));
    }

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  RUBRIC EVALUATION RESULTS");
    eprintln!("{}", "=".repeat(60));
    for (id, name, score) in &scores {
        if let Some(s) = score {
            let total = s.intent_achieved + s.tool_selection + s.honest_limitations
                + s.no_hallucination + s.no_wasted_work;
            eprintln!("  [{total}/10] {id} — {name}");
        } else {
            eprintln!("  [ERR] {id} — {name}");
        }
    }
    eprintln!("{}", "=".repeat(60));
}
