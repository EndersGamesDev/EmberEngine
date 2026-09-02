//! Non-panicking browser reporting for uncaptured wgpu errors.

use wasm_bindgen::JsValue;

pub fn publish_browser_error(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(message));
    if let Some(status) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("status"))
    {
        status.set_class_name("status failed");
        status.set_text_content(Some(message));
    }
}

pub fn install_logging_handler(device: &wgpu::Device, label: &'static str) {
    device.on_uncaptured_error(Box::new(move |error| {
        publish_browser_error(&format!("{label} uncaptured wgpu error: {error}"));
    }));
}
