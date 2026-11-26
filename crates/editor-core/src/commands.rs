//! The commands of the editor: everything that can be done by name. The menus, the command
//! palette, shortcuts, the editor script and tests all run commands from this one list
//! ([`COMMANDS`]) through [`Workspace::run`], so a thing behaves the same wherever it is asked
//! for. What a command changes becomes one entry of the history under the command's title.
//!
//! (Agent tools are a list of their own, `mcp/tools.rs`: they describe themselves to a model
//! and take structured parameters. They change the project through the same workspace.)

use rustpg2d_engine::refactor::RefKind;

use crate::workspace::Workspace;

/// One command.
pub struct Command {
    /// `group.name`, typed in scripts and shown in the palette.
    pub id: &'static str,
    /// What the menus and the history call it.
    pub title: &'static str,
    /// The default shortcut ("Ctrl+S"; "" = none).
    pub keys: &'static str,
    /// The words it takes, by name (a trailing `…` takes the rest of the line).
    pub params: &'static [&'static str],
    /// One sentence on what it does.
    pub help: &'static str,
    /// Does it change the project (and so get a history entry of its own title)?
    pub changes: bool,
    run: fn(&mut Workspace, &[String]) -> Result<String, String>,
}

fn kind_named(name: &str) -> Result<RefKind, String> {
    let n = name.trim().to_lowercase();
    RefKind::ALL.iter().copied().find(|k| k.name() == n || k.name().replace(' ', "_") == n).ok_or_else(|| format!("\"{name}\" is not a kind (item, prop, character, map…)"))
}

fn arg<'a>(args: &'a [String], k: usize, what: &str) -> Result<&'a str, String> {
    args.get(k).map(String::as_str).filter(|a| !a.is_empty()).ok_or_else(|| format!("which {what}?"))
}

fn need_project(ws: &Workspace) -> Result<(), String> {
    if ws.project_open {
        Ok(())
    } else {
        Err("no project is open".into())
    }
}

