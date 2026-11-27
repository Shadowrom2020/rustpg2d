//! The Creator's Manual inside the editor: its chapters (the Markdown files of
//! `Documentation/manual`, built into the editor so they are there offline), cut into the
//! blocks a help panel draws, searched, and looked up by what the creator is doing (F1).

use std::sync::OnceLock;

/// A piece of a line of text.
#[derive(Clone, Debug, PartialEq)]
pub enum Span {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    /// A link: its text, and where it leads (`03-maps.md#collision`, `#anchor`, `https://…`).
    Link(String, String),
}

/// A block of a chapter.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// A heading: its level (1 = the chapter's title), its text, its anchor.
    Heading(u8, String, String),
    Para(Vec<Span>),
    /// A list: numbered or not; each item with how deep it is indented.
    List(bool, Vec<(u8, Vec<Span>)>),
    Code(String),
    /// A table: its rows of cells (the first row is the header).
    Table(Vec<Vec<Vec<Span>>>),
    /// A picture's caption (the pictures themselves aren't in the editor).
    Picture(String),
    Rule,
}

/// A heading of a chapter, to jump to.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub level: u8,
    pub title: String,
    pub anchor: String,
}

pub struct Chapter {
    /// Its file's name (`03-maps.md`): what links name.
    pub file: &'static str,
    pub title: String,
    pub text: &'static str,
    pub blocks: Vec<Block>,
    pub sections: Vec<Section>,
}

macro_rules! chapters {
    ($($file:literal),* $(,)?) => {
        [$(($file, include_str!(concat!("../../../Documentation/manual/", $file)))),*]
    };
}

const FILES: [(&str, &str); 21] = chapters![
    "README.md",
    "01-getting-started.md",
    "02-the-editor.md",
    "03-maps.md",
    "04-objects-and-zones.md",
    "05-database.md",
    "06-characters.md",
    "07-events.md",
    "08-scripting.md",
    "09-gameplay.md",
    "10-art-and-sound.md",
    "11-asset-library.md",
    "12-languages.md",
    "13-ai-agents.md",
    "14-publishing.md",
    "15-players-guide.md",
    "16-troubleshooting.md",
    "17-testing.md",
    "18-plugins.md",
    "19-srd-rules.md",
    "20-visual-scripts.md",
];

/// The manual's chapters, the contents page first.
pub fn chapters() -> &'static [Chapter] {
    static ALL: OnceLock<Vec<Chapter>> = OnceLock::new();
    ALL.get_or_init(|| {
        FILES
            .iter()
            .map(|(file, text)| {
                let blocks = parse(text);
                let sections: Vec<Section> = blocks.iter().filter_map(|b| if let Block::Heading(level, title, anchor) = b { Some(Section { level: *level, title: title.clone(), anchor: anchor.clone() }) } else { None }).collect();
                let title = sections.first().map_or_else(|| file.to_string(), |s| s.title.clone());
                Chapter { file, title, text, blocks, sections }
            })
            .collect()
    })
}

/// The chapter a link's file part names.
pub fn chapter_named(file: &str) -> Option<usize> {
    chapters().iter().position(|c| c.file == file)
}

/// A heading as an anchor, as the manual's links write them: lower case, spaces as `-`,
/// everything but letters, digits, `-` and `_` left out.
pub fn anchor_of(title: &str) -> String {
    title.trim().to_lowercase().chars().filter_map(|c| if c == ' ' { Some('-') } else if c.is_alphanumeric() || c == '-' || c == '_' { Some(c) } else { None }).collect()
}

