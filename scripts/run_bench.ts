// Agent benchmark orchestration.
// Usage:
//   bun run bench                  # run all benchmark scenarios
//   bun run bench --scenario b01   # run a single scenario by id prefix
//
// Requires:
//   - Live LLM provider configured in agents.toml
//   - CadQuery Python runner (CADQUERY_RUNNER_PYTHON or python3 on PATH)
//
// Results are written to benchmarks/results/*.json

import { $ } from "bun";
import path from "node:path";
import fs from "node:fs";

const REPO_ROOT = path.resolve(import.meta.dirname, "..");

function parseArgs(argv: string[]): { scenario?: string } {
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--scenario" && i + 1 < argv.length) {
      return { scenario: argv[i + 1] };
    }
  }
  return {};
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  const scenariosDir = path.join(REPO_ROOT, "benchmarks/scenarios");
  if (!fs.existsSync(scenariosDir)) {
    console.error(`No scenarios directory found at ${scenariosDir}`);
    process.exit(1);
  }

  const scenarioFiles = fs
    .readdirSync(scenariosDir)
    .filter((f) => f.endsWith(".json"))
    .sort();

  if (scenarioFiles.length === 0) {
    console.error("No scenario JSON files found");
    process.exit(1);
  }

  console.log(`Found ${scenarioFiles.length} benchmark scenario(s)`);

  if (args.scenario) {
    const matching = scenarioFiles.filter((f) =>
      f.startsWith(args.scenario!)
    );
    if (matching.length === 0) {
      console.error(`No scenarios matching prefix: ${args.scenario}`);
      console.error(`Available: ${scenarioFiles.join(", ")}`);
      process.exit(1);
    }
    console.log(`Filtering to ${matching.length} scenario(s) matching '${args.scenario}'`);
  }

  const resultsDir = path.join(REPO_ROOT, "benchmarks/results");
  fs.mkdirSync(resultsDir, { recursive: true });

  let testFilter = "";
  if (args.scenario) {
    testFilter = "run_all_benchmarks";
  }

  console.log("\nRunning cargo test (benchmark harness)...\n");

  const env: Record<string, string> = {
    ...process.env as Record<string, string>,
  };
  if (args.scenario) {
    env.BENCH_SCENARIO_FILTER = args.scenario;
  }

  const result =
    await $`cargo test -p app-server-core --test benchmark_tests -- --ignored --nocapture ${testFilter}`
      .cwd(REPO_ROOT)
      .env(env)
      .nothrow();

  if (result.exitCode !== 0) {
    console.error(`\nBenchmark harness exited with code ${result.exitCode}`);
  }

  const resultFiles = fs
    .readdirSync(resultsDir)
    .filter((f) => f.endsWith("-result.json"))
    .sort();

  if (resultFiles.length > 0) {
    console.log(`\n${"=".repeat(60)}`);
    console.log("  RESULT FILES");
    console.log("=".repeat(60));
    for (const f of resultFiles) {
      const data = JSON.parse(
        fs.readFileSync(path.join(resultsDir, f), "utf-8")
      );
      const status = data.passed ? "PASS" : "FAIL";
      console.log(`  [${status}] ${data.scenario_id} — ${data.scenario_name}`);
      if (data.failure_reason) {
        console.log(`         reason: ${data.failure_reason}`);
      }
    }
    console.log("=".repeat(60));
  }

  process.exit(result.exitCode);
}

main();
