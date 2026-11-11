//! The Meshy API (https://api.meshy.ai): image-to-image and text-to-image tasks, their status,
//! downloads and the account balance. Blocking calls, made from the art jobs' worker threads.

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

const BASE: &str = "https://api.meshy.ai";

/// The models offered, with their price per picture in credits (Meshy's pricing; the task
/// reports what it really cost).
pub const MODELS: [(&str, &str, u32); 5] = [
    ("nano-banana-2-lite", "Nano Banana 2 Lite (cheapest)", 3),
    ("nano-banana-2-1", "Nano Banana 2.1", 4),
    ("nano-banana-2", "Nano Banana 2", 6),
    ("nano-banana-pro", "Nano Banana Pro (Pro plan)", 9),
    ("gpt-image-2", "GPT Image 2", 12),
];

pub fn model_cost(model: &str) -> u32 {
    MODELS.iter().find(|m| m.0 == model).map_or(12, |m| m.2)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(120))).http_status_as_error(false).build().into()
}

/// A request's JSON answer; HTTP errors become messages (with Meshy's own text, never the key).
fn answer(resp: Result<ureq::http::Response<ureq::Body>, ureq::Error>, what: &str) -> Result<Value> {
    let mut resp = resp.map_err(|e| anyhow!("{what}: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().with_config().limit(4 << 20).read_to_string().unwrap_or_default();
    if status >= 400 {
        let msg = serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_string)).unwrap_or(text);
        bail!(match status {
            401 => "Meshy refused the API key (401). Check it in Edit › Settings.".to_string(),
            402 => format!("Meshy: not enough credits, or the model needs another plan (402): {msg}"),
            429 => format!("Meshy is busy with your other pictures (429): {msg}"),
            _ => format!("{what}: HTTP {status}: {msg}"),
        });
    }
    serde_json::from_str(&text).with_context(|| format!("{what}: not JSON"))
}

/// The account's credits.
pub fn balance(key: &str) -> Result<u64> {
    let v = answer(agent().get(format!("{BASE}/openapi/v1/balance")).header("Authorization", format!("Bearer {key}")).call(), "balance")?;
    v.get("balance").and_then(Value::as_u64).ok_or_else(|| anyhow!("balance: no number in the answer"))
}

/// What a picture is made from.
#[derive(Clone, Debug)]
pub enum Request {
    /// Reference pictures (data URIs) and a prompt: matches the references' style.
    FromImages { prompt: String, refs: Vec<String>, aspect: String },
    /// A prompt only; `background`: keep the background (painted pictures).
    FromText { prompt: String, aspect: String, background: bool },
}

/// Starts a task; returns its id.
pub fn start(key: &str, model: &str, req: &Request) -> Result<String> {
    let (path, body) = match req {
        Request::FromImages { prompt, refs, aspect } => ("image-to-image", json!({ "ai_model": model, "prompt": prompt, "reference_image_urls": refs, "aspect_ratio": aspect, "remove_background": true })),
        Request::FromText { prompt, aspect, background } => ("text-to-image", json!({ "ai_model": model, "prompt": prompt, "aspect_ratio": aspect, "remove_background": !background })),
    };
    let v = answer(agent().post(format!("{BASE}/openapi/v1/{path}")).header("Authorization", format!("Bearer {key}")).send_json(&body), "starting a picture")?;
    v.get("result").and_then(Value::as_str).map(str::to_string).ok_or_else(|| anyhow!("starting a picture: no task id in the answer"))
}

/// A task's state.
#[derive(Clone, Debug, Default)]
pub struct Task {
    pub status: String,
    pub progress: u32,
    pub url: Option<String>,
    pub credits: u32,
    pub error: String,
}

pub fn task(key: &str, image_to_image: bool, id: &str) -> Result<Task> {
    let path = if image_to_image { "image-to-image" } else { "text-to-image" };
    let v = answer(agent().get(format!("{BASE}/openapi/v1/{path}/{id}")).header("Authorization", format!("Bearer {key}")).call(), "checking a picture")?;
    Ok(Task {
        status: v.get("status").and_then(Value::as_str).unwrap_or("PENDING").to_string(),
        progress: v.get("progress").and_then(Value::as_u64).unwrap_or(0) as u32,
        url: v.get("image_urls").and_then(|u| u.get(0)).and_then(Value::as_str).map(str::to_string),
        credits: v.get("consumed_credits").and_then(Value::as_u64).unwrap_or(0) as u32,
        error: v.get("task_error").and_then(|e| e.get("message")).and_then(Value::as_str).unwrap_or_default().to_string(),
    })
}

/// Downloads a finished picture.
pub fn download(url: &str) -> Result<Vec<u8>> {
    let mut resp = agent().get(url).call().map_err(|e| anyhow!("downloading the picture: {e}"))?;
    if resp.status().as_u16() >= 400 {
        bail!("downloading the picture: HTTP {}", resp.status());
    }
    resp.body_mut().with_config().limit(64 << 20).read_to_vec().context("downloading the picture")
}

/// A PNG as a data URI (reference pictures).
pub fn data_uri(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", rustpg2d_engine::hash::base64(png))
}
