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
  HemisphereLight,
  Mesh,
  MeshStandardMaterial,
  OrthographicCamera,
  PerspectiveCamera,
  Plane,
  PlaneGeometry,
  PointLight,
  Scene,
  Vector3,
  WebGLRenderer,
} from "three";
import {
  classifyPointerMode,
  fitCameraToBounds,
  wheelDeltaToZoomAmount,
} from "../canvas/camera-controls";
import {
  DEFAULT_MESH_VIEWER_OPTIONS,
  type MeshViewerOptions,
} from "./viewer-options";
import { PRESET_STATES, type CameraPreset } from "../canvas/camera-state";
import {
  computeMeshInfo,
  type MeshInfo,
} from "./mesh-info";
import {
  clippingPlanesForBounds,
  meshRenderInputsReady,
  meshSceneMetrics,
  orthographicHalfHeightForCamera,
  pointLightAutoPositionForBounds,
  type MeshRenderViewport,
} from "./mesh-render-metrics";
import type { PointLightMode, PointLightPosition } from "./viewer-options";

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
  frameCamera(preset: CameraPreset): void;
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
const GRID_DIVISIONS = 40;

export function createMeshViewer(canvas: HTMLCanvasElement): MeshViewerHandle {
  const renderer = new WebGLRenderer({
    canvas,
    antialias: true,
    alpha: true,
    powerPreference: "high-performance",
  });
  const backgroundHex = DEFAULT_MESH_VIEWER_OPTIONS.backgroundColor;
  const backgroundColor = new Color(backgroundHex);
  renderer.setClearColor(backgroundColor, 1);

  const scene = new Scene();
  scene.background = backgroundColor;

  const ambient = new AmbientLight(0xffffff, 0.35);
  scene.add(ambient);
  const hemi = new HemisphereLight(0xf4f8ff, 0x6a7480, 0.65);
  hemi.position.set(0, 0, 1);
  scene.add(hemi);
  const key = new DirectionalLight(0xffffff, 0.85);
  key.position.set(4, 6, 4);
  key.shadow.camera.left = -240;
  key.shadow.camera.right = 240;
  key.shadow.camera.top = 240;
  key.shadow.camera.bottom = -240;
  key.shadow.camera.near = 1;
  key.shadow.camera.far = 1200;
  scene.add(key);
  const fill = new DirectionalLight(0xffffff, 0.5);
  fill.position.set(-5, -4, 3);
  scene.add(fill);
  const rim = new DirectionalLight(0xd9ecff, 0.42);
  rim.position.set(-4, 5, 7);
  scene.add(rim);
  const back = new DirectionalLight(0xffffff, 0.35);
  back.position.set(4, -6, -2);
  scene.add(back);
  const extraPoint = new PointLight(0xffffff, 0, 0, 2);
  extraPoint.visible = false;
  extraPoint.castShadow = false;
  extraPoint.shadow.mapSize.width = 1024;
  extraPoint.shadow.mapSize.height = 1024;
  extraPoint.shadow.camera.near = 0.5;
  extraPoint.shadow.camera.far = 5000;
  scene.add(extraPoint);

  let grid = createGrid(
    DEFAULT_MESH_VIEWER_OPTIONS.gridMajorColor,
    DEFAULT_MESH_VIEWER_OPTIONS.gridMinorColor,
  );
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
  buildPlate.position.z = -0.02;
  buildPlate.receiveShadow = true;
  scene.add(buildPlate);

  const perspectiveCamera = new PerspectiveCamera(35, 1, 0.1, 5000);
  const orthographicCamera = new OrthographicCamera(-100, 100, 100, -100, 0.1, 5000);
  let camera: PerspectiveCamera | OrthographicCamera = perspectiveCamera;
  camera.position.set(160, 160, 200);
  camera.up.set(0, 0, 1);
  camera.lookAt(0, 0, 0);
  orthographicCamera.position.copy(camera.position);
  orthographicCamera.up.copy(camera.up);
  orthographicCamera.lookAt(0, 0, 0);

  const target = new Vector3(0, 0, 0);
  let upVec = new Vector3(0, 0, 1);
  let options = { ...DEFAULT_MESH_VIEWER_OPTIONS };
  let viewportWidth = 1;
  let viewportHeight = 1;
  let viewportDpr = 1;
  let viewportReady = false;

  let meshObj: Mesh | null = null;
  let meshMaterial: MeshStandardMaterial | null = null;
  let stats: { vertices: number; indices: number } | null = null;
  let meshInfo: MeshInfo | null = null;
  let meshHasVertexColors = false;
  let pendingFrame: { info: MeshInfo; preset: CameraPreset } | null = null;
  let framedDistance = camera.position.distanceTo(target);
  let autoFramePreset: CameraPreset | null = null;
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
      delete canvas.dataset.orthographicHalfHeight;
      return;
    }
    const baseHalfHeight = orthographicHalfHeightForCamera(
      meshInfo,
      currentViewport(),
      emitCameraState(),
    );
    if (baseHalfHeight === null) {
      delete canvas.dataset.orthographicHalfHeight;
      return;
    }
    const distance = camera.position.distanceTo(target);
    const zoomScale = Math.max(distance, 0.1) / Math.max(framedDistance, 1);
    const halfHeight = Math.max(baseHalfHeight * zoomScale, 0.1);
    const halfWidth = halfHeight * (viewportWidth / viewportHeight);
    orthographicCamera.left = -halfWidth;
    orthographicCamera.right = halfWidth;
    orthographicCamera.top = halfHeight;
    orthographicCamera.bottom = -halfHeight;
    canvas.dataset.orthographicHalfHeight = halfHeight.toFixed(3);
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
    syncClipPlanes();
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
    syncClipPlanes();
    updateProjection();
    render();
  }

  function applyOptions(): void {
    applyAppearance();
    setProjectionMode(options.projectionMode);
    grid.visible = options.showGrid;
    axes.visible = options.showAxis;
    buildPlate.visible = options.showBuildPlate;
    renderer.shadowMap.enabled = options.shadowsEnabled;
    renderer.localClippingEnabled = options.clipPlaneEnabled;
    key.castShadow = options.shadowsEnabled;
    fill.castShadow = false;
    rim.castShadow = false;
    back.castShadow = false;
    syncPointLight();
    const metrics = currentMetrics();
    scene.fog =
      options.fogEnabled && metrics
        ? new Fog(backgroundColor, metrics.fogNear, metrics.fogFar)
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

  function applyAppearance(): void {
    backgroundColor.set(options.backgroundColor);
    renderer.setClearColor(backgroundColor, 1);
    scene.background = backgroundColor;
    syncGridColors();
    const intensity = options.lightingIntensity;
    ambient.intensity = 0.35 * intensity;
    hemi.intensity = 0.65 * intensity;
    key.intensity = 0.85 * intensity;
    fill.intensity = 0.5 * intensity;
    rim.intensity = 0.42 * intensity;
    back.intensity = 0.35 * intensity;
    syncPointLight();
  }

  function syncPointLight(): void {
    const mode = effectivePointLightMode();
    if (mode === "off") {
      extraPoint.visible = false;
      extraPoint.intensity = 0;
      extraPoint.castShadow = false;
      return;
    }
    const position = currentPointLightPosition(mode);
    extraPoint.position.set(position[0], position[1], position[2]);
    extraPoint.visible = true;
    extraPoint.intensity = Math.max(0, options.lightingIntensity * 0.55);
    extraPoint.castShadow = options.shadowsEnabled;
  }

  function syncGridColors(): void {
    const currentMajor = grid.userData["majorColor"];
    const currentMinor = grid.userData["minorColor"];
    if (
      currentMajor === options.gridMajorColor &&
      currentMinor === options.gridMinorColor
    ) {
      return;
    }
    const next = createGrid(options.gridMajorColor, options.gridMinorColor);
    next.visible = grid.visible;
    next.position.copy(grid.position);
    next.scale.copy(grid.scale);
    scene.remove(grid);
    grid.geometry.dispose();
    disposeGridMaterial(grid.material);
    grid = next;
    scene.add(grid);
  }

  function syncCanvasDataset(): void {
    const metrics = currentMetrics();
    canvas.dataset.renderMode = options.renderMode;
    canvas.dataset.projectionMode = options.projectionMode;
    canvas.dataset.colorMode = options.colorMode;
    canvas.dataset.showGrid = String(options.showGrid);
    canvas.dataset.showAxis = String(options.showAxis);
    canvas.dataset.showBuildPlate = String(options.showBuildPlate);
    canvas.dataset.shadowsEnabled = String(options.shadowsEnabled);
    canvas.dataset.fogEnabled = String(options.fogEnabled);
    canvas.dataset.clipPlaneEnabled = String(options.clipPlaneEnabled);
    canvas.dataset.previewBackground = options.backgroundColor;
    canvas.dataset.gridMajorColor = options.gridMajorColor;
    canvas.dataset.gridMinorColor = options.gridMinorColor;
    canvas.dataset.gridColorSignature = String(grid.userData["colorSignature"] ?? "");
    canvas.dataset.lightingIntensity = String(options.lightingIntensity);
    const pointLightMode = effectivePointLightMode();
    canvas.dataset.pointLightMode = options.pointLightMode;
    canvas.dataset.effectivePointLightMode = pointLightMode;
    canvas.dataset.pointLightForced = String(
      options.shadowsEnabled && options.pointLightMode === "off",
    );
    canvas.dataset.pointLightEnabled = String(extraPoint.visible);
    canvas.dataset.pointLightCastShadow = String(extraPoint.castShadow);
    if (extraPoint.visible) {
      canvas.dataset.pointLightPosition = formatPointLightPosition([
        extraPoint.position.x,
        extraPoint.position.y,
        extraPoint.position.z,
      ]);
    } else {
      delete canvas.dataset.pointLightPosition;
    }
    canvas.dataset.lightRigIntensity = (
      ambient.intensity +
      hemi.intensity +
      key.intensity +
      fill.intensity +
      rim.intensity +
      back.intensity +
      extraPoint.intensity
    ).toFixed(3);
    if (metrics) {
      canvas.dataset.gizmoSize = String(Math.round(metrics.gizmoSize));
      canvas.dataset.fogNear = metrics.fogNear.toFixed(3);
      canvas.dataset.fogFar = metrics.fogFar.toFixed(3);
    } else {
      delete canvas.dataset.gizmoSize;
      delete canvas.dataset.fogNear;
      delete canvas.dataset.fogFar;
    }
  }

  function effectivePointLightMode(): PointLightMode {
    if (options.shadowsEnabled && options.pointLightMode === "off") return "auto";
    return options.pointLightMode;
  }

  function currentPointLightPosition(mode: PointLightMode): PointLightPosition {
    if (mode === "manual" && options.pointLightPosition) {
      return options.pointLightPosition;
    }
    if (meshInfo && viewportReady) {
      return pointLightAutoPositionForBounds(
        meshInfo.bounds,
        viewportWidth / viewportHeight,
      );
    }
    return options.pointLightPosition ?? [0, -160, 160];
  }

  function formatPointLightPosition(position: PointLightPosition): string {
    return position.map((item) => item.toFixed(3)).join(",");
  }

  function clearMetricsDataset(): void {
    delete canvas.dataset.gizmoSize;
    delete canvas.dataset.fogNear;
    delete canvas.dataset.fogFar;
    delete canvas.dataset.clipNear;
    delete canvas.dataset.clipFar;
    delete canvas.dataset.orthographicHalfHeight;
  }

  function frameToInfo(info: MeshInfo, preset: CameraPreset): void {
    if (!meshRenderInputsReady(info, currentViewport())) {
      pendingFrame = { info, preset };
      return;
    }
    pendingFrame = null;
    const nextCamera = fitCameraToBounds(
      info.bounds,
      preset,
      viewportWidth / viewportHeight,
    );
    framedDistance = Math.hypot(
      nextCamera.position[0] - nextCamera.target[0],
      nextCamera.position[1] - nextCamera.target[1],
      nextCamera.position[2] - nextCamera.target[2],
    );
    autoFramePreset = preset;
    applyCamera(nextCamera);
    emitCamera();
  }

  function updateSceneScale(info: MeshInfo | null): void {
    const metrics = meshSceneMetrics(info, currentViewport());
    if (info && !metrics) return;
    const plateSize = metrics?.plateSize ?? 200;
    const scale = plateSize / 200;
    grid.scale.setScalar(scale);
    axes.scale.setScalar(Math.max(0.2, (metrics?.axisSize ?? 80) / 80));
    buildPlate.scale.set(plateSize / 200, plateSize / 200, 1);
    if (info) {
      const center = new Vector3(...info.center);
      const bottom = info.bounds.min[2] - Math.max(info.radius * 0.015, 0.02);
      grid.position.set(center.x, center.y, bottom);
      buildPlate.position.set(center.x, center.y, bottom - 0.01);
      clipPlane.constant = -center.x;
      return;
    }
    grid.position.set(0, 0, 0);
    buildPlate.position.set(0, 0, -0.02);
    clipPlane.constant = 0;
  }

  function orbit(dx: number, dy: number): void {
    const offset = camera.position.clone().sub(target);
    const radius = offset.length();
    if (radius === 0) return;
    const spherical = orbitAngles(offset, upVec);
    const azimuth = spherical.azimuth;
    const elevation = spherical.elevation;
    const nextAzimuth = wrapAngle(azimuth - dx * 0.01);
    const nextElevation = wrapAngle(elevation - dy * 0.01);
    const cp = Math.cos(nextElevation);
    offset.set(
      radius * cp * Math.cos(nextAzimuth),
      radius * cp * Math.sin(nextAzimuth),
      radius * Math.sin(nextElevation),
    );
    upVec = orbitUp(nextAzimuth, nextElevation);
    camera.position.copy(target.clone().add(offset));
    camera.up.copy(upVec);
    autoFramePreset = null;
    camera.lookAt(target);
    syncClipPlanes();
    updateProjection();
    emitCamera();
    render();
  }

  function pan(dx: number, dy: number): void {
    const offset = camera.position.clone().sub(target);
    const distance = offset.length();
    const factor = distance * 0.002;
    const right = new Vector3()
      .crossVectors(camera.up, offset)
      .normalize();
    const trueUp = new Vector3().crossVectors(offset, right).normalize();
    const shiftX = -dx * factor;
    const shiftY = dy * factor;
    const delta = right.multiplyScalar(shiftX).add(trueUp.multiplyScalar(shiftY));
    camera.position.add(delta);
    target.add(delta);
    autoFramePreset = null;
    camera.lookAt(target);
    syncClipPlanes();
    updateProjection();
    emitCamera();
    render();
  }

  function dolly(delta: number): void {
    const offset = camera.position.clone().sub(target);
    const factor = Math.min(5, Math.max(0.2, 1 - delta * 0.12));
    offset.multiplyScalar(factor);
    const nextDist = offset.length();
    if (nextDist < 0.1 || nextDist > 20000) return;
    camera.position.copy(target.clone().add(offset));
    autoFramePreset = null;
    syncClipPlanes();
    camera.lookAt(target);
    updateProjection();
    emitCamera();
    render();
  }

  function onPointerDown(ev: PointerEvent): void {
    const mode = classifyPointerMode({
      button: ev.button,
      altKey: ev.altKey,
    });
    if (mode === "none") return;
    canvas.setPointerCapture(ev.pointerId);
    pointer.lastX = ev.clientX;
    pointer.lastY = ev.clientY;
    pointer.mode = mode;
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
    dolly(wheelDeltaToZoomAmount(ev.deltaY, ev.deltaMode));
  }
  function onContextMenu(ev: Event): void {
    ev.preventDefault();
  }

  function currentViewport(): MeshRenderViewport {
    return {
      width: viewportReady ? viewportWidth : 0,
      height: viewportReady ? viewportHeight : 0,
      dpr: viewportReady ? viewportDpr : 0,
      projectionMode: options.projectionMode,
    };
  }

  function currentMetrics() {
    return meshSceneMetrics(meshInfo, currentViewport());
  }

  function syncClipPlanes(): void {
    if (!meshInfo) return;
    const planes = clippingPlanesForBounds(emitCameraState(), meshInfo.bounds);
    perspectiveCamera.near = planes.near;
    perspectiveCamera.far = planes.far;
    orthographicCamera.near = planes.near;
    orthographicCamera.far = planes.far;
    camera.near = planes.near;
    camera.far = planes.far;
    canvas.dataset.clipNear = planes.near.toFixed(3);
    canvas.dataset.clipFar = planes.far.toFixed(3);
  }

  function emitCameraState(): CameraState {
    return {
      position: [camera.position.x, camera.position.y, camera.position.z],
      target: [target.x, target.y, target.z],
      up: [upVec.x, upVec.y, upVec.z],
      fovYDeg: perspectiveCamera.fov,
      near: camera.near,
      far: camera.far,
    };
  }

  function wrapAngle(angle: number): number {
    const tau = Math.PI * 2;
    let wrapped = angle % tau;
    if (wrapped <= -Math.PI) wrapped += tau;
    else if (wrapped > Math.PI) wrapped -= tau;
    return wrapped;
  }

  function orbitAngles(
    offset: Vector3,
    up: Vector3,
  ): { azimuth: number; elevation: number } {
    const horizontal = Math.hypot(offset.x, offset.y);
    if (horizontal > 1e-9) {
      const azimuth = Math.atan2(offset.y, offset.x);
      const baseElevation = Math.atan2(offset.z, horizontal);
      const expectedUp = orbitUp(azimuth, baseElevation);
      if (expectedUp.dot(up) >= 0) return { azimuth, elevation: baseElevation };
      return {
        azimuth: wrapAngle(azimuth + Math.PI),
        elevation: wrapAngle(Math.PI - baseElevation),
      };
    }
    const horizontalUp = Math.hypot(up.x, up.y);
    if (horizontalUp > 1e-9) {
      const azimuth =
        offset.z >= 0
          ? Math.atan2(-up.y, -up.x)
          : Math.atan2(up.y, up.x);
      return {
        azimuth,
        elevation: offset.z >= 0 ? Math.PI / 2 : -Math.PI / 2,
      };
    }
    return {
      azimuth: 0,
      elevation: offset.z >= 0 ? Math.PI / 2 : -Math.PI / 2,
    };
  }

  function orbitUp(azimuth: number, elevation: number): Vector3 {
    return new Vector3(
      -Math.sin(elevation) * Math.cos(azimuth),
      -Math.sin(elevation) * Math.sin(azimuth),
      Math.cos(elevation),
    ).normalize();
  }

  function createGrid(majorColor: string, minorColor: string): GridHelper {
    const next = new GridHelper(200, GRID_DIVISIONS, majorColor, minorColor);
    next.rotation.x = Math.PI / 2;
    next.userData["majorColor"] = majorColor;
    next.userData["minorColor"] = minorColor;
    next.userData["colorSignature"] = gridColorSignature(next);
    if (Array.isArray(next.material)) {
      for (const material of next.material) {
        material.transparent = true;
        material.opacity = 0.58;
      }
    } else {
      next.material.transparent = true;
      next.material.opacity = 0.58;
    }
    return next;
  }

  function disposeGridMaterial(material: GridHelper["material"]): void {
    if (Array.isArray(material)) {
      for (const item of material) item.dispose();
      return;
    }
    material.dispose();
  }

  function gridColorSignature(helper: GridHelper): string {
    const colors = helper.geometry.getAttribute("color");
    const centerVertex = (GRID_DIVISIONS / 2) * 4;
    return `${gridColorHex(colors, centerVertex)}|${gridColorHex(colors, 0)}`;
  }

  function gridColorHex(
    colors: {
      getX(index: number): number;
      getY(index: number): number;
      getZ(index: number): number;
    },
    index: number,
  ): string {
    const r = Math.round(colors.getX(index) * 255);
    const g = Math.round(colors.getY(index) * 255);
    const b = Math.round(colors.getZ(index) * 255);
    return `#${hexByte(r)}${hexByte(g)}${hexByte(b)}`;
  }

  function hexByte(value: number): string {
    return Math.max(0, Math.min(255, value)).toString(16).padStart(2, "0");
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
        pendingFrame = null;
        if (opts?.frame !== false) autoFramePreset = null;
      }
      if (!payload || payload.positions.length === 0) {
        updateSceneScale(null);
        clearMetricsDataset();
        syncPointLight();
        syncCanvasDataset();
        render();
        return null;
      }
      const info = computeMeshInfo(payload.positions, payload.indices);
      if (!info) {
        updateSceneScale(null);
        clearMetricsDataset();
        syncPointLight();
        syncCanvasDataset();
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
      if (opts?.frame !== false) {
        frameToInfo(info, opts?.preset ?? "iso");
      }
      applyOptions();
      render();
      return info;
    },
    setCamera(state) {
      autoFramePreset = null;
      applyCamera(state);
      emitCamera();
    },
    frameCamera(preset) {
      if (meshInfo) {
        frameToInfo(meshInfo, preset);
        applyOptions();
        return;
      }
      autoFramePreset = preset;
      applyCamera(PRESET_STATES[preset]);
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
      viewportDpr = Math.max(0, dpr);
      viewportReady =
        Number.isFinite(width) &&
        Number.isFinite(height) &&
        Number.isFinite(dpr) &&
        width > 0 &&
        height > 0 &&
        dpr > 0;
      renderer.setPixelRatio(Math.max(1, Math.min(dpr, 2)));
      renderer.setSize(w, h, false);
      if (meshInfo) updateSceneScale(meshInfo);
      syncClipPlanes();
      if (pendingFrame && meshRenderInputsReady(pendingFrame.info, currentViewport())) {
        frameToInfo(pendingFrame.info, pendingFrame.preset);
      } else if (autoFramePreset && meshInfo && meshRenderInputsReady(meshInfo, currentViewport())) {
        frameToInfo(meshInfo, autoFramePreset);
      }
      applyOptions();
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
      disposeGridMaterial(grid.material);
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
  const typed = meshFromTypedPayload(outer);
  if (typed) return typed;
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

function meshFromTypedPayload(inner: Record<string, unknown>): MeshPayload | null {
  const positions = inner["positions"];
  if (!(positions instanceof Float32Array)) return null;
  const normals = inner["normals"];
  const vertexColors = inner["vertex_colors"];
  const indices = inner["indices"];
  return {
    positions,
    normals: normals instanceof Float32Array && normals.length > 0 ? normals : null,
    vertexColors:
      vertexColors instanceof Float32Array && vertexColors.length > 0
        ? vertexColors
        : null,
    indices: indices instanceof Uint32Array && indices.length > 0 ? indices : null,
  };
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
