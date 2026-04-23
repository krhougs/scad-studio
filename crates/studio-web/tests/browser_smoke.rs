#![cfg(all(target_arch = "wasm32", feature = "browser-smoke"))]

use gloo_timers::future::TimeoutFuture;
use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

const SMOKE_WS_URL: &str = "ws://127.0.0.1:39180";

#[wasm_bindgen_test(async)]
async fn browser_smoke_loads_workspace_listing_and_preview() {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should exist");
    document.set_title("studio-web browser smoke");
    if document.get_element_by_id("studio-web-root").is_none() {
        let root = document.create_element("div").expect("create smoke root");
        root.set_id("studio-web-root");
        document
            .body()
            .expect("body should exist")
            .append_child(&root)
            .expect("append smoke root");
    }

    studio_web::boot_studio_web(SMOKE_WS_URL).expect("boot wasm app");

    wait_for_text("studio-web wasm shell").await;
    wait_for_text("README.md").await;
    wait_for_text("model.stl").await;
    wait_for_text("preview ready").await;
}

#[wasm_bindgen_test(async)]
async fn browser_smoke_mounts_mesh_preview_canvas_when_mesh_arrives() {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should exist");
    document.set_title("studio-web browser smoke mesh canvas");
    if document.get_element_by_id("studio-web-root").is_none() {
        let root = document.create_element("div").expect("create smoke root");
        root.set_id("studio-web-root");
        document
            .body()
            .expect("body should exist")
            .append_child(&root)
            .expect("append smoke root");
    }

    assert_webgpu_adapter_available().await;

    studio_web::boot_studio_web(SMOKE_WS_URL).expect("boot wasm app");

    wait_for_text("preview ready").await;
    wait_for_text("vertices:").await;
    let canvas = wait_for_element("preview-mesh-canvas")
        .await
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("preview canvas should be an HtmlCanvasElement");
    wait_for_attribute("preview-mesh-canvas", "data-preview-render-state", "ready").await;

    let summary = wait_for_element_text("preview-summary", "vertices:").await;
    let (vertex_count, index_count, triangle_count) = parse_preview_summary_counts(&summary);
    assert!(
        vertex_count > 0,
        "expected non-empty mesh vertex count, got summary: {summary}"
    );
    assert!(
        index_count > 0,
        "expected non-empty mesh index count, got summary: {summary}"
    );
    assert!(
        triangle_count > 0,
        "expected non-empty mesh triangle count, got summary: {summary}"
    );

    wait_for_non_blank_canvas(&canvas).await;
}

async fn wait_for_text(expected: &str) {
    for _ in 0..120 {
        let text = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.body())
            .and_then(|body| body.text_content())
            .unwrap_or_default();
        if text.contains(expected) {
            return;
        }
        TimeoutFuture::new(50).await;
    }
    panic!("expected text not found: {expected}");
}

async fn wait_for_element(element_id: &str) -> web_sys::Element {
    for _ in 0..120 {
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(element_id))
        {
            return element;
        }
        TimeoutFuture::new(50).await;
    }
    panic!("expected element not found: {element_id}");
}

async fn wait_for_element_text(element_id: &str, expected: &str) -> String {
    for _ in 0..120 {
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(element_id))
        {
            let text = element.text_content().unwrap_or_default();
            if text.contains(expected) {
                return text;
            }
        }
        TimeoutFuture::new(50).await;
    }
    panic!("expected text not found in element {element_id}: {expected}");
}

async fn wait_for_attribute(element_id: &str, attribute: &str, expected: &str) {
    for _ in 0..120 {
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(element_id))
            && element
                .get_attribute(attribute)
                .as_deref()
                .is_some_and(|value| value == expected)
        {
            return;
        }
        TimeoutFuture::new(50).await;
    }
    panic!("expected attribute {attribute} on {element_id} to become {expected}");
}

async fn assert_webgpu_adapter_available() {
    let window = web_sys::window().expect("window should exist");
    let navigator = Reflect::get(window.as_ref(), &JsValue::from_str("navigator"))
        .expect("navigator should be readable");
    let gpu = Reflect::get(&navigator, &JsValue::from_str("gpu"))
        .expect("navigator.gpu should be readable");
    assert!(
        !gpu.is_null() && !gpu.is_undefined(),
        "expected navigator.gpu to exist in smoke browser"
    );

    let request_adapter = Reflect::get(&gpu, &JsValue::from_str("requestAdapter"))
        .expect("gpu.requestAdapter should be readable")
        .dyn_into::<Function>()
        .expect("gpu.requestAdapter should be a function");
    let promise = request_adapter
        .call0(&gpu)
        .expect("requestAdapter should return a promise")
        .dyn_into::<Promise>()
        .expect("requestAdapter should return a Promise");
    let adapter = JsFuture::from(promise)
        .await
        .expect("requestAdapter promise should resolve");
    assert!(
        !adapter.is_null() && !adapter.is_undefined(),
        "expected WebGPU adapter to be available in smoke browser"
    );
}

fn parse_preview_summary_counts(summary: &str) -> (usize, usize, usize) {
    let values = summary.split('·').map(extract_count).collect::<Vec<_>>();
    assert!(
        values.len() == 3,
        "expected preview summary to expose three mesh counts, got: {summary}"
    );
    (values[0], values[1], values[2])
}

