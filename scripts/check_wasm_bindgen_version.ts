import { readFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dir, "..");
const EXPECTED = "0.2.117";
const CARGO_MANIFESTS = [
  path.join(root, "crates", "studio-web-wasm", "Cargo.toml"),
  path.join(root, "crates", "app-server-protocol-wasm", "Cargo.toml"),
];

function extractCargoVersion(cargoPath: string): string | null {
  const cargoText = readFileSync(cargoPath, "utf8");
  const match = cargoText.match(/wasm-bindgen\s*=\s*"=?([0-9]+\.[0-9]+\.[0-9]+)"/);
  return match ? match[1] : null;
}

async function extractCliVersion(): Promise<string | null> {
  const proc = Bun.spawn(["wasm-bindgen", "--version"], {
    stdout: "pipe",
    stderr: "pipe",
    stdin: "ignore",
  });
  const exit = await proc.exited;
  if (exit !== 0) {
    return null;
  }
  const text = await new Response(proc.stdout).text();
  const match = text.match(/wasm-bindgen\s+([0-9]+\.[0-9]+\.[0-9]+)/);
  return match ? match[1] : null;
}

async function main() {
  const cargoVersions = CARGO_MANIFESTS.map((cargoPath) => ({
    cargoPath,
    version: extractCargoVersion(cargoPath),
  }));
  const missing = cargoVersions.find((item) => !item.version);
  if (missing) {
    console.error(`could not parse wasm-bindgen version from ${missing.cargoPath}`);
    process.exit(1);
  }

  const cliVersion = await extractCliVersion();
  if (!cliVersion) {
    console.error(
      [
        "wasm-bindgen CLI not found. Install the pinned version:",
        `  cargo install wasm-bindgen-cli --version ${EXPECTED} --locked`,
      ].join("\n"),
    );
    process.exit(1);
  }

  const mismatchedCargoVersions = cargoVersions.filter((item) => item.version !== EXPECTED);
  if (mismatchedCargoVersions.length > 0 || cliVersion !== EXPECTED) {
    console.error(
      [
        `wasm-bindgen version drift detected. Expected: ${EXPECTED}`,
        ...cargoVersions.map((item) => `  ${path.relative(root, item.cargoPath)}: ${item.version}`),
        `  CLI:        ${cliVersion}`,
        "Fix by installing the pinned CLI and updating Cargo.toml in lockstep.",
      ].join("\n"),
    );
    process.exit(1);
  }

  console.log(`[wasm-bindgen] version OK: ${EXPECTED}`);
}

await main();
