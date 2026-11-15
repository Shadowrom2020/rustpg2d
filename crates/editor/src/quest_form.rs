//! The quest designer (Database › Quests): a quest described in one place — who gives it and
//! when, what the giver says when offering it, each step (its journal line, how it is done,
//! where it leads, what the giver says meanwhile, actions when it starts and when it is done)
//! and the rewards. `runtime/quests.rs` plays it; triggers and scripts can still move quests on.

use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::defs::Database;
use rustpg2d_engine::events::Cond;
use rustpg2d_engine::quest::{QuestCheck, QuestDef, QuestStep, RewardItem, StepGoal};

use crate::ui::{self, ph};
use crate::widgets::{self, Refs};

/// The quest form. `db`: the definitions as they were this frame (names for the summary).
pub fn quest_form(ui: &mut egui::Ui, d: &mut QuestDef, refs: &Refs, db: &Database) {
    ui.heading("Quest");
    ui.label(egui::RichText::new(summary(d, db)).color(Color32::from_rgb(230, 200, 120)));
    ui.add_space(4.0);
    egui::Grid::new("quest").num_columns(2).show(ui, |ui| {
        ui.label("Id");
        ui.add(egui::TextEdit::singleline(&mut d.id).desired_width(200.0))
            .on_hover_text("Used by 'Quest' actions / conditions and scripts (start_quest(\"id\")). Changing it does not update them.");
        ui.end_row();
        ui.label("Title");
        ui.add(egui::TextEdit::singleline(&mut d.title).desired_width(320.0));
        ui.end_row();
    });
    ui.label("Description (shown in the journal)");
    crate::database::text_box(ui, &mut d.description, 3, "");

    section(ui, "Giver and offer", true, |ui| giver_ui(ui, d, refs));
    section(ui, &format!("Steps ({})", d.stages.len()), true, |ui| steps_ui(ui, d, refs));
    section(ui, "Rewards", true, |ui| rewards_ui(ui, d, refs));
}

/// A collapsible section of the form.
fn section(ui: &mut egui::Ui, title: &str, open: bool, body: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(6.0);
    egui::CollapsingHeader::new(egui::RichText::new(title).strong().size(16.0)).id_salt(("quest_section", title.split(' ').next().unwrap_or(title))).default_open(open).show(ui, body);
}

/// One line about the whole quest: "Given by Elder Rowan · after The Strange Lights · 3 steps ·
/// 25 Coin, 80 XP".
pub fn summary(d: &QuestDef, db: &Database) -> String {
    let mut parts = Vec::new();
    if d.has_giver() {
        let name = if d.giver_name.trim().is_empty() { db.character(&d.giver).map_or(d.giver.clone(), |c| c.name.clone()) } else { d.giver_name.clone() };
        parts.push(format!("Given by {name}"));
        for c in &d.requires {
            if let Cond::Quest { quest, check: QuestCheck::Completed, .. } = &c.cond {
                if !c.negate {
                    parts.push(format!("after {}", db.quest(quest).map_or(quest.clone(), |q| q.title.clone())));
                }
            }
        }
        let others = d.requires.iter().filter(|c| !matches!(c.cond, Cond::Quest { check: QuestCheck::Completed, .. }) || c.negate).count();
        if others > 0 {
            parts.push(format!("{others} more condition(s)"));
        }
    } else {
        parts.push("Started by triggers or scripts".into());
    }
    parts.push(match d.stages.len() {
        1 => "1 step".into(),
        n => format!("{n} steps"),
    });
    let mut reward: Vec<String> = d.rewards.items.iter().map(|i| format!("{} {}", i.count, db.item(&i.item).map_or(i.item.clone(), |it| it.name.clone()))).collect();
    if d.xp > 0 {
        reward.push(format!("{} XP", d.xp));
    }
    if d.rewards.skill_points > 0 {
        reward.push(format!("{} skill points", d.rewards.skill_points));
    }
    if !d.rewards.actions.is_empty() {
        reward.push(format!("{} more action(s)", d.rewards.actions.len()));
    }
    if !reward.is_empty() {
        parts.push(reward.join(", "));
    }
    parts.join("  ·  ")
}