fn extract_count(segment: &str) -> usize {
    segment
        .split(':')
        .nth(1)
        .expect("summary segment should contain ':'")
        .trim()
        .parse::<usize>()
        .expect("summary count should parse as usize")
}

fn count_non_clear_samples(source: &web_sys::HtmlCanvasElement) -> usize {
    let (pixels, width, height) = read_canvas_pixels_via_2d_copy(source);
    let mut non_clear_samples = 0;

    for y in (0..height).step_by(8) {
        for x in (0..width).step_by(8) {
            let index = (y * width + x) * 4;
            let red = pixels[index];
            let green = pixels[index + 1];
            let blue = pixels[index + 2];
            let alpha = pixels[index + 3];
            if alpha > 0 && differs_from_clear_color(red, green, blue) {
                non_clear_samples += 1;
            }
        }
    }

    non_clear_samples
}

fn read_canvas_pixels_via_2d_copy(source: &web_sys::HtmlCanvasElement) -> (Vec<u8>, usize, usize) {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should exist for canvas sampling");
    let scratch = document
        .create_element("canvas")
        .expect("create scratch canvas")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("scratch canvas should cast to HtmlCanvasElement");
    scratch.set_width(source.width().max(1));
    scratch.set_height(source.height().max(1));

    let context = scratch
        .get_context("2d")
        .expect("scratch canvas 2d context lookup should succeed")
        .expect("scratch canvas should provide a 2d context")
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .expect("scratch 2d context should cast correctly");
    context
        .draw_image_with_html_canvas_element(source, 0.0, 0.0)
        .expect("copy preview canvas into scratch canvas");

    let image_data = context
        .get_image_data(0.0, 0.0, scratch.width() as f64, scratch.height() as f64)
        .expect("read scratch canvas pixels");
    let pixels = image_data.data().to_vec();
    let width = scratch.width() as usize;
    let height = scratch.height() as usize;
    (pixels, width, height)
}

fn canvas_data_url_differs_from_uniform_reference(source: &web_sys::HtmlCanvasElement) -> bool {
    let (pixels, width, height) = read_canvas_pixels_via_2d_copy(source);
    let reference_red = pixels[0];
    let reference_green = pixels[1];
    let reference_blue = pixels[2];
    let reference_alpha = pixels[3] as f64 / 255.0;

    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should exist for reference canvas");
    let reference = document
        .create_element("canvas")
        .expect("create reference canvas")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("reference canvas should cast to HtmlCanvasElement");
    reference.set_width(width as u32);
    reference.set_height(height as u32);

    let context = reference
        .get_context("2d")
        .expect("reference canvas 2d context lookup should succeed")
        .expect("reference canvas should provide a 2d context")
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .expect("reference 2d context should cast correctly");
    context.set_fill_style_str(&format!(
        "rgba({reference_red}, {reference_green}, {reference_blue}, {reference_alpha})"
    ));
    context.fill_rect(0.0, 0.0, width as f64, height as f64);

    let source_data_url = source
        .to_data_url()
        .expect("preview canvas should serialize to a data URL");
    let reference_data_url = reference
        .to_data_url()
        .expect("reference canvas should serialize to a data URL");
    source_data_url != reference_data_url
}

async fn wait_for_non_blank_canvas(source: &web_sys::HtmlCanvasElement) {
    let mut observed_samples = Vec::new();
    let mut observed_data_url_diffs = Vec::new();
    for _ in 0..20 {
        let non_clear_samples = count_non_clear_samples(source);
        let data_url_differs = canvas_data_url_differs_from_uniform_reference(source);
        observed_samples.push(non_clear_samples);
        observed_data_url_diffs.push(data_url_differs);
        if non_clear_samples > 50 || data_url_differs {
            return;
        }
        TimeoutFuture::new(100).await;
    }
    panic!(
        "expected rendered canvas to differ from a uniform clear surface, observed sampled counts: {:?}, observed data-url diffs: {:?}",
        observed_samples, observed_data_url_diffs
    );
}

fn differs_from_clear_color(red: u8, green: u8, blue: u8) -> bool {
    red.abs_diff(18) + green.abs_diff(23) + blue.abs_diff(31) > 18
}

#[wasm_bindgen_test(async)]
async fn browser_smoke_directory_tree_expand_and_navigate() {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should exist");
    document.set_title("studio-web browser smoke tree");
    if document.get_element_by_id("studio-web-root").is_none() {
        let root = document.create_element("div").expect("create smoke root");
        root.set_id("studio-web-root");
        document
            .body()
            .expect("body should exist")
            .append_child(&root)
            .expect("append smoke root");
    }

    studio_web::boot_studio_web(SMOKE_WS_URL).expect("boot wasm app");

    wait_for_text("README.md").await;
    wait_for_text("examples").await;

    wait_for_element("tree-toggle-0").await;
    let dir_button = wait_for_element("tree-dir-0").await;
    dir_button
        .dyn_into::<web_sys::HtmlElement>()
        .expect("cast to HtmlElement")
        .click();

    wait_for_text("notes.txt").await;

    let toggle = wait_for_element("tree-toggle-0").await;
    let toggle_text = toggle.text_content().unwrap_or_default();
    assert!(
        toggle_text.contains('\u{25BC}'),
        "examples should show expanded toggle after click, got: {toggle_text}"
    );
}
