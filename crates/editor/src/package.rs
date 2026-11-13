//! Linux packages of an exported game (File → Export game), built from the export folder
//! (`<slug>/<slug>` + `<slug>/data/`) with the shared `rustpg2d-packaging` crate:
//! `<slug>-x86_64.AppImage` and `<slug>.flatpak`. Inside both the game sits in `lib/<slug>/`
//! next to its `data/`, so it finds the game the same way as in the plain export; saves go to
//! the player's data folder (in Flatpak's sandbox: `~/.var/app/<app id>/data/<slug>/saves`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rustpg2d_engine::imageio::Rgba;
use rustpg2d_engine::project::Project;
use rustpg2d_engine::release::{DATA_DIR, ENGINE_LICENSE, ENGINE_LICENSE_FILE};
use rustpg2d_packaging::{AppSpec, Entry, Source};

pub use rustpg2d_packaging::{check_app_id, flatpak_available, glibc_needed, FLATPAK_RUNTIMES};

/// A Flatpak application id for a game: `org.rustpg2d.<slug with underscores>`.
pub fn default_app_id(slug: &str) -> String {
    let mut s: String = slug.chars().map(|c| if c == '-' { '_' } else { c }).collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert_str(0, "game_");
    }
    format!("org.rustpg2d.{s}")
}

/// The game's icon: a PNG chosen by the creator (any size), else the project's `icon.png` or
/// the player's portrait (`hero_icon`), else `model` (the player model's head and shoulders,
/// rendered by the editor), else R(ust)PG2D's emblem.
pub fn icon(project: &Project, custom: &str, model: &dyn Fn() -> Option<Rgba>) -> Result<Vec<u8>> {
    if !custom.trim().is_empty() {
        let p = PathBuf::from(custom.trim());
        let p = if p.is_absolute() { p } else { project.root.join(p) };
        let bytes = std::fs::read(&p).with_context(|| format!("reading the icon {}", p.display()))?;
        Rgba::decode(&bytes).with_context(|| format!("the icon {} isn't a PNG image", p.display()))?;
        return Ok(bytes);
    }
    rustpg2d_packaging::hero_icon(project).or_else(model).unwrap_or_else(rustpg2d_packaging::emblem).encode()
}

/// The package contents of an exported game.
fn game_spec(folder: &Path, slug: &str, name: &str, app_id: &str, runtime: &str, icon: &[u8]) -> Result<AppSpec> {
    let lib = format!("lib/{slug}");
    let mut spec = AppSpec {
        command: slug.into(),
        name: name.into(),
        comment: "A role-playing game made with R(ust)PG2D".into(),
        categories: "Game;RolePlaying;".into(),
        app_id: app_id.into(),
        runtime: runtime.into(),
        // Window (X11; Wayland desktops run it through XWayland), sound, and devices: the GPU and
        // gamepads (/dev/input; `--device=input` would need Flatpak 1.15.6+) - nothing else.
        finish_args: ["--share=ipc", "--socket=x11", "--socket=pulseaudio", "--device=all"].map(String::from).to_vec(),
        files: vec![
            Entry { dest: format!("{lib}/{slug}"), src: Source::Path(folder.join(slug)), exec: true },
            // The engine's license (the game runner is R(ust)PG2D); the game's own license is its creator's choice.
            Entry { dest: format!("share/licenses/{slug}/{ENGINE_LICENSE_FILE}"), src: Source::Bytes(ENGINE_LICENSE.as_bytes().to_vec()), exec: false },
        ],
        bin: vec![(slug.into(), format!("{lib}/{slug}"))],
        icon: icon.to_vec(),
        license: String::new(),
    };
    // The game's files: its data.pack, or the data folder of a loose export.
    let pack = folder.join(rustpg2d_engine::pack::PACK_FILE);
    if pack.is_file() {
        spec.files.push(Entry { dest: format!("{lib}/{}", rustpg2d_engine::pack::PACK_FILE), src: Source::Path(pack), exec: false });
    } else {
        spec.add_dir(&folder.join(DATA_DIR), &format!("{lib}/{DATA_DIR}"), &|_| false)?;
    }
    Ok(spec)
}

