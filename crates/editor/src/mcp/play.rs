//! Play-tests for agents: the main thread saves and builds the game command ([`plan`]), the
//! agent thread runs it ([`run`]) with the autoplay script, collects the output lines and the
//! screenshots the script took (`shot NAME` → PNG files in a scratch folder the game runs in),
//! and stops the game after the timeout.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use crate::app::Editor;

use super::exec::Reply;
use super::tools::PlayParams;

/// Screenshots per play-test at most, lines kept, image size limit.
pub const MAX_SHOTS: usize = 6;
pub const MAX_LINES: usize = 200;
pub const MAX_IMAGE_SIDE: u32 = 1280;

/// A game run ready to start.
pub struct PlayPlan {
    pub cmd: std::process::Command,
    /// The scratch folder the game runs in (screenshots land there); removed afterwards.
    pub dir: PathBuf,
    /// Screenshot files the script will write, in order.
    pub shots: Vec<PathBuf>,
    pub timeout: Duration,
    pub warnings: Vec<String>,
}

static RUNS: AtomicU32 = AtomicU32::new(0);

/// Rewrites `shot NAME` commands to files in `dir` (relative names: the game runs there, so
/// paths with spaces can't break the script) and appends `quit`.
pub fn rewrite_autoplay(script: &str, dir: &Path) -> Result<(String, Vec<PathBuf>), String> {
    let mut cmds = Vec::new();
    let mut shots = Vec::new();
    for raw in script.split(';') {
        let cmd = raw.trim();
        if cmd.is_empty() {
            continue;
        }
        let mut words = cmd.split_whitespace();
        if words.next() == Some("shot") {
            if shots.len() >= MAX_SHOTS {
                return Err(format!("at most {MAX_SHOTS} screenshots per play-test"));
            }
            let name: String = words.collect::<Vec<_>>().join("").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(24).collect();
            let file = format!("{}-{}.png", shots.len() + 1, if name.is_empty() { "shot".to_string() } else { name });
            shots.push(dir.join(&file));
            cmds.push(format!("shot {file}"));
        } else {
            cmds.push(cmd.to_string());
        }
    }
    if !matches!(cmds.last().map(|c| c.split_whitespace().next().unwrap_or("")), Some("quit")) {
        cmds.push("quit".into());
    }
    Ok((cmds.join("; "), shots))
}