/// Who gives the quest, when, and what they say offering it.
fn giver_ui(ui: &mut egui::Ui, d: &mut QuestDef, refs: &Refs) {
    ui.horizontal(|ui| {
        ui.label("Giver").on_hover_text("Who offers the quest when talked to, and whom Talk steps without a character lead back to: a placed character's name (just that one, e.g. ida) or a character definition (every character placed from it).");
        widgets::suggest(ui, "quest_giver", &mut d.giver, &refs.people, 180.0);
        if d.giver.trim().is_empty() {
            ui.weak("none: Quest actions and start_quest() start it");
        } else {
            ui.label("shown as").on_hover_text("The giver's name in the quest's dialogs (empty: its character's name, which may be a generic one like Merchant)");
            ui.add(egui::TextEdit::singleline(&mut d.giver_name).desired_width(140.0).hint_text("its character's name"));
        }
    });
    if !d.has_giver() {
        ui.weak("With a giver, talking to them offers the quest (with a ! over their head while they have it to offer), steps can wait for a talk with them, and they can comment on the current step.");
        return;
    }
    ui.add_space(4.0);
    ui.label("Offered when").on_hover_text("Besides not being started yet: e.g. the Quest condition 'completed' on another quest makes this one its follow-up.");
    ui.weak("All must hold. A follow-up: add Quest › completed with the first quest. Empty = offered from the start.");
    widgets::conditions_ui(ui, "quest_requires", &mut d.requires, refs);
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Offering it").strong());
    ui.weak("The giver's dialog boxes, then the question; accepting starts the quest. Without a question it starts after the lines. {player}, {sir|madam} and {var} work as in dialogs.");
    lines_ui(ui, "offer_lines", &mut d.offer.lines, "What the giver says first", "+ Line");
    egui::Grid::new("offer_q").num_columns(2).show(ui, |ui| {
        ui.label("Question");
        ui.add(egui::TextEdit::singleline(&mut d.offer.question).desired_width(360.0).hint_text("Will you help? (empty = no question)"));
        ui.end_row();
        let asks = !d.offer.question.trim().is_empty();
        ui.label("Answers");
        ui.horizontal(|ui| {
            ui.add_enabled(asks, egui::TextEdit::singleline(&mut d.offer.accept).desired_width(170.0).hint_text("Yes"));
            ui.add_enabled(asks, egui::TextEdit::singleline(&mut d.offer.decline).desired_width(170.0).hint_text("No"));
        });
        ui.end_row();
    });
    ui.label("After accepting");
    lines_ui(ui, "offer_accepted", &mut d.offer.accepted, "What the giver says then", "+ Line");
    if !d.offer.question.trim().is_empty() {
        ui.label("After declining (asked again next time)");
        lines_ui(ui, "offer_declined", &mut d.offer.declined, "What the giver says then", "+ Line");
    }
}

