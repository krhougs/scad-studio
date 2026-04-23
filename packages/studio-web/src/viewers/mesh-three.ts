// Three.js WebGL renderer for PreviewMeshPayload. All async wgpu / Promise
// handling lives in TS; wasm side only decodes bytes. Keeps contract §10
// (wasm never awaits JS Promises) intact.

import {
  AmbientLight,
  BufferAttribute,
  BufferGeometry,
  Box3,
  Color,
  DirectionalLight,
  GridHelper,
  Mesh,
  MeshStandardMaterial,
  PerspectiveCamera,
  Scene,
  Vector3,
  WebGLRenderer,
} from "three";

export type MeshPayload = {
  positions: Float32Array;
  normals: Float32Array | null;
  indices: Uint32Array | null;
  vertexColors: Float32Array | null;
};

export type CameraState = {
  position: [number, number, number];
  target: [number, number, number];
  up: [number, number, number];
  fovYDeg: number;
  near: number;
  far: number;
};

export type MeshViewerHandle = {
  setMesh(payload: MeshPayload | null): void;
  setCamera(camera: CameraState): void;
  resize(width: number, height: number, dpr: number): void;
  /** 用户通过交互改变了相机；外部 state 需要同步。 */
  onCameraChange(cb: (camera: CameraState) => void): void;
  dispose(): void;
  /** 返回当前显示的 mesh 统计；nullable 表示尚未载入。 */
  getStats(): { vertices: number; indices: number } | null;
};

type PointerMode = "idle" | "orbit" | "pan";

