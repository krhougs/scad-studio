import { expect, test, type Page } from "@playwright/test";
import { createHarness } from "./_smoke-harness";

const HARNESS = createHarness({
  bindPort: 39193,
  vitePort: 5188,
});

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
});

test("@cadquery-viewer face picking emits selection target in browser", async ({
  page,
}) => {
  await installViewer(page, cadQueryTriangleScene());

  const canvas = page.locator("#cadquery-canvas");
  await canvas.click({ position: { x: 320, y: 240 } });

  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "face", partIndex: 0, faceIndex: 0 });
});

test("@cadquery-viewer picks edge vertex modes and ignores root object modes", async ({
  page,
}) => {
  await installViewer(page, cadQueryTriangleScene());
  const canvas = page.locator("#cadquery-canvas");

  await setSelectionMode(page, "edge");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "edge", partIndex: 0, edgeIndex: 0 });

  await setSelectionMode(page, "vertex");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "vertex", partIndex: 0, vertexIndex: 0 });

  await setSelectionMode(page, "part");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toBeUndefined();

  await setSelectionMode(page, "assembly");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toBeUndefined();
});

test("@cadquery-viewer picks component instance and feature modes", async ({
  page,
}) => {
  await installViewer(page, cadQueryComponentInstanceScene());
  const canvas = page.locator("#cadquery-canvas");

  await setSelectionMode(page, "component");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "component", partIndex: 0 });

  await setSelectionMode(page, "instance");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "instance", partIndex: 0 });

  await setSelectionMode(page, "feature");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({
      kind: "feature",
      partIndex: 0,
      feature: "lid_alignment_surface",
    });
});

test("@cadquery-viewer ignores feature mode without feature map ownership", async ({
  page,
}) => {
  await installViewer(page, cadQueryUnmappedFeatureScene());
  const canvas = page.locator("#cadquery-canvas");

  await setSelectionMode(page, "feature");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toBeUndefined();
});

