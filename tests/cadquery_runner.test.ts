import { expect, test } from "bun:test";
import { cp, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const repoRoot = resolve(import.meta.dir, "..");
const python = Bun.env.CADQUERY_RUNNER_PYTHON ?? "python3.11";

type RunnerResult = {
  code: number | null;
  stdout: string;
  stderr: string;
  json: any;
};

async function runRunner(projectRoot: string, script: string): Promise<RunnerResult> {
  const outputDir = await mkdtemp(join(tmpdir(), "budn-cq-output-"));
  const env = {
    ...Bun.env,
    PYTHONPATH: [repoRoot, Bun.env.PYTHONPATH].filter(Boolean).join(":"),
    PYTHONDONTWRITEBYTECODE: "1",
  };
  const proc = Bun.spawn(
    [
      python,
      "-m",
      "budn_cad_runner",
      "--script",
      script,
      "--project-root",
      projectRoot,
      "--output-dir",
      outputDir,
      "--exports",
      "",
    ],
    { cwd: repoRoot, env, stdout: "pipe", stderr: "pipe" },
  );
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  await rm(outputDir, { force: true, recursive: true });
  let json: any;
  try {
    json = JSON.parse(stdout);
  } catch (error) {
    throw new Error(`runner did not emit JSON; code=${code}; stderr=${stderr}`);
  }
  return { code, stdout, stderr, json };
}

test("cadquery runner emits a single part mesh for a Workplane build", async () => {
  const projectRoot = join(repoRoot, "tests/fixtures/cadquery-runner/simple");
  const result = await runRunner(projectRoot, "parts/top_lid.py");

  expect(result.code).toBe(0);
  expect(result.json.status).toBe("success");
  expect(result.json.unit).toBe("millimeter");
  expect(result.json.root_ref_text).toBe("@part[top_lid]");
  expect(result.json.root_object_kind).toBe("part");
  expect(result.json.parts).toHaveLength(1);
  expect(result.json.parts[0].mesh.faces.length).toBeGreaterThan(0);
  expect(result.json.metadata.bounding_box).toEqual({
    min: [-40, -30, -4],
    max: [40, 30, 4],
  });
});

test("cadquery runner emits assembly parts with instance paths and metadata refs", async () => {
  const projectRoot = join(repoRoot, "tests/fixtures/cadquery-runner/assembly");
  const result = await runRunner(projectRoot, "assemblies/full_enclosure.py");

  expect(result.code).toBe(0);
  expect(result.json.status).toBe("success");
  expect(result.json.root_ref_text).toBe("@assembly[full_enclosure]");
  expect(result.json.root_object_kind).toBe("assembly");
  expect(result.json.parts.map((part: any) => part.instance_path)).toEqual([
    "full_enclosure/bottom_case",
    "full_enclosure/top_lid",
    "full_enclosure/pcb_main",
  ]);
  expect(result.json.parts.map((part: any) => part.ref_text)).toEqual([
    "@part[bottom_case]",
    "@part[top_lid]",
    "@component[pcb_main]",
  ]);
  expect(result.json.parts[2].transform).toHaveLength(16);
  expect(result.json.parts[2].object_kind).toBe("component");
});

test("cadquery runner changes deps_hash and build_id when an imported file changes", async () => {
  const sourceRoot = join(repoRoot, "tests/fixtures/cadquery-runner/imported");
  const projectRoot = await mkdtemp(join(tmpdir(), "budn-cq-project-"));
  await cp(sourceRoot, projectRoot, { recursive: true });

  try {
    const first = await runRunner(projectRoot, "parts/top_lid.py");
    await writeFile(
      join(projectRoot, "components/dimensions.py"),
      "WIDTH = 96\nLENGTH = 60\nHEIGHT = 8\n",
      "utf8",
    );
    const second = await runRunner(projectRoot, "parts/top_lid.py");

    expect(first.code).toBe(0);
    expect(second.code).toBe(0);
    expect(first.json.manifest.dependencies.map((dep: any) => dep.path)).toContain(
      "components/dimensions.py",
    );
    expect(second.json.manifest.deps_hash).not.toBe(first.json.manifest.deps_hash);
    expect(second.json.build_id).not.toBe(first.json.build_id);
  } finally {
    await rm(projectRoot, { force: true, recursive: true });
  }
});

test("cadquery runner reports build exceptions as build_error", async () => {
  const projectRoot = join(repoRoot, "tests/fixtures/cadquery-runner/build-error");
  const result = await runRunner(projectRoot, "parts/broken.py");

  expect(result.code).toBe(1);
  expect(result.json.status).toBe("build_error");
  expect(result.json.error_type).toBe("ValueError");
  expect(result.json.error).toContain("intentional build failure");
  expect(result.stderr).toContain("ValueError");
});
