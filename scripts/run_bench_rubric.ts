// LLM rubric evaluation for agent benchmark results.
// Delegates to the Rust rubric_tests harness which shares the product's LLM
// calling code and agents.toml configuration.
//
// Usage:
//   bun scripts/run_bench_rubric.ts                 # evaluate all results
//   bun scripts/run_bench_rubric.ts --scenario b01  # evaluate a single result
//
// Prerequisites:
//   - agents.toml with a valid LLM provider (BUDN_AGENT_CONFIG env or discovery)
//   - Benchmark results in benchmarks/results/*.json (run `bun run bench` first)
//
// No agents.toml content, secrets, or provider config is read or output by this script.

import { spawnSync } from "node:child_process";
import path from "node:path";

const REPO_ROOT = path.resolve(import.meta.dirname, "..");

function parseArgs(argv: string[]): { scenario?: string } {
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--scenario" && i + 1 < argv.length) {
      return { scenario: argv[i + 1] };
    }
  }
  return {};
}

const args = parseArgs(process.argv.slice(2));
const env: Record<string, string> = { ...process.env as Record<string, string> };
if (args.scenario) {
  env.BENCH_SCENARIO_FILTER = args.scenario;
}

console.log("Running rubric evaluation via cargo test...\n");

const result = spawnSync(
  "cargo",
  [
    "test",
    "-p",
    "app-server-core",
    "--test",
    "rubric_tests",
    "--",
    "--ignored",
    "--nocapture",
  ],
  {
    cwd: REPO_ROOT,
    stdio: "inherit",
    env,
  },
);

process.exit(result.status ?? 1);
