//! Minimal browser client used to measure the complete fixture's WASM payload.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("PliegoRS client requires a browser document"))?;
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("PliegoRS client requires a document element"))?;
    root.set_attribute("data-pliego-wasm", "ready")
}