/// A line's text as spans: `**bold**`, `*italic*`, `` `code` ``, `[text](target)`.
pub fn spans(line: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut text = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let find = |from: usize, pat: &[char]| (from..chars.len().saturating_sub(pat.len() - 1)).find(|k| chars[*k..*k + pat.len()] == *pat);
    let flush = |text: &mut String, out: &mut Vec<Span>| {
        if !text.is_empty() {
            out.push(Span::Text(std::mem::take(text)));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        // (A backslash keeps the next character as it is: `\*`, `\|`.)
        if c == '\\' && i + 1 < chars.len() {
            text.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = find(i + 1, &['`']) {
                flush(&mut text, &mut out);
                out.push(Span::Code(chars[i + 1..end].iter().collect()));
                i = end + 1;
                continue;
            }
        }
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = find(i + 2, &['*', '*']) {
                flush(&mut text, &mut out);
                out.push(Span::Bold(plain(&chars[i + 2..end].iter().collect::<String>())));
                i = end + 2;
                continue;
            }
        }
        if c == '*' && chars.get(i + 1).is_some_and(|n| !n.is_whitespace()) {
            if let Some(end) = find(i + 1, &['*']).filter(|e| !chars[*e - 1].is_whitespace()) {
                flush(&mut text, &mut out);
                out.push(Span::Italic(plain(&chars[i + 1..end].iter().collect::<String>())));
                i = end + 1;
                continue;
            }
        }
        if c == '[' {
            if let Some(close) = find(i + 1, &[']', '(']) {
                if let Some(end) = find(close + 2, &[')']) {
                    flush(&mut text, &mut out);
                    out.push(Span::Link(plain(&chars[i + 1..close].iter().collect::<String>()), chars[close + 2..end].iter().collect()));
                    i = end + 1;
                    continue;
                }
            }
        }
        text.push(c);
        i += 1;
    }
    flush(&mut text, &mut out);
    out
}

/// A line without its marks (for headings, search and what is inside bold or a link).
pub fn plain(line: &str) -> String {
    spans(line)
        .into_iter()
        .map(|s| match s {
            Span::Text(t) | Span::Bold(t) | Span::Italic(t) | Span::Code(t) | Span::Link(t, _) => t,
        })
        .collect()
}

/// The cells of a table's line (`| a | b |`).
fn cells(line: &str) -> Vec<String> {
    let line = line.trim().trim_start_matches('|').trim_end_matches('|');
    // (A `\|` inside a cell is no border.)
    let mut out = vec![String::new()];
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                out.last_mut().unwrap().push_str("\\|");
                chars.next();
            }
            '|' => out.push(String::new()),
            c => out.last_mut().unwrap().push(c),
        }
    }
    out.into_iter().map(|c| c.trim().to_string()).collect()
}

/// A list item's line: how deep, whether numbered, its text.
fn list_item(line: &str) -> Option<(u8, bool, &str)> {
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        return Some(((indent / 2) as u8, false, rest));
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 3 {
        if let Some(rest) = t[digits..].strip_prefix(". ") {
            return Some(((indent / 2) as u8, true, rest));
        }
    }
    None
}

/// A chapter's Markdown as blocks (the Markdown the manual is written in: headings,
/// paragraphs, lists, tables, fenced code, pictures, rules).
pub fn parse(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            i += 1;
            continue;
        }
        if let Some(lang) = t.strip_prefix("```") {
            let _ = lang;
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            i += 1;
            out.push(Block::Code(code.join("\n")));
            continue;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            let title = plain(t[hashes..].trim());
            out.push(Block::Heading(hashes as u8, title.clone(), anchor_of(&title)));
            i += 1;
            continue;
        }
        if t == "---" || t == "***" {
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        if let Some(alt) = t.strip_prefix("![").and_then(|r| r.split_once("](")).map(|(alt, _)| alt) {
            out.push(Block::Picture(alt.to_string()));
            i += 1;
            continue;
        }
        if t.starts_with('|') {
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let row = cells(lines[i]);
                // (The line of dashes under the header.)
                if !row.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))) {
                    rows.push(row.iter().map(|c| spans(c)).collect());
                }
                i += 1;
            }
            out.push(Block::Table(rows));
            continue;
        }
        if let Some((_, ordered, _)) = list_item(line) {
            let mut items: Vec<(u8, String)> = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                if let Some((depth, _, rest)) = list_item(l) {
                    items.push((depth, rest.to_string()));
                } else if !l.trim().is_empty() && l.starts_with(' ') && !items.is_empty() {
                    // (An item's text goes on in the next line, indented.)
                    let last = items.last_mut().unwrap();
                    last.1.push(' ');
                    last.1.push_str(l.trim());
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Block::List(ordered, items.into_iter().map(|(d, text)| (d, spans(&text))).collect()));
            continue;
        }
        // A paragraph: lines up to an empty one or another kind of block.
        let mut para = vec![t];
        i += 1;
        while i < lines.len() {
            let n = lines[i].trim();
            let other = n.is_empty() || n.starts_with('#') || n.starts_with("```") || n.starts_with('|') || n.starts_with("![") || list_item(lines[i]).is_some();
            if other {
                break;
            }
            para.push(n);
            i += 1;
        }
        out.push(Block::Para(spans(&para.join(" "))));
    }
    out
}