export function createMeshViewer(canvas: HTMLCanvasElement): MeshViewerHandle {
  const renderer = new WebGLRenderer({
    canvas,
    antialias: true,
    alpha: true,
    powerPreference: "high-performance",
  });
  renderer.setClearColor(new Color(0x070708), 1);

  const scene = new Scene();
  scene.background = null;

  const ambient = new AmbientLight(0xffffff, 0.55);
  scene.add(ambient);
  const key = new DirectionalLight(0xffffff, 0.75);
  key.position.set(4, 6, 4);
  scene.add(key);
  const fill = new DirectionalLight(0xffffff, 0.35);
  fill.position.set(-4, -2, -3);
  scene.add(fill);

  const grid = new GridHelper(200, 40, 0x2c2c31, 0x1a1a1d);
  (grid.material as { transparent: boolean; opacity: number }).transparent = true;
  (grid.material as { transparent: boolean; opacity: number }).opacity = 0.5;
  scene.add(grid);

  const camera = new PerspectiveCamera(35, 1, 0.1, 5000);
  camera.position.set(160, 160, 200);
  camera.lookAt(0, 0, 0);

  const target = new Vector3(0, 0, 0);
  let upVec = new Vector3(0, 1, 0);

  let meshObj: Mesh | null = null;
  let stats: { vertices: number; indices: number } | null = null;

  const pointer = {
    mode: "idle" as PointerMode,
    lastX: 0,
    lastY: 0,
  };

  let cameraCallback: ((c: CameraState) => void) | null = null;

  function emitCamera(): void {
    if (!cameraCallback) return;
    cameraCallback({
      position: [camera.position.x, camera.position.y, camera.position.z],
      target: [target.x, target.y, target.z],
      up: [upVec.x, upVec.y, upVec.z],
      fovYDeg: camera.fov,
      near: camera.near,
      far: camera.far,
    });
  }

  function render(): void {
    renderer.render(scene, camera);
  }

  function applyCamera(state: CameraState): void {
    camera.position.set(state.position[0], state.position[1], state.position[2]);
    target.set(state.target[0], state.target[1], state.target[2]);
    upVec.set(state.up[0], state.up[1], state.up[2]);
    camera.up.copy(upVec);
    camera.fov = state.fovYDeg;
    camera.near = state.near;
    camera.far = state.far;
    camera.lookAt(target);
    camera.updateProjectionMatrix();
    render();
  }

  function frameToMesh(mesh: Mesh): void {
    const box = new Box3().setFromObject(mesh);
    if (box.isEmpty()) return;
    const center = new Vector3();
    box.getCenter(center);
    const size = new Vector3();
    box.getSize(size);
    const maxDim = Math.max(size.x, size.y, size.z, 1);
    const distance = maxDim * 2.4;
    camera.position.set(
      center.x + distance * 0.85,
      center.y + distance * 0.85,
      center.z + distance * 1.05,
    );
    target.copy(center);
    camera.up.copy(upVec);
    camera.lookAt(target);
    camera.near = Math.max(maxDim / 1000, 0.01);
    camera.far = Math.max(maxDim * 20, 1000);
    camera.updateProjectionMatrix();
    emitCamera();
  }

  function orbit(dx: number, dy: number): void {
    const offset = camera.position.clone().sub(target);
    const radius = offset.length();
    if (radius === 0) return;
    const theta = Math.atan2(offset.x, offset.z);
    const phi = Math.acos(Math.max(-1, Math.min(1, offset.y / radius)));
    const nextTheta = theta - dx * 0.008;
    const nextPhi = Math.max(0.1, Math.min(Math.PI - 0.1, phi - dy * 0.008));
    offset.set(
      radius * Math.sin(nextPhi) * Math.sin(nextTheta),
      radius * Math.cos(nextPhi),
      radius * Math.sin(nextPhi) * Math.cos(nextTheta),
    );
    camera.position.copy(target.clone().add(offset));
    camera.up.copy(upVec);
    camera.lookAt(target);
    emitCamera();
    render();
  }

  function pan(dx: number, dy: number): void {
    const offset = camera.position.clone().sub(target);
    const distance = offset.length();
    const factor = distance * Math.tan((camera.fov * Math.PI) / 360);
    const rect = canvas.getBoundingClientRect();
    const right = new Vector3()
      .crossVectors(camera.up, offset)
      .normalize();
    const trueUp = new Vector3().crossVectors(offset, right).normalize();
    const shiftX = (-dx / rect.height) * factor * 2;
    const shiftY = (dy / rect.height) * factor * 2;
    const delta = right.multiplyScalar(shiftX).add(trueUp.multiplyScalar(shiftY));
    camera.position.add(delta);
    target.add(delta);
    camera.lookAt(target);
    emitCamera();
    render();
  }

  function dolly(delta: number): void {
    const offset = camera.position.clone().sub(target);
    const factor = Math.exp(delta * 0.0015);
    offset.multiplyScalar(factor);
    const nextDist = offset.length();
    if (nextDist < 0.1 || nextDist > 20000) return;
    camera.position.copy(target.clone().add(offset));
    camera.lookAt(target);
    emitCamera();
    render();
  }

  function onPointerDown(ev: PointerEvent): void {
    canvas.setPointerCapture(ev.pointerId);
    pointer.lastX = ev.clientX;
    pointer.lastY = ev.clientY;
    const panMode = ev.button === 2 || ev.altKey || ev.button === 1;
    pointer.mode = panMode ? "pan" : "orbit";
    ev.preventDefault();
  }
  function onPointerMove(ev: PointerEvent): void {
    if (pointer.mode === "idle") return;
    const dx = ev.clientX - pointer.lastX;
    const dy = ev.clientY - pointer.lastY;
    pointer.lastX = ev.clientX;
    pointer.lastY = ev.clientY;
    if (pointer.mode === "orbit") orbit(dx, dy);
    else pan(dx, dy);
  }
  function onPointerUp(ev: PointerEvent): void {
    if (pointer.mode === "idle") return;
    pointer.mode = "idle";
    try {
      canvas.releasePointerCapture(ev.pointerId);
    } catch {
      /* ignore */
    }
  }
  function onWheel(ev: WheelEvent): void {
    ev.preventDefault();
    dolly(ev.deltaY);
  }
  function onContextMenu(ev: Event): void {
    ev.preventDefault();
  }

  canvas.addEventListener("pointerdown", onPointerDown);
  canvas.addEventListener("pointermove", onPointerMove);
  canvas.addEventListener("pointerup", onPointerUp);
  canvas.addEventListener("pointercancel", onPointerUp);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  canvas.addEventListener("contextmenu", onContextMenu);

  render();

  return {
    setMesh(payload) {
      if (meshObj) {
        scene.remove(meshObj);
        meshObj.geometry.dispose();
        (meshObj.material as MeshStandardMaterial).dispose();
        meshObj = null;
        stats = null;
      }
      if (!payload || payload.positions.length === 0) {
        render();
        return;
      }
      const geometry = new BufferGeometry();
      geometry.setAttribute(
        "position",
        new BufferAttribute(payload.positions, 3),
      );
      if (payload.normals && payload.normals.length === payload.positions.length) {
        geometry.setAttribute(
          "normal",
          new BufferAttribute(payload.normals, 3),
        );
      } else {
        geometry.computeVertexNormals();
      }
      if (payload.vertexColors) {
        // 4-component rgba — store as 3 rgb for MeshStandardMaterial vertex colors.
        const rgb = new Float32Array((payload.vertexColors.length / 4) * 3);
        for (let i = 0, j = 0; i < payload.vertexColors.length; i += 4, j += 3) {
          rgb[j] = payload.vertexColors[i];
          rgb[j + 1] = payload.vertexColors[i + 1];
          rgb[j + 2] = payload.vertexColors[i + 2];
        }
        geometry.setAttribute("color", new BufferAttribute(rgb, 3));
      }
      if (payload.indices) {
        geometry.setIndex(new BufferAttribute(payload.indices, 1));
      }
      const material = new MeshStandardMaterial({
        color: 0x3a3a40,
        metalness: 0.15,
        roughness: 0.72,
        vertexColors: payload.vertexColors !== null,
      });
      meshObj = new Mesh(geometry, material);
      scene.add(meshObj);
      const vertexCount = payload.positions.length / 3;
      const indexCount = payload.indices ? payload.indices.length : vertexCount;
      stats = { vertices: vertexCount, indices: indexCount };
      frameToMesh(meshObj);
      render();
    },
    setCamera(state) {
      applyCamera(state);
    },
    resize(width, height, dpr) {
      const w = Math.max(1, Math.floor(width));
      const h = Math.max(1, Math.floor(height));
      renderer.setPixelRatio(Math.min(dpr, 2));
      renderer.setSize(w, h, false);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      render();
    },
    onCameraChange(cb) {
      cameraCallback = cb;
    },
    dispose() {
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerup", onPointerUp);
      canvas.removeEventListener("pointercancel", onPointerUp);
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("contextmenu", onContextMenu);
      if (meshObj) {
        meshObj.geometry.dispose();
        (meshObj.material as MeshStandardMaterial).dispose();
      }
      grid.geometry.dispose();
      const gridMat = grid.material as { dispose?: () => void };
      gridMat.dispose?.();
      renderer.dispose();
    },
    getStats() {
      return stats;
    },
  };
}