/// The steps: the journal line, how it is done, where it leads, what the giver says meanwhile,
/// its actions.
fn steps_ui(ui: &mut egui::Ui, d: &mut QuestDef, refs: &Refs) {
    ui.weak("The journal shows the steps reached so far. Each step is done when its condition is met (or by Quest actions and scripts), then the next one starts; after the last one the quest is completed.");
    let n = d.stages.len();
    d.targets.resize(n, Default::default());
    d.steps.resize(n, QuestStep::default());
    let giver = d.has_giver();
    let (mut swap, mut remove) = (None, None);
    for k in 0..n {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Step {}", k + 1)).strong());
                if ui.add_enabled(k > 0, egui::Button::new(ph::ARROW_UP).small()).on_hover_text("Move up").clicked() {
                    swap = Some(k - 1);
                }
                if ui.add_enabled(k + 1 < n, egui::Button::new(ph::ARROW_DOWN).small()).on_hover_text("Move down").clicked() {
                    swap = Some(k);
                }
                if ui.small_button(ph::TRASH).on_hover_text("Remove (later steps move up: check triggers and scripts that set stages)").clicked() {
                    remove = Some(k);
                }
            });
            ui.add(egui::TextEdit::multiline(&mut d.stages[k]).desired_rows(1).desired_width(480.0).hint_text("The objective in the journal, e.g. Defeat slimes ({pest_slimes}/5)"));
            let s = &mut d.steps[k];
            ui.horizontal(|ui| {
                ui.label("Done when");
                egui::ComboBox::from_id_salt(("step_goal", k)).selected_text(s.done.label()).width(260.0).show_ui(ui, |ui| {
                    let talk = StepGoal::Talk { character: String::new(), lines: Vec::new(), when: Vec::new(), speaker: String::new() };
                    for g in [talk, StepGoal::Reach, StepGoal::Conditions(Vec::new()), StepGoal::Manual] {
                        let on = std::mem::discriminant(&s.done) == std::mem::discriminant(&g);
                        if ui.selectable_label(on, g.label()).clicked() && !on {
                            s.done = g;
                        }
                    }
                });
            });
            match &mut s.done {
                StepGoal::Talk { character, lines, when, speaker } => {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        widgets::suggest(ui, &format!("step_talk{k}"), character, &refs.people, 160.0);
                        ui.weak(if character.trim().is_empty() { "(empty = the giver)" } else { "a placed name, or a definition" });
                        ui.label("shown as");
                        ui.add(egui::TextEdit::singleline(speaker).desired_width(120.0).hint_text("its name"));
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            lines_ui(ui, &format!("step_lines{k}"), lines, "What they say", "+ Line");
                            ui.weak("Only when (e.g. Has item, for a hand-in; take it with a Take item action when the step is done):");
                            widgets::conditions_ui(ui, &format!("step_when{k}"), when, refs);
                        });
                    });
                }
                StepGoal::Conditions(c) => {
                    ui.weak("Checked all the time: has an item, defeated so many, a variable or flag, another quest…");
                    widgets::conditions_ui(ui, &format!("step_cond{k}"), c, refs);
                }
                StepGoal::Reach => {
                    ui.weak("On entering the zone, or interacting with the character or prop named below.");
                }
                StepGoal::Manual => {
                    ui.weak("A Quest action (advance, set stage, complete) or a script (advance_quest…) moves it on.");
                }
            }
            let t = &mut d.targets[k];
            ui.horizontal(|ui| {
                ui.label("Leads to").on_hover_text("Its quest marker: over this character, prop or zone (by its name on the map) while the step is current; on other maps at the exit leading there. A Talk step's character gets the marker anyway when it is on the map.");
                widgets::suggest(ui, &format!("qt_map{k}"), &mut t.map, &refs.maps, 110.0);
                ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(140.0).hint_text("name on the map"));
            });
            let s = &mut d.steps[k];
            if giver {
                ui.horizontal(|ui| {
                    ui.label("The giver says meanwhile");
                    ui.add(egui::TextEdit::singleline(&mut s.giver_line).desired_width(340.0).hint_text("(nothing: their own triggers run)"));
                });
            }
            actions_section(ui, &format!("When step {} starts", k + 1), &format!("step_start{k}"), &mut s.on_start, refs);
            actions_section(ui, &format!("When step {} is done", k + 1), &format!("step_done{k}"), &mut s.on_done, refs);
        });
    }
    if let Some(k) = swap {
        d.stages.swap(k, k + 1);
        d.targets.swap(k, k + 1);
        d.steps.swap(k, k + 1);
    }
    if let Some(k) = remove {
        d.stages.remove(k);
        d.targets.remove(k);
        d.steps.remove(k);
    }
    if ui.button(ui::t(ph::PLUS, "Step")).clicked() {
        d.stages.push(String::new());
        // A new last step of a given quest: back to the giver.
        let back = d.has_giver() && d.stages.len() > 1;
        d.steps.push(QuestStep { done: if back { StepGoal::Talk { character: String::new(), lines: Vec::new(), when: Vec::new(), speaker: String::new() } } else { StepGoal::Manual }, ..Default::default() });
    }
    // (Trailing empty markers and steps aren't saved.)
    while d.targets.last().is_some_and(|t| t.map.is_empty() && t.name.is_empty()) {
        d.targets.pop();
    }
    while d.steps.last().is_some_and(QuestStep::is_empty) {
        d.steps.pop();
    }
}

