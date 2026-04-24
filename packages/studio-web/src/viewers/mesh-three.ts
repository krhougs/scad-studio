// Three.js WebGL renderer for PreviewMeshPayload. All async wgpu / Promise
// handling lives in TS; wasm side only decodes bytes. Keeps contract §10
// (wasm never awaits JS Promises) intact.

import {
  AmbientLight,
  AxesHelper,
  BufferAttribute,
  BufferGeometry,
  Color,
  DirectionalLight,
  DoubleSide,
  Fog,
  GridHelper,
  Mesh,
  MeshStandardMaterial,
  OrthographicCamera,
  PerspectiveCamera,
  Plane,
  PlaneGeometry,
  Scene,
  Vector3,
  WebGLRenderer,
} from "three";
import { fitCameraToBounds } from "../canvas/camera-controls";
import {
  DEFAULT_MESH_VIEWER_OPTIONS,
  type MeshViewerOptions,
} from "./viewer-options";
import type { CameraPreset } from "../canvas/camera-state";
import {
  computeMeshInfo,
  meshBuildPlateSize,
  type MeshInfo,
} from "./mesh-info";

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
  setMesh(
    payload: MeshPayload | null,
    opts?: { frame?: boolean; preset?: CameraPreset },
  ): MeshInfo | null;
  setCamera(camera: CameraState): void;
  setOptions(options: MeshViewerOptions): void;
  resize(width: number, height: number, dpr: number): void;
  /** 用户通过交互改变了相机；外部 state 需要同步。 */
  onCameraChange(cb: (camera: CameraState) => void): void;
  dispose(): void;
  /** 返回当前显示的 mesh 统计；nullable 表示尚未载入。 */
  getStats(): { vertices: number; indices: number } | null;
  getInfo(): MeshInfo | null;
};

type PointerMode = "idle" | "orbit" | "pan";

