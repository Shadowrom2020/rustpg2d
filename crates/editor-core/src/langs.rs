//! Translating a game: the languages it is offered in with how far each is done, and a
//! language's table: every text players read, its translation beside it, written as
//! `lang/<code>.ron` (the engine's `lang.rs` reads it; an empty translation shows the original).

use std::path::PathBuf;

use rustpg2d_engine::lang::{self, LangFile};

use crate::workspace::Workspace;

/// A language the game is translated into.
#[derive(Clone, Debug, PartialEq)]
pub struct Language {
    pub code: String,
    /// Its own name ("Deutsch").
    pub name: String,
    /// Texts with a translation, and texts the game has.
    pub done: usize,
    pub total: usize,
    /// The menus, the HUD and the windows of the game itself are translated already (the
    /// engine has that language built in).
    pub builtin_ui: bool,
}

/// A language's translations, being worked on.
#[derive(Clone, Debug, PartialEq)]
pub struct LangTable {
    pub code: String,
    pub file: LangFile,
    /// The texts the game has, in the order they are listed.
    pub texts: Vec<String>,
    /// Translations of texts the game no longer has (kept until they are taken out).
    pub unused: Vec<String>,
    /// Something was typed that isn't in the file yet.
    pub dirty: bool,
}

impl LangTable {
    pub fn translation(&self, text: &str) -> &str {
        self.file.texts.get(text).map_or("", String::as_str)
    }

    pub fn set(&mut self, text: &str, translation: &str) {
        if self.translation(text) != translation {
            self.file.texts.insert(text.to_string(), translation.to_string());
            self.dirty = true;
        }
    }

    /// Texts that have no translation yet.
    pub fn missing(&self) -> usize {
        self.texts.iter().filter(|t| self.translation(t).is_empty()).count()
    }

    /// The texts that fit what is searched (in the original or the translation), those without
    /// a translation alone when `missing_only`.
    pub fn shown(&self, search: &str, missing_only: bool) -> Vec<&str> {
        let f = search.trim().to_lowercase();
        self.texts
            .iter()
            .map(String::as_str)
            .filter(|t| !missing_only || self.translation(t).is_empty())
            .filter(|t| f.is_empty() || t.to_lowercase().contains(&f) || self.translation(t).to_lowercase().contains(&f))
            .collect()
    }

    /// Takes out the translations of texts the game no longer has. Returns how many.
    pub fn drop_unused(&mut self) -> usize {
        let n = self.unused.len();
        for k in std::mem::take(&mut self.unused) {
            self.file.texts.remove(&k);
        }
        self.dirty |= n > 0;
        n
    }

    /// `de:missing=3/120:changed` for scripts.
    pub fn status(&self) -> String {
        format!("{}:missing={}/{}:{}", self.code, self.missing(), self.texts.len(), if self.dirty { "changed" } else { "saved" })
    }
}

