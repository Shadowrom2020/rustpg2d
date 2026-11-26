//! The browser's side of the web game: the clock, the console, storage, the page's address, the
//! gamepad, downloads and fetching files. (What the JavaScript files beside the game did while it
//! was loaded by macroquad's loader.)

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::Error;

fn window() -> web_sys::Window {
    web_sys::window().expect("a browser window")
}

/// Seconds since the page was opened.
pub fn now() -> f64 {
    window().performance().map_or(0.0, |p| p.now() / 1000.0)
}

/// Seconds since 1970.
pub fn unix_now() -> f64 {
    js_sys::Date::now() / 1000.0
}

/// A line for the browser's console.
pub fn log(level: &str, text: &str) {
    let v = JsValue::from_str(text);
    match level {
        "error" => web_sys::console::error_1(&v),
        "warn" => web_sys::console::warn_1(&v),
        _ => web_sys::console::log_1(&v),
    }
}

/// Keys of this game in `localStorage`: each game on a site (by the page's folder) has its own.
fn prefix() -> String {
    let path = window().location().pathname().unwrap_or_default();
    let folder = path.rfind('/').map_or("", |i| &path[..=i]);
    format!("rustpg2d:{folder}:")
}

fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

/// Keeps a text under `key` (the game's written files, as base64).
pub fn storage_set(key: &str, value: &str) {
    if let Some(s) = storage() {
        if s.set_item(&format!("{}{key}", prefix()), value).is_err() {
            log("error", &format!("R(ust)PG2D: could not store {key}"));
        }
    }
}

pub fn storage_remove(key: &str) {
    if let Some(s) = storage() {
        let _ = s.remove_item(&format!("{}{key}", prefix()));
    }
}

/// Everything this game stored: (key, text).
pub fn storage_dump() -> Vec<(String, String)> {
    let Some(s) = storage() else {
        log("warn", "R(ust)PG2D: no browser storage, nothing will be saved");
        return Vec::new();
    };
    let p = prefix();
    let mut out = Vec::new();
    for i in 0..s.length().unwrap_or(0) {
        let Ok(Some(k)) = s.key(i) else { continue };
        if let Some(rest) = k.strip_prefix(&p) {
            if let Ok(Some(v)) = s.get_item(&k) {
                out.push((rest.to_string(), v));
            }
        }
    }
    out
}

/// The page's query string without `?`, URL-decoded.
pub fn query() -> String {
    let raw = window().location().search().unwrap_or_default();
    let raw = raw.trim_start_matches('?').replace('+', " ");
    js_sys::decode_uri_component(&raw).map(String::from).unwrap_or(raw)
}

/// The browser's language (`navigator.language`, e.g. "de-DE").
pub fn language() -> String {
    window().navigator().language().unwrap_or_default()
}

fn gamepad() -> Option<web_sys::Gamepad> {
    let pads = window().navigator().get_gamepads().ok()?;
    pads.iter().filter_map(|p| p.dyn_into::<web_sys::Gamepad>().ok()).find(|p| p.connected())
}

/// The first connected gamepad (a pad shows up after its first button press): its buttons in the
/// "standard" layout as bits 0-15 (A B X Y, LB RB LT RT, View Menu, stick clicks, d-pad) and its
/// left stick (x, y; y grows downwards).
pub fn pad_state() -> (u32, f32, f32) {
    let Some(p) = gamepad() else { return (0, 0.0, 0.0) };
    let mut bits = 0u32;
    for (i, b) in p.buttons().iter().take(16).enumerate() {
        if let Ok(b) = b.dyn_into::<web_sys::GamepadButton>() {
            if b.pressed() || b.value() > 0.5 {
                bits |= 1 << i;
            }
        }
    }
    let axes = p.axes();
    let axis = |i: u32| axes.get(i).as_f64().unwrap_or(0.0) as f32;
    (bits, axis(0), axis(1))
}

/// The first connected gamepad's name ("" = none).
pub fn pad_id() -> String {
    gamepad().map(|p| p.id()).unwrap_or_default()
}

/// Offers `data` as a download named `name` (photos): a link that is clicked at once.
pub fn download(name: &str, data: &[u8]) {
    let run = || -> Result<(), JsValue> {
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(data));
        let bag = web_sys::BlobPropertyBag::new();
        bag.set_type("image/png");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &bag)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let document = window().document().ok_or(JsValue::NULL)?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
        a.set_href(&url);
        a.set_download(name);
        let body = document.body().ok_or(JsValue::NULL)?;
        body.append_child(&a)?;
        a.click();
        a.remove();
        // (The link's data is freed with the page.)
        Ok(())
    };
    if run().is_err() {
        log("error", &format!("R(ust)PG2D: could not offer {name}"));
    }
}

/// Fetches a file beside the page.
pub async fn fetch(path: &str) -> Result<Vec<u8>, Error> {
    let fail = |what: &str| Error::File(format!("{path}: {what}"));
    let resp = wasm_bindgen_futures::JsFuture::from(window().fetch_with_str(path)).await.map_err(|_| fail("the download failed"))?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| fail("no answer"))?;
    if !resp.ok() {
        return Err(fail(&format!("the server answered {}", resp.status())));
    }
    let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer().map_err(|_| fail("no data"))?).await.map_err(|_| fail("the download broke off"))?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

/// The canvas the game draws on: the page's `<canvas id="glcanvas">`.
pub(crate) fn canvas() -> Option<web_sys::HtmlCanvasElement> {
    window().document()?.get_element_by_id("glcanvas")?.dyn_into().ok()
}

/// Tells the player (a box of the browser): the game can't start.
pub fn alert(text: &str) {
    let _ = window().alert_with_message(text);
}