/// Main thread: saves, finds the game runner and builds the command (like F5's play-test).
pub fn plan(ed: &mut Editor, p: PlayParams) -> Result<PlayPlan, String> {
    ed.mcp_writable()?;
    ed.save_all();
    ed.watch_resnapshot();
    let runner = rustpg2d_engine::release::Platform::host().game_runner();
    let exe = std::env::current_exe().ok().map(|e| e.with_file_name(runner)).filter(|e| e.is_file()).ok_or_else(|| format!("{runner} (the game) was not found next to the editor; from source run `cargo build -p rustpg2d-game`"))?;
    let n = RUNS.fetch_add(1, Ordering::Relaxed) + 1;
    let dir = std::env::temp_dir().join(format!("rustpg2d-mcp-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let (script, shots) = rewrite_autoplay(&p.script, &dir)?;
    let mut warnings = Vec::new();
    let mut cmd = std::process::Command::new(exe);
    cmd.arg(ed.project.root.canonicalize().unwrap_or_else(|_| ed.project.root.clone()));
    cmd.arg("--dev");
    if !p.title.unwrap_or(false) {
        cmd.arg("--no-title");
    }
    if let Some(l) = p.lang.as_deref().filter(|l| !l.is_empty()) {
        cmd.args(["--lang", l]);
    }
    if !p.from_start.unwrap_or(false) {
        let map = match p.map.clone().or_else(|| ed.doc.as_ref().map(|d| d.map.name.clone())) {
            Some(m) => m,
            None => return Err("no map is open: give `map` or set `from_start`".into()),
        };
        if !ed.maps.contains(&map) {
            return Err(format!("\"{map}\" is not a map.{}", super::exec::suggestions(&map, ed.maps.iter().map(String::as_str))));
        }
        cmd.args(["--map", &map]);
        let spawns = ed.refs.spawns.get(&map).cloned().unwrap_or_default();
        let ts = ed.ts();
        match (p.spawn.as_deref(), p.pos) {
            (Some(s), _) => {
                if !spawns.contains(&s.to_string()) {
                    return Err(format!("\"{s}\" is not a spawn zone of {map}.{}", super::exec::suggestions(s, spawns.iter().map(String::as_str))));
                }
                cmd.args(["--spawn", s]);
            }
            (None, Some([x, y])) => {
                cmd.args(["--pos", &format!("{},{}", x as f32 * ts + ts / 2.0, y as f32 * ts + ts / 2.0)]);
            }
            (None, None) => {
                if spawns.iter().any(|s| s == "start") {
                    cmd.args(["--spawn", "start"]);
                } else if let Some(s) = spawns.first() {
                    cmd.args(["--spawn", s]);
                } else {
                    let m = ed.mcp_load_map(&map)?;
                    cmd.args(["--pos", &format!("{},{}", m.width as f32 * ts / 2.0, m.height as f32 * ts / 2.0)]);
                    warnings.push("the map has no spawn zone: started at its centre".into());
                }
            }
        }
    }
    cmd.env("RUSTPG2D_AUTOPLAY", script).env("RUSTPG2D_MUTE", "1");
    cmd.current_dir(&dir);
    let timeout = Duration::from_secs(p.timeout_secs.unwrap_or(60).clamp(5, 300) as u64);
    Ok(PlayPlan { cmd, dir, shots, timeout, warnings })
}

/// Agent thread: runs the game, waits (at most the timeout), gathers output and screenshots.
pub async fn run(plan: PlayPlan) -> Reply {
    use tokio::io::AsyncBufReadExt;
    let PlayPlan { cmd, dir, shots, timeout, mut warnings } = plan;
    let program = cmd.get_program().to_owned();
    // A game built from another engine version may not read this editor's data.
    if let Ok(out) = tokio::process::Command::new(&program).arg("--engine-build").output().await {
        if String::from_utf8_lossy(&out.stdout).trim() != rustpg2d_engine::ENGINE_BUILD {
            warnings.push("the game runner was built from a different engine version than the editor; if it fails, rebuild it (cargo build -p rustpg2d-game)".into());
        }
    }
    let mut cmd = tokio::process::Command::from(cmd);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let started = std::time::Instant::now();
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Reply::Text(format!("cannot start the game: {e}")),
    };
    async fn collect<R: tokio::io::AsyncRead + Unpin>(reader: Option<R>) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(r) = reader {
            let mut l = tokio::io::BufReader::new(r).lines();
            while let Ok(Some(line)) = l.next_line().await {
                lines.push(line);
            }
        }
        lines
    }
    let out_task = tokio::spawn(collect(child.stdout.take()));
    let err_task = tokio::spawn(collect(child.stderr.take()));
    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(s)) => format!("exit {}", s.code().map_or("(signal)".to_string(), |c| c.to_string())),
        Ok(Err(e)) => format!("wait failed: {e}"),
        Err(_) => {
            let _ = child.kill().await;
            format!("stopped after the timeout ({} s); add `quit` earlier or raise timeout_secs", timeout.as_secs())
        }
    };
    let mut lines: Vec<String> = out_task.await.unwrap_or_default();
    let err_lines: Vec<String> = err_task.await.unwrap_or_default();
    lines.extend(err_lines.into_iter().filter(|l| !l.trim().is_empty()).map(|l| format!("stderr: {l}")));
    let total = lines.len();
    let kept: Vec<String> = lines.into_iter().rev().take(MAX_LINES).map(|l| if l.chars().count() > 600 { format!("{}…", l.chars().take(600).collect::<String>()) } else { l }).collect::<Vec<_>>().into_iter().rev().collect();
    let mut text = format!("Game run: {status}, {:.1} s\n", started.elapsed().as_secs_f32());
    for w in &warnings {
        text += &format!("Warning: {w}\n");
    }
    if total > kept.len() {
        text += &format!("({} earlier lines dropped)\n", total - kept.len());
    }
    text += &kept.join("\n");
    let mut images = Vec::new();
    for (i, shot) in shots.iter().enumerate() {
        match std::fs::read(shot) {
            Ok(png) => match downscale(&png) {
                Ok(p) => {
                    images.push(p);
                    text += &format!("\nscreenshot {}: {}", i + 1, shot.file_name().unwrap_or_default().to_string_lossy());
                }
                Err(e) => text += &format!("\nscreenshot {} unreadable: {e}", i + 1),
            },
            Err(_) => text += &format!("\nscreenshot {} was not taken (the script didn't reach its `shot`)", i + 1),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    Reply::TextImages(text, images)
}

/// A PNG no larger than [`MAX_IMAGE_SIDE`] on either side (nearest neighbour: pixel art).
pub fn downscale(png: &[u8]) -> Result<Vec<u8>, String> {
    let img = rustpg2d_engine::imageio::Rgba::decode(png).map_err(|e| e.to_string())?;
    let side = img.w.max(img.h);
    if side <= MAX_IMAGE_SIDE {
        return Ok(png.to_vec());
    }
    let f = MAX_IMAGE_SIDE as f32 / side as f32;
    img.resized(((img.w as f32 * f) as u32).max(1), ((img.h as f32 * f) as u32).max(1)).encode().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autoplay_scripts_get_shot_files_and_a_quit() {
        let dir = Path::new("/tmp/x");
        let (s, shots) = rewrite_autoplay("wait 1; shot Title Screen!; press e;shot ; status", dir).unwrap();
        assert_eq!(s, "wait 1; shot 1-TitleScreen.png; press e; shot 2-shot.png; status; quit");
        assert_eq!(shots, vec![dir.join("1-TitleScreen.png"), dir.join("2-shot.png")]);
        let (s, _) = rewrite_autoplay("status; quit", dir).unwrap();
        assert_eq!(s, "status; quit");
        assert!(rewrite_autoplay(&"shot a;".repeat(7), dir).is_err());
    }
}