/// Whether `code` can be a language's code (`de`, `pt-br`).
pub fn valid_code(code: &str) -> bool {
    code.len() >= 2 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Workspace {
    fn lang_path(&self, code: &str) -> PathBuf {
        self.project.root.join("lang").join(format!("{code}.ron"))
    }

    /// The languages the game is translated into, each with how far it is. (The texts are
    /// those of the project as it is saved: maps are read from their files.)
    pub fn languages(&self) -> Vec<Language> {
        let texts = lang::content_texts(&self.project);
        self.project
            .config
            .languages
            .iter()
            .filter(|c| **c != self.project.config.language)
            .map(|code| {
                let file = lang::load_file(&self.project.root, code).unwrap_or_default();
                let done = texts.iter().filter(|t| file.texts.get(*t).is_some_and(|v| !v.is_empty())).count();
                let name = if file.name.is_empty() { lang::language_name(code) } else { file.name.clone() };
                Language { code: code.clone(), name, done, total: texts.len(), builtin_ui: !lang::builtin_ui(code).is_empty() }
            })
            .collect()
    }

    /// The table of `code`: what its file has, with every text of the game (new ones empty).
    pub fn lang_table(&self, code: &str) -> Result<LangTable, String> {
        if !valid_code(code) {
            return Err(format!("\"{code}\" is not a language code (like de, fr or pt-br)"));
        }
        if code == self.project.config.language {
            return Err(format!("{code} is the language the game is written in"));
        }
        let texts: Vec<String> = lang::content_texts(&self.project).into_iter().collect();
        let (file, unused) = lang::template(&self.project, code, lang::load_file(&self.project.root, code));
        Ok(LangTable { code: code.to_string(), file, texts, unused, dirty: false })
    }

    /// Adds a language to the game (when it isn't one yet) and writes its file with every
    /// text, not translated yet. Everything is saved first, so the maps' texts are current.
    pub fn add_language(&mut self, code: &str) -> Result<String, String> {
        self.export_translation(code)
    }

    /// Takes a language off the game's list. Its file stays (what was translated isn't lost).
    pub fn remove_language(&mut self, code: &str) -> Result<String, String> {
        if !self.project.config.languages.iter().any(|c| c == code) {
            return Err(format!("the game isn't offered in \"{code}\""));
        }
        self.begin(&format!("Take the language {code} off the game"));
        self.project.config.languages.retain(|c| c != code);
        self.defs_dirty = true;
        Ok(format!("{code} is no longer offered; lang/{code}.ron stays, with what was translated"))
    }

    /// Writes a table as its language's file.
    pub fn save_lang_table(&mut self, table: &mut LangTable) -> Result<String, String> {
        let path = self.lang_path(&table.code);
        rustpg2d_engine::project::write_ron(&path, &table.file).map_err(|e| format!("{e:#}"))?;
        self.watch_note(&path);
        table.dirty = false;
        Ok(format!("lang/{}.ron: {} texts, {} not translated yet", table.code, table.texts.len(), table.missing()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::refactor::RefKind;

    #[test]
    fn a_language_is_added_translated_and_counted() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-langs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.history_frame(None);
        ws.make_def(RefKind::Item, "Golden key", None).unwrap();
        ws.make_def(RefKind::Quest, "The lost ring", None).unwrap();
        ws.history_frame(None);
        assert!(ws.languages().is_empty());
        // What is no code, and the game's own language, are refused.
        assert!(ws.add_language("d").is_err() && ws.add_language(&ws.project.config.language.clone()).is_err());
        assert!(ws.lang_table("d e").is_err());
        // Added: on the game's list, its file written with every text, none translated.
        let said = ws.add_language("de").unwrap();
        assert!(said.starts_with("lang/de.ron: "), "{said}");
        let langs = ws.languages();
        assert_eq!((langs.len(), langs[0].code.as_str(), langs[0].name.as_str(), langs[0].done, langs[0].builtin_ui), (1, "de", "Deutsch", 0, true));
        assert!(langs[0].total >= 2);
        let mut table = ws.lang_table("de").unwrap();
        assert!(table.texts.iter().any(|t| t == "Golden key") && table.texts.iter().any(|t| t == "The lost ring"));
        assert_eq!((table.missing(), table.dirty), (table.texts.len(), false));
        // Translated and written: counted, and read by the game's own reader.
        table.set("Golden key", "Goldener Schlüssel");
        assert!(table.dirty && table.status().ends_with(":changed"));
        assert_eq!(table.shown("schlüssel", false), ["Golden key"]);
        assert!(!table.shown("", true).contains(&"Golden key"));
        ws.save_lang_table(&mut table).unwrap();
        assert!(!table.dirty);
        assert_eq!(ws.languages()[0].done, 1);
        let read = lang::load_file(&dir, "de").unwrap();
        assert_eq!(read.texts.get("Golden key").map(String::as_str), Some("Goldener Schlüssel"));
        // A text the game no longer has: its translation is kept until it is taken out.
        ws.run_line("defs.delete item golden_key").unwrap();
        ws.save_all();
        let mut table = ws.lang_table("de").unwrap();
        assert_eq!(table.unused, ["Golden key"]);
        assert_eq!(table.drop_unused(), 1);
        assert!(table.dirty && !table.file.texts.contains_key("Golden key"));
        // Taken off the list, the file stays.
        ws.remove_language("de").unwrap();
        assert!(ws.languages().is_empty() && dir.join("lang/de.ron").is_file());
        assert!(ws.remove_language("de").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