export function createMeshViewer(canvas: HTMLCanvasElement): MeshViewerHandle {
  const renderer = new WebGLRenderer({
    canvas,
    antialias: true,
    alpha: true,
    powerPreference: "high-performance",
  });
  const backgroundColor = new Color(0x070708);
  renderer.setClearColor(backgroundColor, 1);

  const scene = new Scene();
  scene.background = backgroundColor;

  const ambient = new AmbientLight(0xffffff, 0.55);
  scene.add(ambient);
  const key = new DirectionalLight(0xffffff, 0.75);
  key.position.set(4, 6, 4);
  key.shadow.camera.left = -240;
  key.shadow.camera.right = 240;
  key.shadow.camera.top = 240;
  key.shadow.camera.bottom = -240;
  key.shadow.camera.near = 1;
  key.shadow.camera.far = 1200;
  scene.add(key);
  const fill = new DirectionalLight(0xffffff, 0.35);
  fill.position.set(-4, -2, -3);
  scene.add(fill);
  const rim = new DirectionalLight(0xc7e5ff, 0.5);
  rim.position.set(-5, 8, 6);
  scene.add(rim);

  const grid = new GridHelper(200, 40, 0x2c2c31, 0x1a1a1d);
  (grid.material as { transparent: boolean; opacity: number }).transparent = true;
  (grid.material as { transparent: boolean; opacity: number }).opacity = 0.5;
  scene.add(grid);

  const axes = new AxesHelper(80);
  scene.add(axes);

  const buildPlate = new Mesh(
    new PlaneGeometry(200, 200),
    new MeshStandardMaterial({
      color: 0x1d2930,
      transparent: true,
      opacity: 0.22,
      side: DoubleSide,
      roughness: 0.9,
    }),
  );
  buildPlate.rotation.x = -Math.PI / 2;
  buildPlate.position.y = -0.02;
  buildPlate.receiveShadow = true;
  scene.add(buildPlate);

  const perspectiveCamera = new PerspectiveCamera(35, 1, 0.1, 5000);
  const orthographicCamera = new OrthographicCamera(-100, 100, 100, -100, 0.1, 5000);
  let camera: PerspectiveCamera | OrthographicCamera = perspectiveCamera;
  camera.position.set(160, 160, 200);
  camera.lookAt(0, 0, 0);
  orthographicCamera.position.copy(camera.position);
  orthographicCamera.lookAt(0, 0, 0);

  const target = new Vector3(0, 0, 0);
  let upVec = new Vector3(0, 1, 0);
  let options = { ...DEFAULT_MESH_VIEWER_OPTIONS };
  let viewportWidth = 1;
  let viewportHeight = 1;

  let meshObj: Mesh | null = null;
  let meshMaterial: MeshStandardMaterial | null = null;
  let stats: { vertices: number; indices: number } | null = null;
  let meshInfo: MeshInfo | null = null;
  let meshHasVertexColors = false;
  const clipPlane = new Plane(new Vector3(1, 0, 0), 0);

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
      fovYDeg: perspectiveCamera.fov,
      near: camera.near,
      far: camera.far,
    });
  }

  function render(): void {
    renderer.render(scene, camera);
  }

  function updateProjection(): void {
    if (options.projectionMode === "perspective") {
      perspectiveCamera.aspect = viewportWidth / viewportHeight;
      perspectiveCamera.updateProjectionMatrix();
      return;
    }
    const distance = Math.max(camera.position.distanceTo(target), 20);
    const halfHeight = distance * 0.38;
    const halfWidth = halfHeight * (viewportWidth / viewportHeight);
    orthographicCamera.left = -halfWidth;
    orthographicCamera.right = halfWidth;
    orthographicCamera.top = halfHeight;
    orthographicCamera.bottom = -halfHeight;
    orthographicCamera.updateProjectionMatrix();
  }

  function setProjectionMode(mode: MeshViewerOptions["projectionMode"]): void {
    const previous = camera;
    camera = mode === "orthographic" ? orthographicCamera : perspectiveCamera;
    camera.position.copy(previous.position);
    camera.up.copy(previous.up);
    camera.near = previous.near;
    camera.far = previous.far;
    camera.lookAt(target);
    updateProjection();
  }

  function applyCamera(state: CameraState): void {
    camera.position.set(state.position[0], state.position[1], state.position[2]);
    target.set(state.target[0], state.target[1], state.target[2]);
    upVec.set(state.up[0], state.up[1], state.up[2]);
    camera.up.copy(upVec);
    perspectiveCamera.fov = state.fovYDeg;
    camera.near = state.near;
    camera.far = state.far;
    camera.lookAt(target);
    updateProjection();
    render();
  }

  function applyOptions(): void {
    setProjectionMode(options.projectionMode);
    grid.visible = options.showGrid;
    axes.visible = options.showAxis;
    buildPlate.visible = options.showBuildPlate;
    renderer.shadowMap.enabled = options.shadowsEnabled;
    renderer.localClippingEnabled = options.clipPlaneEnabled;
    key.castShadow = options.shadowsEnabled;
    fill.castShadow = options.shadowsEnabled;
    scene.fog =
      options.fogEnabled && meshInfo
        ? new Fog(
            backgroundColor,
            Math.max(meshInfo.radius * 3, 60),
            Math.max(meshInfo.radius * 9, 220),
          )
        : null;
    if (meshObj && meshMaterial) {
      meshObj.castShadow = options.shadowsEnabled;
      meshObj.receiveShadow = options.shadowsEnabled;
      meshMaterial.color.set(options.colorMode === "mono" ? 0x9fb8c6 : 0x7f858a);
      meshMaterial.vertexColors =
        options.colorMode === "color" && meshHasVertexColors;
      meshMaterial.wireframe = options.renderMode === "wireframe";
      meshMaterial.transparent = options.renderMode === "xray";
      meshMaterial.opacity = options.renderMode === "xray" ? 0.36 : 1;
      meshMaterial.depthWrite = options.renderMode !== "xray";
      meshMaterial.clippingPlanes = options.clipPlaneEnabled ? [clipPlane] : [];
      meshMaterial.needsUpdate = true;
    }
    syncCanvasDataset();
    render();
  }

  function syncCanvasDataset(): void {
    canvas.dataset.renderMode = options.renderMode;
    canvas.dataset.projectionMode = options.projectionMode;
    canvas.dataset.colorMode = options.colorMode;
    canvas.dataset.showGrid = String(options.showGrid);
    canvas.dataset.showAxis = String(options.showAxis);
    canvas.dataset.showBuildPlate = String(options.showBuildPlate);
    canvas.dataset.shadowsEnabled = String(options.shadowsEnabled);
    canvas.dataset.fogEnabled = String(options.fogEnabled);
    canvas.dataset.clipPlaneEnabled = String(options.clipPlaneEnabled);
  }

  function frameToInfo(info: MeshInfo, preset: CameraPreset): void {
    applyCamera(
      fitCameraToBounds(info.bounds, preset, viewportWidth / viewportHeight),
    );
    emitCamera();
  }

  function updateSceneScale(info: MeshInfo | null): void {
    const plateSize = meshBuildPlateSize(info);
    const scale = plateSize / 200;
    grid.scale.setScalar(scale);
    axes.scale.setScalar(Math.max(0.2, plateSize / 80));
    buildPlate.scale.set(plateSize / 200, plateSize / 200, 1);
    if (info) {
      const center = new Vector3(...info.center);
      const bottom = info.bounds.min[1] - Math.max(info.radius * 0.015, 0.02);
      grid.position.set(center.x, bottom, center.z);
      buildPlate.position.set(center.x, bottom - 0.01, center.z);
      clipPlane.constant = -center.x;
      return;
    }
    grid.position.set(0, 0, 0);
    buildPlate.position.set(0, -0.02, 0);
    clipPlane.constant = 0;
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
    const factor =
      options.projectionMode === "orthographic"
        ? (orthographicCamera.top - orthographicCamera.bottom) / 2
        : distance * Math.tan((perspectiveCamera.fov * Math.PI) / 360);
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
    updateProjection();
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

  applyOptions();

  return {
    setMesh(payload, opts) {
      if (meshObj) {
        scene.remove(meshObj);
        meshObj.geometry.dispose();
        (meshObj.material as MeshStandardMaterial).dispose();
        meshObj = null;
        meshMaterial = null;
        stats = null;
        meshInfo = null;
        meshHasVertexColors = false;
      }
      if (!payload || payload.positions.length === 0) {
        updateSceneScale(null);
        render();
        return null;
      }
      const info = computeMeshInfo(payload.positions, payload.indices);
      if (!info) {
        updateSceneScale(null);
        render();
        return null;
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
      geometry.computeBoundingBox();
      const material = new MeshStandardMaterial({
        color: 0x7f858a,
        metalness: 0.05,
        roughness: 0.58,
        side: DoubleSide,
        vertexColors: options.colorMode === "color" && payload.vertexColors !== null,
      });
      meshMaterial = material;
      meshObj = new Mesh(geometry, material);
      scene.add(meshObj);
      stats = { vertices: info.vertices, indices: info.indices };
      meshInfo = info;
      meshHasVertexColors = payload.vertexColors !== null;
      updateSceneScale(info);
      if (opts?.frame !== false) frameToInfo(info, opts?.preset ?? "iso");
      applyOptions();
      render();
      return info;
    },
    setCamera(state) {
      applyCamera(state);
      emitCamera();
    },
    setOptions(next) {
      options = { ...next };
      applyOptions();
    },
    resize(width, height, dpr) {
      const w = Math.max(1, Math.floor(width));
      const h = Math.max(1, Math.floor(height));
      viewportWidth = w;
      viewportHeight = h;
      renderer.setPixelRatio(Math.min(dpr, 2));
      renderer.setSize(w, h, false);
      updateProjection();
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
      axes.dispose();
      buildPlate.geometry.dispose();
      (buildPlate.material as MeshStandardMaterial).dispose();
      renderer.dispose();
    },
    getStats() {
      return stats;
    },
    getInfo() {
      return meshInfo;
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
