use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_server_core::{
    AgentTurnInput, AgentToolCall, AgentToolObserver, AgentToolRunContext,
    CadQueryContractConfig, CadQueryModelContract, CadQueryRunConfig,
    CadQueryToolCachedResult, CadQueryToolRunRequest, CadQueryToolRunResult,
    CadQueryToolRuntime, CadQueryToolRuntimeError, HostedToolRequest,
    RigAgentCallbacks, ToolExecutor, WorkspaceToolExecutor,
    run_cadquery_contract, run_cadquery_runner,
    stage_cadquery_project_owned, build_turn_context, run_rig_agent_turn_with_config,
};
use app_server_core::llm::load_rig_agent_config_with_discovery;
use app_server_protocol::AgentMode;

const RUNNER_TIMEOUT: Duration = Duration::from_secs(60);

fn cadquery_python_path() -> PathBuf {
    std::env::var_os("CADQUERY_RUNNER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("python3"))
}

#[derive(Debug, Clone)]
struct BenchToolTrace {
    tool_name: String,
    arguments_summary: String,
    result_status: String,
}

struct TracingObserver {
    traces: Mutex<Vec<BenchToolTrace>>,
}

impl TracingObserver {
    fn new() -> Self {
        Self {
            traces: Mutex::new(Vec::new()),
        }
    }

    fn traces(&self) -> Vec<BenchToolTrace> {
        self.traces.lock().unwrap().clone()
    }
}

impl AgentToolObserver for TracingObserver {
    fn tool_start(&self, call: &AgentToolCall) {
        eprintln!("[bench:tool_start] {}", call.function_name);
        self.traces.lock().unwrap().push(BenchToolTrace {
            tool_name: call.function_name.clone(),
            arguments_summary: truncate(&call.arguments, 200),
            result_status: "pending".into(),
        });
    }

    fn tool_result(&self, call: &AgentToolCall, result: &str) {
        let status = extract_status(result);
        eprintln!(
            "[bench:tool_result] {} => {}",
            call.function_name, status
        );
        let mut traces = self.traces.lock().unwrap();
        if let Some(trace) = traces
            .iter_mut()
            .rev()
            .find(|t| t.tool_name == call.function_name && t.result_status == "pending")
        {
            trace.result_status = status;
        }
    }
}

fn extract_status(result: &str) -> String {
    serde_json::from_str::<serde_json::Value>(result)
        .ok()
        .and_then(|v| v.get("status")?.as_str().map(|s| s.to_owned()))
        .unwrap_or_else(|| "unknown".into())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}...", &s[..max])
    }
}

struct BenchmarkCadQueryRuntime {
    workspace_root: PathBuf,
    python: PathBuf,
}

impl BenchmarkCadQueryRuntime {
    fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            python: cadquery_python_path(),
        }
    }

    fn runner_warnings(stderr: &str) -> Vec<String> {
        stderr
            .lines()
            .filter(|l| l.contains("WARNING") || l.contains("UserWarning"))
            .map(|l| l.to_owned())
            .collect()
    }
}

#[async_trait::async_trait]
impl CadQueryToolRuntime for BenchmarkCadQueryRuntime {
    async fn model_contract(
        &self,
        request: &CadQueryToolRunRequest,
    ) -> Option<Result<CadQueryModelContract, CadQueryToolRuntimeError>> {
        let result = run_cadquery_contract(CadQueryContractConfig {
            python: self.python.clone(),
            code: request.code.clone(),
            timeout: RUNNER_TIMEOUT,
        })
        .await
        .map(|c| CadQueryModelContract {
            has_model_description: c.has_model_description,
        })
        .map_err(|e| {
            CadQueryToolRuntimeError::new("cadquery_contract_error", e.to_string(), false)
        });
        Some(result)
    }

    async fn dry_run(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        let staged = stage_cadquery_project_owned(
            self.workspace_root.clone(),
            PathBuf::from(&request.target_path),
            request.code.clone(),
        )
        .await
        .map_err(|e| {
            CadQueryToolRuntimeError::new("cadquery_staging_error", e.to_string(), true)
        })?;

        let result = run_cadquery_runner(CadQueryRunConfig {
            python: self.python.clone(),
            project_root: staged.root().to_path_buf(),
            script: staged.script_arg(),
            output_dir: staged.output_dir(),
            export_formats: Vec::new(),
            params_json: request.params_json,
            timeout: RUNNER_TIMEOUT,
        })
        .await;

        staged.cleanup().await;

        let run_result = result.map_err(|e| {
            CadQueryToolRuntimeError::new("cadquery_runner_error", e.to_string(), true)
        })?;

        let warnings = Self::runner_warnings(&run_result.stderr);
        Ok(CadQueryToolRunResult {
            mesh: run_result.mesh,
            committed_files: Vec::new(),
            exports: Vec::new(),
            warnings,
        })
    }

