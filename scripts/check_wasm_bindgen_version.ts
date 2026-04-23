import { readFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dir, "..");
const EXPECTED = "0.2.117";

function extractCargoVersion(): string | null {
  const cargoPath = path.join(root, "crates", "studio-web-wasm", "Cargo.toml");
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
  const cargoVersion = extractCargoVersion();
  if (!cargoVersion) {
    console.error(
      "could not parse wasm-bindgen version from crates/studio-web-wasm/Cargo.toml",
    );
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

  if (cargoVersion !== EXPECTED || cliVersion !== EXPECTED) {
    console.error(
      [
        `wasm-bindgen version drift detected. Expected: ${EXPECTED}`,
        `  Cargo.toml: ${cargoVersion}`,
        `  CLI:        ${cliVersion}`,
        "Fix by installing the pinned CLI and updating Cargo.toml in lockstep.",
      ].join("\n"),
    );
    process.exit(1);
  }

  console.log(`[wasm-bindgen] version OK: ${EXPECTED}`);
}

await main();