/// Every command, in the order menus show their groups.
pub const COMMANDS: &[Command] = &[
    Command {
        id: "file.save_all",
        title: "Save all",
        keys: "Ctrl+S",
        params: &[],
        help: "Writes every changed map, the definitions and the project settings.",
        changes: false,
        run: |ws, _| {
            need_project(ws)?;
            let was = ws.is_dirty();
            ws.save_all();
            Ok(if ws.is_dirty() { ws.status.0.clone() } else if was { "Saved".into() } else { "Nothing to save".into() })
        },
    },
    Command {
        id: "file.close_project",
        title: "Close project",
        keys: "",
        params: &[],
        help: "Writes the project file and goes back to the start.",
        changes: false,
        run: |ws, _| {
            need_project(ws)?;
            if ws.is_dirty() {
                return Err("there are unsaved changes: save first (file.save_all)".into());
            }
            ws.close_project();
            Ok("Closed the project".into())
        },
    },
    Command {
        id: "edit.undo",
        title: "Undo",
        keys: "Ctrl+Z",
        params: &[],
        help: "Takes back the latest change, wherever it was made.",
        changes: false,
        run: |ws, _| ws.undo().ok_or_else(|| "nothing to undo".into()),
    },
    Command {
        id: "edit.redo",
        title: "Redo",
        keys: "Ctrl+Y",
        params: &[],
        help: "Makes the latest undone change again.",
        changes: false,
        run: |ws, _| ws.redo().ok_or_else(|| "nothing to redo".into()),
    },
    Command {
        id: "edit.rename",
        title: "Rename",
        keys: "",
        params: &["kind", "old id", "new id"],
        help: "Renames a definition or a map, with every reference in definitions, maps and scripts following.",
        changes: true,
        run: |ws, a| {
            need_project(ws)?;
            let kind = kind_named(arg(a, 0, "kind")?)?;
            ws.try_rename_id(kind, arg(a, 1, "id")?, arg(a, 2, "new id")?)
        },
    },
    Command {
        id: "defs.new",
        title: "New definition",
        keys: "Ctrl+N",
        params: &["kind", "id"],
        help: "Adds a definition of a kind with the values a new one starts with.",
        changes: true,
        run: |ws, a| {
            need_project(ws)?;
            let kind = kind_named(arg(a, 0, "kind")?)?;
            let id = arg(a, 1, "id")?;
            ws.new_def(kind, id)?;
            Ok(format!("New {} \"{id}\"", kind.name()))
        },
    },
    Command {
        id: "defs.duplicate",
        title: "Duplicate definition",
        keys: "",
        params: &["kind", "id", "new id"],
        help: "Adds a copy of a definition under a new id.",
        changes: true,
        run: |ws, a| {
            need_project(ws)?;
            let kind = kind_named(arg(a, 0, "kind")?)?;
            let (id, new) = (arg(a, 1, "id")?, arg(a, 2, "new id")?);
            ws.duplicate_def(kind, id, new)?;
            Ok(format!("Copied {} \"{id}\" as \"{new}\"", kind.name()))
        },
    },
    Command {
        id: "defs.delete",
        title: "Delete definition",
        keys: "",
        params: &["kind", "id", "force"],
        help: "Removes a definition; one that is still used only with the word force.",
        changes: true,
        run: |ws, a| {
            need_project(ws)?;
            let kind = kind_named(arg(a, 0, "kind")?)?;
            let id = arg(a, 1, "id")?;
            ws.delete_def(kind, id, a.get(2).is_some_and(|f| f == "force"))?;
            Ok(format!("Deleted {} \"{id}\"", kind.name()))
        },
    },
    Command {
        id: "map.open",
        title: "Open map",
        keys: "",
        params: &["map"],
        help: "Makes a map the one being worked on; the others stay open with their changes.",
        changes: false,
        run: |ws, a| {
            need_project(ws)?;
            let name = arg(a, 0, "map")?;
            if !ws.maps.iter().any(|m| m == name) {
                return Err(format!("\"{name}\" is not a map"));
            }
            ws.open_map(name);
            match &ws.doc {
                Some(d) if d.map.name == name => Ok(format!("Opened {name}")),
                _ => Err(ws.status.0.clone()),
            }
        },
    },
    Command {
        id: "map.close",
        title: "Close map",
        keys: "",
        params: &["map"],
        help: "Closes an open map (its unsaved changes are refused: save first).",
        changes: false,
        run: |ws, a| {
            let name = arg(a, 0, "map")?.to_string();
            if ws.open_maps().iter().any(|d| d.map.name == name && d.is_dirty()) {
                return Err(format!("{name} has unsaved changes: save first (file.save_all)"));
            }
            if ws.close_map(&name) {
                Ok(format!("Closed {name}"))
            } else {
                Err(format!("{name} is not open"))
            }
        },
    },
    Command {
        id: "find.search",
        title: "Go to anything",
        keys: "Ctrl+P",
        params: &["text…"],
        help: "Finds maps, definitions, script functions, characters, zones and dialog lines.",
        changes: false,
        run: |ws, a| {
            need_project(ws)?;
            let hits = ws.search(&a.join(" "));
            let mut out = format!("{} hit(s)", hits.len());
            for h in hits.iter().take(12) {
                out += &format!("\n{} {} ({})", h.what.name(), h.label, h.detail);
            }
            Ok(out)
        },
    },
    Command {
        id: "find.usages",
        title: "Find usages",
        keys: "",
        params: &["kind", "id"],
        help: "Lists every place an id is used: placed things, triggers, other definitions, scripts.",
        changes: false,
        run: |ws, a| {
            need_project(ws)?;
            let kind = kind_named(arg(a, 0, "kind")?)?;
            let id = arg(a, 1, "id")?;
            let list = ws.find_usages(kind, id);
            let mut out = format!("{} usage(s) of {} \"{id}\"", list.len(), kind.name());
            for u in list.iter().take(20) {
                out += &format!("\n{}: {}", u.target.describe(), u.label);
            }
            ws.usages.of = Some((kind, id.to_string()));
            ws.usages.list = list;
            Ok(out)
        },
    },
    Command {
        id: "check.problems",
        title: "Check the project",
        keys: "",
        params: &[],
        help: "Looks for everything broken: unknown ids, missing files, script errors, missing translations.",
        changes: false,
        run: |ws, _| {
            need_project(ws)?;
            ws.refresh_problems();
            let (e, w) = ws.problems.counts();
            let mut out = format!("{e} error(s), {w} warning(s)");
            for p in ws.problems.list.iter().take(20) {
                out += &format!("\n{}", p.line());
            }
            Ok(out)
        },
    },
    Command {
        id: "assets.reload",
        title: "Reload art and scripts",
        keys: "",
        params: &[],
        help: "Reads the project's pictures, models and scripts again from its files.",
        changes: false,
        run: |ws, _| {
            need_project(ws)?;
            ws.reload_assets();
            ws.refresh_refs();
            Ok("Reloaded".into())
        },
    },
    Command {
        id: "agent.start",
        title: "Start the agent server",
        keys: "",
        params: &["port"],
        help: "Lets AI agents read and build this project through the Model Context Protocol.",
        changes: false,
        run: |ws, a| {
            let port = a.first().and_then(|p| p.parse().ok()).unwrap_or(ws.mcp.port);
            ws.mcp.start(port).map(|addr| format!("The agent server listens on http://{addr}/mcp"))
        },
    },
    Command {
        id: "agent.stop",
        title: "Stop the agent server",
        keys: "",
        params: &[],
        help: "Closes the agent server's port.",
        changes: false,
        run: |ws, _| {
            ws.mcp.stop();
            Ok("The agent server is off".into())
        },
    },
];