/// A place in the manual that fits what was searched for.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub chapter: usize,
    /// The section it is in (the anchor of the nearest heading above).
    pub anchor: String,
    /// That section's title.
    pub section: String,
    /// The line that fits, without its marks.
    pub line: String,
}

/// Looks for every word of `query` in the manual: sections whose title has them all first,
/// then lines that do. At most `most`.
pub fn search(query: &str, most: usize) -> Vec<Hit> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let fits = |text: &str| {
        let t = text.to_lowercase();
        words.iter().all(|w| t.contains(w.as_str()))
    };
    let (mut titles, mut lines) = (Vec::new(), Vec::new());
    for (k, c) in chapters().iter().enumerate() {
        let mut section = (c.sections.first().map_or(String::new(), |s| s.anchor.clone()), c.title.clone());
        for line in c.text.lines() {
            let t = line.trim();
            let hashes = t.chars().take_while(|ch| *ch == '#').count();
            if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
                let title = plain(t[hashes..].trim());
                section = (anchor_of(&title), title.clone());
                if fits(&title) {
                    titles.push(Hit { chapter: k, anchor: section.0.clone(), section: title, line: c.title.clone() });
                }
                continue;
            }
            if t.is_empty() || t.starts_with("```") || t.starts_with("![") {
                continue;
            }
            let text = plain(t);
            if fits(&text) && lines.len() < most {
                lines.push(Hit { chapter: k, anchor: section.0.clone(), section: section.1.clone(), line: text });
            }
        }
    }
    titles.extend(lines);
    titles.truncate(most);
    titles
}