/// What completing the quest gives.
fn rewards_ui(ui: &mut egui::Ui, d: &mut QuestDef, refs: &Refs) {
    ui.weak("Given when the quest is completed, whatever completes it. The journal shows them.");
    let mut remove = None;
    for (k, it) in d.rewards.items.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            widgets::suggest(ui, &format!("reward_item{k}"), &mut it.item, &refs.items, 180.0);
            ui.add(egui::DragValue::new(&mut it.count).range(1..=9999).prefix("x "));
            if ui.small_button(ph::TRASH).on_hover_text("Remove").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        d.rewards.items.remove(k);
    }
    if ui.button(ui::t(ph::PLUS, "Item")).clicked() {
        d.rewards.items.push(RewardItem { item: String::new(), count: 1 });
    }
    egui::Grid::new("quest_rewards").num_columns(2).show(ui, |ui| {
        ui.label("Experience");
        ui.add(egui::DragValue::new(&mut d.xp).range(0..=1_000_000).suffix(" XP")).on_hover_text("Project › Experience and levels must be on.");
        ui.end_row();
        ui.label("Skill points");
        ui.add(egui::DragValue::new(&mut d.rewards.skill_points).range(0..=100));
        ui.end_row();
    });
    actions_section(ui, "Also", "reward_actions", &mut d.rewards.actions, refs);
    ui.weak("E.g. Run script quest_thanks: friendship with the giver (they run with the giver as 'self' when the giver is here), Change standing, Unlock achievement.");
}

/// A collapsible list of actions (closed while empty).
fn actions_section(ui: &mut egui::Ui, title: &str, salt: &str, actions: &mut Vec<rustpg2d_engine::events::Action>, refs: &Refs) {
    let head = if actions.is_empty() { title.to_string() } else { format!("{title} ({})", actions.len()) };
    egui::CollapsingHeader::new(head).id_salt(("quest_actions", salt)).default_open(!actions.is_empty()).show(ui, |ui| {
        widgets::actions_ui(ui, salt, actions, refs);
    });
}

/// Lines said one after the other (one dialog box each): edit, reorder, remove, add.
fn lines_ui(ui: &mut egui::Ui, salt: &str, lines: &mut Vec<String>, hint: &str, add: &str) {
    let n = lines.len();
    let (mut swap, mut remove) = (None, None);
    for (k, l) in lines.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::multiline(l).desired_rows(1).desired_width(420.0).hint_text(hint).id_salt((salt, k)));
            if ui.add_enabled(k > 0, egui::Button::new(ph::ARROW_UP).small()).on_hover_text("Earlier").clicked() {
                swap = Some(k - 1);
            }
            if ui.add_enabled(k + 1 < n, egui::Button::new(ph::ARROW_DOWN).small()).on_hover_text("Later").clicked() {
                swap = Some(k);
            }
            if ui.small_button(ph::TRASH).on_hover_text("Remove").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = swap {
        lines.swap(k, k + 1);
    }
    if let Some(k) = remove {
        lines.remove(k);
    }
    if ui.small_button(add).clicked() {
        lines.push(String::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::events::Condition;

    #[test]
    fn the_summary_names_giver_follow_up_steps_and_rewards() {
        let mut db = Database::default();
        db.quests.push(QuestDef { id: "lights".into(), title: "The Lights".into(), ..Default::default() });
        let d = QuestDef {
            id: "ashes".into(),
            stages: vec!["a".into(), "b".into()],
            giver: "elder".into(),
            requires: vec![Condition { cond: Cond::Quest { quest: "lights".into(), check: QuestCheck::Completed, stage: 0 }, negate: false }],
            xp: 80,
            rewards: rustpg2d_engine::quest::QuestRewards { items: vec![RewardItem { item: "coin".into(), count: 25 }], ..Default::default() },
            ..Default::default()
        };
        assert_eq!(summary(&d, &db), "Given by elder  ·  after The Lights  ·  2 steps  ·  25 coin, 80 XP");
        assert!(summary(&QuestDef::default(), &db).starts_with("Started by triggers or scripts"));
    }
}