/// Builds `out_dir/<slug>-x86_64.AppImage` from the export `folder`.
pub fn appimage(folder: &Path, slug: &str, name: &str, icon: &[u8], out_dir: &Path) -> Result<PathBuf> {
    let path = out_dir.join(format!("{slug}-x86_64.AppImage"));
    rustpg2d_packaging::appimage(&game_spec(folder, slug, name, &default_app_id(slug), FLATPAK_RUNTIMES[0], icon)?, &path)?;
    Ok(path)
}

/// Builds `out_dir/<slug>.flatpak` from the export `folder`.
pub fn flatpak(folder: &Path, slug: &str, name: &str, app_id: &str, runtime: &str, icon: &[u8], out_dir: &Path) -> Result<PathBuf> {
    let path = out_dir.join(format!("{slug}.flatpak"));
    rustpg2d_packaging::flatpak_bundle(&game_spec(folder, slug, name, app_id, runtime, icon)?, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_ids() {
        assert_eq!(default_app_id("knight-tales"), "org.rustpg2d.knight_tales");
        assert_eq!(default_app_id("2d-quest"), "org.rustpg2d.game_2d_quest");
        assert!(check_app_id(&default_app_id("knight-tales")).is_ok());
    }

    #[test]
    fn icons() {
        use rustpg2d_engine::defs::Database;
        use rustpg2d_engine::project::ProjectConfig;
        let tmp = std::env::temp_dir().join(format!("rustpg2d_icon_test_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let p = Project { root: tmp.clone(), config: ProjectConfig::new("Test"), db: Database::default() };
        // No player character: the emblem.
        let png = icon(&p, "", &|| None).unwrap();
        let img = Rgba::decode(&png).unwrap();
        assert_eq!((img.w, img.h), (rustpg2d_packaging::ICON, rustpg2d_packaging::ICON));
        // The player model's head and shoulders when the editor could render it.
        let mut bust = Rgba::new(8, 8);
        bust.set(3, 3, [200, 100, 50, 255]);
        let png = icon(&p, "", &|| Some(bust.clone())).unwrap();
        assert_eq!(Rgba::decode(&png).unwrap().get(3, 3), [200, 100, 50, 255]);
        // A picture of the creator's choice is used as it is (relative to the project).
        let own = Rgba::new(64, 64).encode().unwrap();
        std::fs::write(tmp.join("icon.png"), &own).unwrap();
        assert_eq!(icon(&p, "icon.png", &|| None).unwrap(), own);
        // The project's icon.png (an export's) comes before the model.
        assert_eq!(Rgba::decode(&icon(&p, "", &|| Some(bust.clone())).unwrap()).unwrap().w, 64);
        std::fs::write(tmp.join("bad.png"), b"not a png").unwrap();
        assert!(icon(&p, "bad.png", &|| None).is_err());
        assert!(icon(&p, "missing.png", &|| None).is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn game_layout() {
        let tmp = std::env::temp_dir().join(format!("rustpg2d_game_spec_test_{}", std::process::id()));
        let folder = tmp.join("knight-tales");
        std::fs::create_dir_all(folder.join("data/maps")).unwrap();
        std::fs::write(folder.join("knight-tales"), b"\x7fELF").unwrap();
        std::fs::write(folder.join("data/maps/start.map"), b"map").unwrap();
        std::fs::write(folder.join("README.txt"), b"readme").unwrap();
        let s = game_spec(&folder, "knight-tales", "Knight Tales", "org.rustpg2d.knight_tales", "25.08", b"png").unwrap();
        let dests: Vec<(&str, bool)> = s.files.iter().map(|e| (e.dest.as_str(), e.exec)).collect();
        assert_eq!(
            dests,
            [("lib/knight-tales/knight-tales", true), ("share/licenses/knight-tales/rustpg2d-LICENSE.txt", false), ("lib/knight-tales/data/maps/start.map", false)],
            "the game, the engine's license and the data; no README"
        );
        assert_eq!(s.bin, [("knight-tales".to_string(), "lib/knight-tales/knight-tales".to_string())]);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