/// The section of chapter `file` whose title has one of `words` (the first that does; the
/// chapter's top when none does, or no words are given). For F1: `("03-maps.md", anchor)`.
pub fn topic(file: &str, words: &[&str]) -> (usize, String) {
    let Some(k) = chapter_named(file) else { return (0, String::new()) };
    let c = &chapters()[k];
    for w in words {
        let w = w.to_lowercase();
        // (A heading that is about it rather than one that mentions it: the shortest.)
        if let Some(s) = c.sections.iter().skip(1).filter(|s| s.title.to_lowercase().contains(&w)).min_by_key(|s| s.title.len()) {
            return (k, s.anchor.clone());
        }
    }
    (k, c.sections.first().map_or(String::new(), |s| s.anchor.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_cut_into_blocks_and_spans() {
        let text = "# 3. Maps\n\nA map is **painted** with *materials*, see [Collision](#collision) and\n`dump X Y`.\n\n- one\n- two with\n  more\n  - deeper\n\n1. first\n2. second\n\n| Key | Does |\n|---|---|\n| **G** | grid \\| lines |\n\n```rhai\nlet x = 1;\n```\n\n![The editor](../images/a.webp)\n\n## Painted ground\n";
        let blocks = parse(text);
        assert_eq!(blocks[0], Block::Heading(1, "3. Maps".into(), "3-maps".into()));
        assert_eq!(blocks[1], Block::Para(vec![Span::Text("A map is ".into()), Span::Bold("painted".into()), Span::Text(" with ".into()), Span::Italic("materials".into()), Span::Text(", see ".into()), Span::Link("Collision".into(), "#collision".into()), Span::Text(" and ".into()), Span::Code("dump X Y".into()), Span::Text(".".into())]));
        assert_eq!(blocks[2], Block::List(false, vec![(0, vec![Span::Text("one".into())]), (0, vec![Span::Text("two with more".into())]), (1, vec![Span::Text("deeper".into())])]));
        assert!(matches!(&blocks[3], Block::List(true, items) if items.len() == 2));
        assert_eq!(blocks[4], Block::Table(vec![vec![vec![Span::Text("Key".into())], vec![Span::Text("Does".into())]], vec![vec![Span::Bold("G".into())], vec![Span::Text("grid | lines".into())]]]));
        assert_eq!(blocks[5], Block::Code("let x = 1;".into()));
        assert_eq!(blocks[6], Block::Picture("The editor".into()));
        assert_eq!(blocks[7], Block::Heading(2, "Painted ground".into(), "painted-ground".into()));
        // Marks that aren't marks stay text: a lone star, a bracket without a link.
        assert_eq!(plain("2 * 3 = 6 and [not a link] and `code`"), "2 * 3 = 6 and [not a link] and code");
        assert_eq!(anchor_of("Low and high obstacles"), "low-and-high-obstacles");
        assert_eq!(anchor_of("The *Ground* tool (G)"), "the-ground-tool-g");
    }

    #[test]
    fn every_chapter_is_there_with_its_sections_and_links_that_lead_somewhere() {
        let all = chapters();
        assert_eq!(all.len(), 21);
        assert_eq!((all[0].file, all[3].file, all[3].title.as_str()), ("README.md", "03-maps.md", "3. Maps"));
        assert!(all.iter().all(|c| !c.sections.is_empty() && c.blocks.len() > 5), "every chapter has a title and text");
        // Every link to a chapter names one that is there, and its anchor a heading of it.
        let mut broken = Vec::new();
        fn links<'a>(blocks: &'a [Block], out: &mut Vec<&'a str>) {
            let mut take = |spans: &'a [Span]| {
                for s in spans {
                    if let Span::Link(_, target) = s {
                        out.push(target.as_str());
                    }
                }
            };
            for b in blocks {
                match b {
                    Block::Para(s) => take(s),
                    Block::List(_, items) => items.iter().for_each(|(_, s)| take(s)),
                    Block::Table(rows) => rows.iter().flatten().for_each(|s| take(s)),
                    _ => {}
                }
            }
        }
        for c in all {
            let mut found = Vec::new();
            links(&c.blocks, &mut found);
            for target in found {
                if target.contains("://") || target.starts_with("../") || target.starts_with("mailto:") {
                    continue;
                }
                let (file, anchor) = target.split_once('#').unwrap_or((target, ""));
                let k = if file.is_empty() { chapter_named(c.file) } else { chapter_named(file) };
                let ok = k.is_some_and(|k| anchor.is_empty() || all[k].sections.iter().any(|s| s.anchor == anchor));
                if !ok {
                    broken.push(format!("{}: {target}", c.file));
                }
            }
        }
        assert!(broken.is_empty(), "links that lead nowhere: {broken:#?}");
    }

    #[test]
    fn the_manual_is_searched_and_a_topic_found() {
        // A section's title first, then lines.
        let hits = search("painted ground", 20);
        assert!(!hits.is_empty());
        assert_eq!((chapters()[hits[0].chapter].file, hits[0].section.as_str()), ("03-maps.md", "Painted ground"));
        assert!(search("lockpick", 50).iter().any(|h| chapters()[h.chapter].file == "09-gameplay.md"));
        assert!(search("zzzznothing", 5).is_empty() && search("   ", 5).is_empty());
        assert!(search("the", 7).len() <= 7);
        // F1: the section about what is at hand, else the chapter's top.
        let (k, anchor) = topic("03-maps.md", &["collision"]);
        assert_eq!((chapters()[k].file, anchor.as_str()), ("03-maps.md", "collision"));
        let (k, anchor) = topic("03-maps.md", &["no such thing"]);
        assert_eq!((chapters()[k].file, anchor.as_str()), ("03-maps.md", "3-maps"));
        assert_eq!(topic("nowhere.md", &[]).0, 0);
    }
}