export function payloadFromPreview(payload: unknown): MeshPayload | null {
  if (!payload || typeof payload !== "object") return null;
  const outer = payload as Record<string, unknown>;
  const ready = (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const artifact = ready["artifact"] as Record<string, unknown> | undefined;
  if (!artifact) return null;
  const format = artifact["format"];
  const inner = artifact["payload"] as Record<string, unknown> | undefined;
  if (!inner) return null;
  if (format === "mesh") {
    return meshFromPayload(inner);
  }
  return null;
}

function meshFromPayload(inner: Record<string, unknown>): MeshPayload | null {
  const positions = flattenVec3(inner["positions"]);
  if (!positions) return null;
  const normals = flattenVec3(inner["normals"]);
  const vertexColors = flattenVec4(inner["vertex_colors"]);
  const indices = flattenU32(inner["indices"]);
  return { positions, normals, vertexColors, indices };
}

function flattenVec3(raw: unknown): Float32Array | null {
  if (!Array.isArray(raw)) return null;
  const out = new Float32Array(raw.length * 3);
  for (let i = 0; i < raw.length; i++) {
    const v = raw[i] as unknown;
    if (!Array.isArray(v) || v.length < 3) return null;
    out[i * 3] = Number(v[0]);
    out[i * 3 + 1] = Number(v[1]);
    out[i * 3 + 2] = Number(v[2]);
  }
  return out;
}

function flattenVec4(raw: unknown): Float32Array | null {
  if (!Array.isArray(raw) || raw.length === 0) return null;
  const out = new Float32Array(raw.length * 4);
  for (let i = 0; i < raw.length; i++) {
    const v = raw[i] as unknown;
    if (!Array.isArray(v) || v.length < 4) return null;
    out[i * 4] = Number(v[0]);
    out[i * 4 + 1] = Number(v[1]);
    out[i * 4 + 2] = Number(v[2]);
    out[i * 4 + 3] = Number(v[3]);
  }
  return out;
}

function flattenU32(raw: unknown): Uint32Array | null {
  if (!Array.isArray(raw) || raw.length === 0) return null;
  const out = new Uint32Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = Number(raw[i]) >>> 0;
  return out;
}
