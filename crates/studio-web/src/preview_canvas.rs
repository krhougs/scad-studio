#![cfg(target_arch = "wasm32")]

use wasm_bindgen::JsValue;
use web_sys::{Document, HtmlCanvasElement};

use scad_scene::{EguiPaintData, MeshData, OrbitalCamera, RenderSettings, Renderer};

pub(crate) const PREVIEW_MESH_CANVAS_ID: &str = "preview-mesh-canvas";
const PREVIEW_MESH_STATUS_ID: &str = "preview-mesh-status";
const PREVIEW_CANVAS_WIDTH: u32 = 640;
const PREVIEW_CANVAS_HEIGHT: u32 = 480;

#[derive(Debug, Default, Clone)]
pub(crate) struct PreviewCanvasState {
    mesh: Option<MeshData>,
    render_id: u64,
}

impl PreviewCanvasState {
    pub(crate) fn set_mesh(&mut self, mesh: MeshData) {
        self.mesh = Some(mesh);
    }

    pub(crate) fn clear(&mut self) {
        self.mesh = None;
    }

    pub(crate) fn html(&self) -> String {
        if self.mesh.is_none() {
            return "<p class=\"studio-web-empty\">Mesh canvas appears after a 3D preview is ready.</p>".into();
        }

        format!(
            r#"
            <div class="studio-web-preview-surface">
              <canvas
                id="{PREVIEW_MESH_CANVAS_ID}"
                class="studio-web-preview-canvas"
                width="{PREVIEW_CANVAS_WIDTH}"
                height="{PREVIEW_CANVAS_HEIGHT}"
                data-preview-render-state="queued"
              ></canvas>
              <p id="{PREVIEW_MESH_STATUS_ID}" class="studio-web-preview-status">mesh render queued</p>
            </div>
            "#
        )
    }

    pub(crate) fn next_render(&mut self) -> Option<(u64, MeshData)> {
        let mesh = self.mesh.clone()?;
        self.render_id = self.render_id.wrapping_add(1);
        Some((self.render_id, mesh))
    }

    pub(crate) fn matches_render(&self, render_id: u64) -> bool {
        self.mesh.is_some() && self.render_id == render_id
    }
}

pub(crate) fn set_preview_canvas_dom_state(
    document: &Document,
    state: &str,
    message: &str,
) -> Result<(), JsValue> {
    if let Some(canvas) = document.get_element_by_id(PREVIEW_MESH_CANVAS_ID) {
        canvas.set_attribute("data-preview-render-state", state)?;
    }
    if let Some(status) = document.get_element_by_id(PREVIEW_MESH_STATUS_ID) {
        status.set_text_content(Some(message));
    }
    Ok(())
}

pub(crate) async fn render_mesh_preview(
    canvas: HtmlCanvasElement,
    mesh: MeshData,
) -> Result<(), String> {
    let mut renderer = Renderer::new_for_canvas(canvas)
        .await
        .map_err(|error| error.to_string())?;
    let mut camera = OrbitalCamera::new(renderer.aspect_ratio());
    camera.reset_view(Some(mesh.bounds));
    renderer.set_mesh(mesh);
    renderer
        .render(
            &camera,
            &RenderSettings::default(),
            None,
            None,
            EguiPaintData::empty(),
        )
        .map_err(|error| error.to_string())
}