    async fn execute(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        let staged = stage_cadquery_project_owned(
            self.workspace_root.clone(),
            PathBuf::from(&request.target_path),
            request.code.clone(),
        )
        .await
        .map_err(|e| {
            CadQueryToolRuntimeError::new("cadquery_staging_error", e.to_string(), true)
        })?;

        let result = run_cadquery_runner(CadQueryRunConfig {
            python: self.python.clone(),
            project_root: staged.root().to_path_buf(),
            script: staged.script_arg(),
            output_dir: staged.output_dir(),
            export_formats: request.export_formats,
            params_json: request.params_json,
            timeout: RUNNER_TIMEOUT,
        })
        .await;

        let run_result = match result {
            Ok(r) => r,
            Err(e) => {
                staged.cleanup().await;
                return Err(CadQueryToolRuntimeError::new(
                    "cadquery_runner_error",
                    e.to_string(),
                    true,
                ));
            }
        };

        if let Err(e) = staged.commit_success().await {
            eprintln!("[bench] commit failed: {e}");
        }

        let warnings = Self::runner_warnings(&run_result.stderr);
        Ok(CadQueryToolRunResult {
            mesh: run_result.mesh,
            committed_files: vec![request.target_path],
            exports: request.export_targets,
            warnings,
        })
    }

    fn get_result(&self, _result_id: &str) -> Option<CadQueryToolCachedResult> {
        None
    }
}

#[derive(Debug, serde::Deserialize)]
struct BenchmarkScenario {
    id: String,
    name: String,
    prompt: String,
    criteria: BenchmarkCriteria,
}

#[derive(Debug, serde::Deserialize)]
struct BenchmarkCriteria {
    cadquery_execute_success: bool,
    brief_expected: bool,
    exports_generated: bool,
    max_dry_run_attempts: usize,
}

struct BenchmarkResult {
    scenario: BenchmarkScenario,
    passed: bool,
    agent_text: String,
    traces: Vec<BenchToolTrace>,
    failure_reason: Option<String>,
}

fn load_scenarios() -> Vec<BenchmarkScenario> {
    let scenarios_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("benchmarks/scenarios");
    let filter = std::env::var("BENCH_SCENARIO_FILTER").ok();
    let mut scenarios = Vec::new();
    for entry in std::fs::read_dir(&scenarios_dir).expect("benchmarks/scenarios dir") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            if let Some(ref prefix) = filter {
                let name = path.file_name().unwrap().to_string_lossy();
                if !name.starts_with(prefix.as_str()) {
                    continue;
                }
            }
            let content = std::fs::read_to_string(&path).unwrap();
            let scenario: BenchmarkScenario =
                serde_json::from_str(&content).unwrap();
            scenarios.push(scenario);
        }
    }
    scenarios.sort_by(|a, b| a.id.cmp(&b.id));
    scenarios
}

fn build_bench_input(prompt: String) -> AgentTurnInput {
    AgentTurnInput {
        mode: AgentMode::Agent,
        prompt,
        history: vec![],
        selections: vec![],
        active_selection_index: None,
        plan_ref: None,
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: None,
    }
}

fn noop_callbacks() -> RigAgentCallbacks {
    RigAgentCallbacks {
        on_token: Arc::new(|_| true),
        on_reasoning: Arc::new(|_| true),
        on_hosted_tool_requested: Arc::new(|_: &HostedToolRequest| true),
        cancelled: Arc::new(|| false),
    }
}

async fn run_benchmark(scenario: &BenchmarkScenario) -> BenchmarkResult {
    let workspace = tempfile::tempdir().expect("temp workspace");
    let ws_root = workspace.path().to_path_buf();

    std::fs::create_dir_all(ws_root.join("parts")).unwrap();
    std::fs::create_dir_all(ws_root.join("outputs")).unwrap();

    let runtime = Arc::new(BenchmarkCadQueryRuntime::new(ws_root.clone()));
    let executor: Arc<dyn ToolExecutor> = Arc::new(
        WorkspaceToolExecutor::new(ws_root.clone())
            .with_cadquery_runtime(runtime),
    );
    let observer = TracingObserver::new();
    let input = build_bench_input(scenario.prompt.clone());

    let context_preview = build_turn_context(&input);
    eprintln!("\n=== Benchmark {} ({}) ===", scenario.id, scenario.name);
    eprintln!("[context length] {} chars", context_preview.len());

    let config = load_rig_agent_config_with_discovery()
        .await
        .expect("load agent config")
        .expect("agent config present");
    eprintln!(
        "[config] provider={} kind={:?} model={}",
        config.provider_id, config.provider_kind, config.model
    );

    let ctx = AgentToolRunContext::new(ws_root.clone(), AgentMode::Agent);
    let result = run_rig_agent_turn_with_config(input, config, executor, ctx, &observer, noop_callbacks()).await;
    let traces = observer.traces();

    match result {
        Ok(r) => evaluate(scenario, r.text, traces),
        Err(e) => BenchmarkResult {
            scenario: clone_scenario(scenario),
            passed: false,
            agent_text: String::new(),
            traces,
            failure_reason: Some(format!("Agent turn error: {}", e.message)),
        },
    }
}

