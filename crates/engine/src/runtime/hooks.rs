//! Script hooks at runtime (`crate::hooks`): [`Game::hook`] calls the functions registered for a
//! hook with a map and returns the map they changed. The call sites are where the engine decides
//! something (a hit's damage, a price, an item use…); `status` prints `hooks=` (calls per hook).

use super::Game;

impl Game {
    /// Whether `name` has functions: call sites that would build a costly map check first.
    pub(super) fn hooked(&self, name: &str) -> bool {
        self.hooks.has(name)
    }

    /// Runs the functions of hook `name` on `map` (each gets what the one before returned) and
    /// returns the result; the actions they queue run after the current ones. Without functions
    /// the map comes back as it was, at no cost.
    pub(super) fn hook(&mut self, name: &str, mut map: rhai::Map) -> rhai::Map {
        if !self.hooks.has(name) {
            return map;
        }
        let functions = self.hooks.get(name).to_vec();
        self.sync_script_player();
        for f in functions {
            *self.hook_calls.entry(name.to_string()).or_insert(0) += 1;
            let (changed, actions) = self.scripts.call_hook(&mut self.world, &f, map.clone());
            if let Some(m) = changed {
                map = m;
            }
            if !actions.is_empty() {
                self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
            }
        }
        map
    }

    /// The `menu_open` script hook: whether `menu` ("pause", "character", "journal"…) may open.
    pub(super) fn menu_allowed(&mut self, menu: &str) -> bool {
        if !self.hooks.has("menu_open") {
            return true;
        }
        let m = self.hook("menu_open", crate::hooks::map([("menu", menu.into()), ("cancel", false.into())]));
        !crate::hooks::flag(&m, "cancel")
    }

    /// `hooks=` for `status`: how often each hook ran its functions (`-` without hooks).
    pub(super) fn hooks_status(&self) -> String {
        if self.project.config.hooks.is_empty() {
            return "-".into();
        }
        let list: Vec<String> = self.hook_calls.iter().map(|(k, n)| format!("{k}:{n}")).collect();
        format!("[{}]", list.join(" "))
    }
}
