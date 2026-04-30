import { expect, test } from "bun:test";
import { access, cp, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const repoRoot = resolve(import.meta.dir, "..");
const python = Bun.env.CADQUERY_RUNNER_PYTHON ?? "python3.11";
const RUNNER_TEST_TIMEOUT_MS = 20_000;

type RunnerResult = {
  code: number | null;
  stdout: string;
  stderr: string;
  json: any;
};

function translationOf(transform: number[]): number[] {
  return [transform[3], transform[7], transform[11]];
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function runRunner(
  projectRoot: string,
  script: string,
  exports = "",
  requestedOutputDir?: string,
): Promise<RunnerResult> {
  const outputDir = requestedOutputDir ?? (await mkdtemp(join(tmpdir(), "budn-cq-output-")));
  const env = {
    ...Bun.env,
    PYTHONPATH: [repoRoot, Bun.env.PYTHONPATH].filter(Boolean).join(":"),
    PYTHONDONTWRITEBYTECODE: "1",
  };
  const proc = Bun.spawn(
    [
      python,
      "-B",
      "-m",
      "budn_cad_runner",
      "--script",
      script,
      "--project-root",
      projectRoot,
      "--output-dir",
      outputDir,
      "--exports",
      exports,
    ],
    { cwd: repoRoot, env, stdout: "pipe", stderr: "pipe" },
  );
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (!requestedOutputDir) {
    await rm(outputDir, { force: true, recursive: true });
  }
  let json: any;
  try {
    json = JSON.parse(stdout);
  } catch (error) {
    throw new Error(`runner did not emit JSON; code=${code}; stderr=${stderr}`);
  }
  return { code, stdout, stderr, json };
}

async function runContract(code: string): Promise<RunnerResult> {
  const contractRoot = await mkdtemp(join(tmpdir(), "budn-cq-contract-"));
  const sourcePath = join(contractRoot, "source.py");
  await writeFile(sourcePath, code);
  const env = {
    ...Bun.env,
    PYTHONPATH: [repoRoot, Bun.env.PYTHONPATH].filter(Boolean).join(":"),
    PYTHONDONTWRITEBYTECODE: "1",
  };
  const proc = Bun.spawn(
    [python, "-B", "-m", "budn_cad_runner", "--contract-file", sourcePath],
    { cwd: repoRoot, env, stdout: "pipe", stderr: "pipe" },
  );
  const [stdout, stderr, codeResult] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  await rm(contractRoot, { force: true, recursive: true });
  let json: any;
  try {
    json = JSON.parse(stdout);
  } catch {
    throw new Error(`contract runner did not emit JSON; code=${codeResult}; stderr=${stderr}`);
  }
  return { code: codeResult, stdout, stderr, json };
}

test("cadquery runner contract analyzer uses Python AST string semantics", async () => {
  const result = await runContract(`
MODEL_DESCRIPTION = (
    "Contract "
    "model"
)

MODEL_DETAILS = {
    "purpose": (
        "Verify "
        "contract"
    ),
    "key_dimensions": {"height": 8.0},
    "intended_use": "automated validation",
    "assumptions": ["no external dependencies"],
    "interaction_notes": "select named features",
    "manufacturing_or_placement_constraints": "print flat",
}
`);

  expect(result.code).toBe(0);
  expect(result.json.status).toBe("success");
  expect(result.json.contract.has_model_description).toBe(true);
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner contract analyzer rejects non-literal model descriptions", async () => {
  const result = await runContract(`
MODEL_DESCRIPTION = "Tuple contract model", dynamic_description()

MODEL_DETAILS = {
    "purpose": "Verify contract",
    "key_dimensions": "unit dimensions",
    "intended_use": "automated validation",
    "assumptions": "no external dependencies",
    "interaction_notes": "select named features",
    "manufacturing_or_placement_constraints": "print flat",
}
`);

  expect(result.code).toBe(0);
  expect(result.json.status).toBe("success");
  expect(result.json.contract.has_model_description).toBe(false);
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner contract analyzer rejects non-top-level or incomplete model contracts", async () => {
  const validDetails = `{
    "purpose": "Verify contract",
    "key_dimensions": "unit dimensions",
    "intended_use": "automated validation",
    "assumptions": "no external dependencies",
    "interaction_notes": "select named features",
    "manufacturing_or_placement_constraints": "print flat",
}`;
  const cases = [
    {
      name: "docstring text",
      code: `"""MODEL_DESCRIPTION = "text"\nMODEL_DETAILS = ${validDetails}\n"""`,
    },
    {
      name: "function scoped",
      code: `MODEL_DESCRIPTION = "Contract model"\ndef details():\n    MODEL_DETAILS = ${validDetails}`,
    },
    {
      name: "empty field",
      code: `MODEL_DESCRIPTION = "Contract model"\nMODEL_DETAILS = {
    "purpose": "",
    "key_dimensions": "unit dimensions",
    "intended_use": "automated validation",
    "assumptions": "no external dependencies",
    "interaction_notes": "select named features",
    "manufacturing_or_placement_constraints": "print flat",
}`,
    },
    {
      name: "parenthesized expression",
      code: `MODEL_DESCRIPTION = ("Contract model") + dynamic_description()\nMODEL_DETAILS = ${validDetails}`,
    },
    {
      name: "final reassignment wins",
      code: `MODEL_DESCRIPTION = "Contract model"\nMODEL_DESCRIPTION = ""\nMODEL_DETAILS = ${validDetails}`,
    },
  ];

  for (const item of cases) {
    const result = await runContract(item.code);
    expect(result.code, item.name).toBe(0);
    expect(result.json.status, item.name).toBe("success");
    expect(result.json.contract.has_model_description, item.name).toBe(false);
  }
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner contract analyzer keeps values across bare annotations", async () => {
  const result = await runContract(`
MODEL_DESCRIPTION = "Contract model"
MODEL_DESCRIPTION: str

MODEL_DETAILS = {
    "purpose": "Verify contract",
    "key_dimensions": "unit dimensions",
    "intended_use": "automated validation",
    "assumptions": "no external dependencies",
    "interaction_notes": "select named features",
    "manufacturing_or_placement_constraints": "print flat",
}
`);

  expect(result.code).toBe(0);
  expect(result.json.status).toBe("success");
  expect(result.json.contract.has_model_description).toBe(true);
}, RUNNER_TEST_TIMEOUT_MS);

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
  expect(result.json.parts[0].mesh.edges.length).toBeGreaterThan(0);
  expect(result.json.parts[0].mesh.vertices.length).toBeGreaterThan(0);
  expect(result.json.parts[0].mesh.edges[0].polyline.length).toBeGreaterThanOrEqual(6);
  expect(result.json.parts[0].mesh.vertices[0].position).toHaveLength(3);
  expect(result.json.parts[0].feature_map.top_surface.face_indices.length).toBeGreaterThan(0);
  expect(result.json.metadata.bounding_box).toEqual({
    min: [-40, -30, -4],
    max: [40, 30, 4],
  });
}, RUNNER_TEST_TIMEOUT_MS);

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
  expect(result.json.parts[0].feature_map.floor.face_indices.length).toBeGreaterThan(0);
  expect(result.json.parts[1].feature_map.top_surface.face_indices.length).toBeGreaterThan(0);
  expect(result.json.parts[2].feature_map.board_body.face_indices.length).toBeGreaterThan(0);
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner flattens nested assembly parts with full instance paths", async () => {
  const projectRoot = await mkdtemp(join(tmpdir(), "budn-cq-nested-assembly-"));
  try {
    await mkdir(join(projectRoot, "assemblies"), { recursive: true });
    await writeFile(
      join(projectRoot, "assemblies/nested.py"),
      [
        "import cadquery as cq",
        'REFS = {"assembly": "root_assembly"}',
        "def build(params=None):",
        '    root = cq.Assembly(name="root_assembly")',
        '    module = cq.Assembly(name="module")',
        '    module.add(cq.Workplane("XY").box(1, 1, 1), name="inner_a", metadata={"ref_text": "@part[inner_a]", "object_kind": "part"})',
        '    module.add(cq.Workplane("XY").box(2, 1, 1), name="inner_b", loc=cq.Location(cq.Vector(3, 0, 0)), metadata={"ref_text": "@part[inner_b]", "object_kind": "part"})',
        '    root.add(module, name="module", loc=cq.Location(cq.Vector(0, 2, 0)), metadata={"ref_text": "@assembly[module]", "object_kind": "assembly"})',
        "    return root",
        "",
      ].join("\n"),
      "utf8",
    );

    const result = await runRunner(projectRoot, "assemblies/nested.py");

    expect(result.code).toBe(0);
    expect(result.json.root_ref_text).toBe("@assembly[root_assembly]");
    expect(result.json.parts.map((part: any) => part.instance_path)).toEqual([
      "root_assembly/module/inner_a",
      "root_assembly/module/inner_b",
    ]);
    expect(result.json.parts.map((part: any) => part.ref_text)).toEqual([
      "@part[inner_a]",
      "@part[inner_b]",
    ]);
    expect(result.json.parts.map((part: any) => translationOf(part.transform))).toEqual([
      [0, 2, 0],
      [3, 2, 0],
    ]);
  } finally {
    await rm(projectRoot, { force: true, recursive: true });
  }
}, RUNNER_TEST_TIMEOUT_MS);

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
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner tracks from-import module aliases as dependencies", async () => {
  const projectRoot = await mkdtemp(join(tmpdir(), "budn-cq-from-import-"));
  try {
    await mkdir(join(projectRoot, "parts"), { recursive: true });
    await mkdir(join(projectRoot, "components"), { recursive: true });
    await writeFile(
      join(projectRoot, "components/dimensions.py"),
      "WIDTH = 80\nLENGTH = 60\nHEIGHT = 8\n",
      "utf8",
    );
    await writeFile(
      join(projectRoot, "parts/top_lid.py"),
      [
        "import cadquery as cq",
        "from components import dimensions",
        'REFS = {"part": "top_lid"}',
        "def build(params=None):",
        '    return cq.Workplane("XY").box(dimensions.WIDTH, dimensions.LENGTH, dimensions.HEIGHT)',
        "",
      ].join("\n"),
      "utf8",
    );

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
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner reports build exceptions as build_error", async () => {
  const projectRoot = join(repoRoot, "tests/fixtures/cadquery-runner/build-error");
  const result = await runRunner(projectRoot, "parts/broken.py");

  expect(result.code).toBe(1);
  expect(result.json.status).toBe("build_error");
  expect(result.json.error_type).toBe("ValueError");
  expect(result.json.error).toContain("intentional build failure");
  expect(result.stderr).toContain("ValueError");
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner exports requested artifacts and hashes them", async () => {
  const sourceRoot = join(repoRoot, "tests/fixtures/cadquery-runner/simple");
  const projectRoot = await mkdtemp(join(tmpdir(), "budn-cq-export-project-"));
  await cp(sourceRoot, projectRoot, { recursive: true });

  try {
    const result = await runRunner(
      projectRoot,
      "parts/top_lid.py",
      "step,stl",
      join(projectRoot, "outputs"),
    );

    expect(result.code).toBe(0);
    expect(Object.keys(result.json.exports).sort()).toEqual(["step", "stl"]);
    expect(result.json.exports.step).toBe("outputs/top_lid.step");
    expect(result.json.exports.stl).toBe("outputs/top_lid.stl");
    expect(result.json.manifest.export_hashes.step).toStartWith("sha256:");
    expect(result.json.manifest.export_hashes.stl).toStartWith("sha256:");
  } finally {
    await rm(projectRoot, { force: true, recursive: true });
  }
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner rejects invalid selector strings without eval", async () => {
  const projectRoot = await mkdtemp(join(tmpdir(), "budn-cq-invalid-selector-"));
  try {
    await writeFile(
      join(projectRoot, "bad_selector.py"),
      [
        "import cadquery as cq",
        'REFS = {"part": "bad", "features": {"bad": {"selector": "__import__(\\"os\\")"}}}',
        "def build(params=None):",
        '    return cq.Workplane("XY").box(1, 1, 1)',
        "",
      ].join("\n"),
      "utf8",
    );
    const result = await runRunner(projectRoot, "bad_selector.py");

    expect(result.code).toBe(2);
    expect(result.json.status).toBe("runner_error");
    expect(result.json.error).toContain("invalid selector");
  } finally {
    await rm(projectRoot, { force: true, recursive: true });
  }
}, RUNNER_TEST_TIMEOUT_MS);

test("cadquery runner rejects export paths outside project root", async () => {
  const projectRoot = join(repoRoot, "tests/fixtures/cadquery-runner/simple");
  const outputDir = await mkdtemp(join(tmpdir(), "budn-cq-external-output-"));
  try {
    const result = await runRunner(projectRoot, "parts/top_lid.py", "step", outputDir);

    expect(result.code).toBe(2);
    expect(result.json.status).toBe("runner_error");
    expect(result.json.error).toContain("project root");
    expect(await pathExists(join(outputDir, "top_lid.step"))).toBe(false);
  } finally {
    await rm(outputDir, { force: true, recursive: true });
  }
}, RUNNER_TEST_TIMEOUT_MS);