fn evaluate(
    scenario: &BenchmarkScenario,
    text: String,
    traces: Vec<BenchToolTrace>,
) -> BenchmarkResult {
    let mut reasons = Vec::new();

    if scenario.criteria.cadquery_execute_success {
        let has_execute_success = traces
            .iter()
            .any(|t| t.tool_name == "cadquery_execute" && t.result_status == "success");
        if !has_execute_success {
            reasons.push("cadquery_execute did not succeed".into());
        }
    }

    if scenario.criteria.brief_expected {
        let has_brief = text.contains("**Brief**") || text.contains("brief");
        if !has_brief {
            reasons.push("no brief found in agent output".into());
        }
    }

    if scenario.criteria.exports_generated {
        let execute_produced_exports = traces.iter().any(|t| {
            t.tool_name == "cadquery_execute" && t.result_status == "success"
        });
        if !execute_produced_exports {
            reasons.push("exports_generated required but cadquery_execute did not succeed".into());
        }
    }

    let dry_run_count = traces
        .iter()
        .filter(|t| t.tool_name == "cadquery_dry_run")
        .count();
    if dry_run_count > scenario.criteria.max_dry_run_attempts {
        reasons.push(format!(
            "too many dry_run attempts: {} > {}",
            dry_run_count, scenario.criteria.max_dry_run_attempts
        ));
    }

    BenchmarkResult {
        scenario: clone_scenario(scenario),
        passed: reasons.is_empty(),
        agent_text: text,
        traces,
        failure_reason: if reasons.is_empty() {
            None
        } else {
            Some(reasons.join("; "))
        },
    }
}

fn clone_scenario(s: &BenchmarkScenario) -> BenchmarkScenario {
    BenchmarkScenario {
        id: s.id.clone(),
        name: s.name.clone(),
        prompt: s.prompt.clone(),
        criteria: BenchmarkCriteria {
            cadquery_execute_success: s.criteria.cadquery_execute_success,
            brief_expected: s.criteria.brief_expected,
            exports_generated: s.criteria.exports_generated,
            max_dry_run_attempts: s.criteria.max_dry_run_attempts,
        },
    }
}

fn print_results(results: &[BenchmarkResult]) {
    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  BENCHMARK RESULTS");
    eprintln!("{}", "=".repeat(60));
    let mut pass_count = 0;
    for r in results {
        let status = if r.passed {
            pass_count += 1;
            "PASS"
        } else {
            "FAIL"
        };
        eprintln!(
            "  [{status}] {} — {}",
            r.scenario.id, r.scenario.name
        );
        if let Some(reason) = &r.failure_reason {
            eprintln!("         reason: {reason}");
        }
        let tool_names: Vec<_> = r.traces.iter().map(|t| {
            format!("{}({})", t.tool_name, t.result_status)
        }).collect();
        eprintln!("         tools: {}", tool_names.join(" -> "));
    }
    eprintln!("{}", "=".repeat(60));
    eprintln!(
        "  {pass_count}/{} passed",
        results.len()
    );
}

#[tokio::test]
#[ignore = "requires live LLM provider + CadQuery Python; run with: cargo test -p app-server-core --test benchmark_tests -- --ignored --nocapture"]
async fn run_all_benchmarks() {
    let scenarios = load_scenarios();
    assert!(
        !scenarios.is_empty(),
        "no benchmark scenarios found in benchmarks/scenarios/"
    );

    let mut results = Vec::new();
    for scenario in &scenarios {
        let result = run_benchmark(scenario).await;
        results.push(result);
    }

    print_results(&results);

    let output_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("benchmarks/results");
    std::fs::create_dir_all(&output_dir).ok();

    for r in &results {
        let filename = format!("{}-result.json", r.scenario.id);
        let trace_data: Vec<_> = r.traces.iter().map(|t| {
            serde_json::json!({
                "tool": t.tool_name,
                "arguments_summary": t.arguments_summary,
                "result_status": t.result_status,
            })
        }).collect();

        let output = serde_json::json!({
            "scenario_id": r.scenario.id,
            "scenario_name": r.scenario.name,
            "prompt": r.scenario.prompt,
            "passed": r.passed,
            "failure_reason": r.failure_reason,
            "agent_text": r.agent_text,
            "agent_text_length": r.agent_text.len(),
            "agent_text_preview": truncate(&r.agent_text, 500),
            "tool_traces": trace_data,
        });

        let path = output_dir.join(&filename);
        std::fs::write(&path, serde_json::to_string_pretty(&output).unwrap()).ok();
    }
}