test("@cadquery-viewer picks non-root part mode", async ({ page }) => {
  await installViewer(page, cadQueryRepeatedInstanceScene());
  const canvas = page.locator("#cadquery-canvas");

  await setSelectionMode(page, "part");
  await canvas.click({ position: { x: 130, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "part", partIndex: 0 });
});

test("@cadquery-viewer picks nested assembly refs instead of only root", async ({
  page,
}) => {
  await installViewer(page, cadQueryNestedAssemblyScene());
  const canvas = page.locator("#cadquery-canvas");

  await setSelectionMode(page, "assembly");
  await canvas.click({ position: { x: 320, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "assembly", partIndex: 0 });
});

test("@cadquery-viewer distinguishes repeated assembly instances", async ({
  page,
}) => {
  await installViewer(page, cadQueryRepeatedInstanceScene());
  const canvas = page.locator("#cadquery-canvas");

  await canvas.click({ position: { x: 130, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "face", partIndex: 0, faceIndex: 0 });

  await canvas.click({ position: { x: 510, y: 240 } });
  await expect
    .poll(() => page.evaluate(() => window.__cadQueryPick))
    .toMatchObject({ kind: "face", partIndex: 1, faceIndex: 0 });
});

test("@cadquery-viewer exposes hover highlight target", async ({ page }) => {
  await installViewer(page, cadQueryTriangleScene());
  const canvas = page.locator("#cadquery-canvas");

  await canvas.hover({ position: { x: 320, y: 240 } });

  await expect(canvas).toHaveAttribute("data-cad-query-hover-kind", "face");
  await expect(canvas).toHaveAttribute(
    "data-cad-query-hover-key",
    /face.*top_lid.*f_0/,
  );

  await page.mouse.move(700, 500);
  await expect(canvas).not.toHaveAttribute("data-cad-query-hover-kind");
  await expect(canvas).not.toHaveAttribute("data-cad-query-hover-key");

  await canvas.hover({ position: { x: 320, y: 240 } });
  await expect(canvas).toHaveAttribute("data-cad-query-hover-kind", "face");
  await setSelectionMode(page, "edge");
  await expect(canvas).not.toHaveAttribute("data-cad-query-hover-kind");
});

test("@cadquery-viewer render modes change canvas pixels", async ({ page }, testInfo) => {
  await installViewer(page, cadQueryTriangleScene());
  const canvas = page.locator("#cadquery-canvas");

  await expect(canvas).toHaveAttribute("data-render-mode", "solid");
  const solidImage = await canvas.screenshot({
    path: testInfo.outputPath("cadquery-render-mode-solid.png"),
  });

  await page.evaluate(() => {
    window.__cadQueryViewer?.setOptions({ renderMode: "wireframe" });
  });
  await expect(canvas).toHaveAttribute("data-render-mode", "wireframe");
  const wireframeImage = await canvas.screenshot({
    path: testInfo.outputPath("cadquery-render-mode-wireframe.png"),
  });

  await page.evaluate(() => {
    window.__cadQueryViewer?.setOptions({ renderMode: "xray" });
  });
  await expect(canvas).toHaveAttribute("data-render-mode", "xray");
  const xrayImage = await canvas.screenshot({
    path: testInfo.outputPath("cadquery-render-mode-xray.png"),
  });

  await expect(imageDifferenceRatio(page, solidImage, wireframeImage)).resolves.toBeGreaterThan(
    0.001,
  );
  await expect(imageDifferenceRatio(page, wireframeImage, xrayImage)).resolves.toBeGreaterThan(
    0.001,
  );
});

async function installViewer(page: Page, scene: object) {
  await page.goto(HARNESS.baseUrl);
  await page.evaluate(async (scenePayload) => {
    document.body.innerHTML =
      '<canvas id="cadquery-canvas" style="width: 640px; height: 480px"></canvas>';
    const modulePath = "/src/viewers/mesh-three.ts";
    const mod = await import(/* @vite-ignore */ modulePath);
    const canvas = document.getElementById(
      "cadquery-canvas",
    ) as HTMLCanvasElement;
    const viewer = mod.createMeshViewer(canvas);
    window.__cadQueryViewer = viewer;
    viewer.resize(640, 480, 1);
    viewer.setCadQuerySelectionMode("face");
    viewer.setCadQueryMesh(scenePayload, {
      frame: false,
      onPick: (pick: unknown) => {
        window.__cadQueryPick = pick;
      },
    });
    viewer.setCamera({
      position: [0, 0, 120],
      target: [0, 0, 0],
      up: [0, 1, 0],
      fovYDeg: 35,
      near: 0.1,
      far: 1000,
    });
  }, scene);
}

async function setSelectionMode(page: Page, mode: string) {
  await page.evaluate((nextMode) => {
    window.__cadQueryPick = undefined;
    window.__cadQueryViewer?.setCadQuerySelectionMode(nextMode);
  }, mode);
}

function cadQueryTriangleScene() {
  return {
    resultId: "cq_browser",
    buildId: "sha256:browser",
    rootRefText: "@part[top_lid]",
    rootObjectKind: "part",
    parts: [cadQueryPart(0, null, 0)],
  };
}

function cadQueryRepeatedInstanceScene() {
  return {
    resultId: "cq_browser",
    buildId: "sha256:browser",
    rootRefText: "@assembly[full_enclosure]",
    rootObjectKind: "assembly",
    parts: [
      {
        ...cadQueryPart(0, "full_enclosure/top_lid_left", 0),
        transform: translationTransform(-30, 0, 0),
      },
      {
        ...cadQueryPart(1, "full_enclosure/top_lid_right", 0),
        transform: translationTransform(30, 0, 0),
      },
    ],
  };
}

function cadQueryComponentInstanceScene() {
  const part = cadQueryPart(0, "full_enclosure/insert", 0);
  return {
    resultId: "cq_browser",
    buildId: "sha256:browser",
    rootRefText: "@assembly[full_enclosure]",
    rootObjectKind: "assembly",
    parts: [
      {
        ...part,
        name: "insert",
        objectKind: "component",
        refText: "@component[insert]",
        faces: part.faces.map((face) => ({
          ...face,
          features: ["raw_surface"],
        })),
        featureMap: [{ feature: "lid_alignment_surface", faceIndices: [0] }],
      },
    ],
  };
}

function cadQueryUnmappedFeatureScene() {
  const part = cadQueryPart(0, "full_enclosure/insert", 0);
  return {
    resultId: "cq_browser",
    buildId: "sha256:browser",
    rootRefText: "@assembly[full_enclosure]",
    rootObjectKind: "assembly",
    parts: [
      {
        ...part,
        name: "insert",
        objectKind: "component",
        refText: "@component[insert]",
        faces: part.faces.map((face) => ({
          ...face,
          features: ["raw_surface"],
        })),
        featureMap: [],
      },
    ],
  };
}

function cadQueryNestedAssemblyScene() {
  return {
    resultId: "cq_browser",
    buildId: "sha256:browser",
    rootRefText: "@assembly[full_enclosure]",
    rootObjectKind: "assembly",
    parts: [
      {
        ...cadQueryPart(0, null, 0),
        name: "inner_frame",
        objectKind: "assembly",
        refText: "@assembly[inner_frame]",
      },
    ],
  };
}

function cadQueryPart(partIndex: number, instancePath: string | null, x: number) {
  return {
    partIndex,
    name: "top_lid",
    objectKind: "part",
    refText: "@part[top_lid]",
    instancePath,
    transform: null,
    faces: [
      {
        faceIndex: 0,
        positions: new Float32Array([
          x - 12,
          -14,
          0,
          x + 12,
          -14,
          0,
          x,
          14,
          0,
        ]),
        normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1]),
        features: ["lid_alignment_surface"],
        ambiguous: false,
      },
    ],
    edges: [
      {
        edgeIndex: 0,
        polyline: new Float32Array([x - 12, 0, 0, x + 12, 0, 0]),
        adjacentFaces: [0],
      },
    ],
    vertices: [{ vertexIndex: 0, position: [x, 0, 0], adjacentEdges: [0] }],
    featureMap: [{ feature: "lid_alignment_surface", faceIndices: [0] }],
  };
}

function translationTransform(x: number, y: number, z: number) {
  return [1, 0, 0, x, 0, 1, 0, y, 0, 0, 1, z, 0, 0, 0, 1];
}

async function imageDifferenceRatio(page: Page, a: Buffer, b: Buffer): Promise<number> {
  return page.evaluate(async ({ aBase64, bBase64 }) => {
    async function decode(base64: string) {
      const response = await fetch(`data:image/png;base64,${base64}`);
      const bitmap = await createImageBitmap(await response.blob());
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("2D canvas context unavailable");
      context.drawImage(bitmap, 0, 0);
      const data = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
      return { width: bitmap.width, height: bitmap.height, data: Array.from(data) };
    }
    const first = await decode(aBase64);
    const second = await decode(bBase64);
    if (first.width !== second.width || first.height !== second.height) {
      throw new Error("screenshots must have matching pixel dimensions");
    }
    let different = 0;
    for (let index = 0; index < first.data.length; index += 1) {
      if (first.data[index] !== second.data[index]) different += 1;
    }
    return different / first.data.length;
  }, { aBase64: a.toString("base64"), bBase64: b.toString("base64") });
}

declare global {
  interface Window {
    __cadQueryPick?: unknown;
    __cadQueryViewer?: {
      setCadQuerySelectionMode(mode: string): void;
      setCamera(state: unknown): void;
      setOptions(options: unknown): void;
    };
  }
}