/// The command with this id.
pub fn find(id: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.id == id)
}

impl Workspace {
    /// Runs a command by its id. Its answer (or what kept it from running) is also the status
    /// line; what it changed is one history entry under its title.
    pub fn run(&mut self, id: &str, args: &[String]) -> Result<String, String> {
        let Some(cmd) = find(id) else { return Err(format!("there is no command \"{id}\"")) };
        self.history_frame(None);
        if cmd.changes {
            self.begin(cmd.title);
        }
        let result = (cmd.run)(self, args);
        self.history_frame(None);
        match &result {
            Ok(msg) => self.status = (msg.lines().next().unwrap_or_default().to_string(), crate::now()),
            Err(e) => self.status = (format!("{}: {e}", cmd.title), crate::now()),
        }
        result
    }

    /// Runs a line as scripts and the palette write it: the command's id, then its words.
    pub fn run_line(&mut self, line: &str) -> Result<String, String> {
        let mut words = line.split_whitespace().map(str::to_string);
        let Some(id) = words.next() else { return Err("an empty line".into()) };
        let args: Vec<String> = words.collect();
        self.run(&id, &args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_well_formed() {
        let mut seen = std::collections::BTreeSet::new();
        for c in COMMANDS {
            assert!(seen.insert(c.id), "{} twice", c.id);
            assert!(c.id.split_once('.').is_some_and(|(g, n)| !g.is_empty() && !n.is_empty()) && c.id.chars().all(|ch| ch.is_ascii_lowercase() || ch == '.' || ch == '_'), "{}", c.id);
            assert!(!c.title.is_empty() && c.help.ends_with('.'), "{}", c.id);
        }
        let keys: Vec<&str> = COMMANDS.iter().map(|c| c.keys).filter(|k| !k.is_empty()).collect();
        assert_eq!(keys.len(), keys.iter().collect::<std::collections::BTreeSet<_>>().len(), "a shortcut is taken twice");
    }

    #[test]
    fn kinds_parse_by_name() {
        assert_eq!(kind_named("item").unwrap(), RefKind::Item);
        assert_eq!(kind_named("Map").unwrap(), RefKind::Map);
        assert!(kind_named("thing").is_err());
    }
}
