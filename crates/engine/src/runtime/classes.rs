//! Classes at runtime: the hero's (`WorldState::class`) and party members' (their definition's,
//! then `Companion::class` with their own level): the hit die, the class's proficiencies, the
//! passive features ([`ClassTraits`]) the numbers read (armor class, speed, attacks, critical
//! hits, fighting styles, resistances…), what checks get from them (`Training`), and the XP split
//! among those who level. The level-up window is `levelup.rs`.

use crate::classes::{ClassDef, ClassTraits, FeatureChoice, FightingStyle, HeroClass};
use crate::srd::Ability;

use super::turns::Who;
use super::Game;

impl Game {
    /// The hero's class definition.
    pub(super) fn hero_class(&self) -> Option<&ClassDef> {
        self.project.db.class(&self.world.class.as_ref()?.class)
    }

    /// The hero's class features at their level (passive effects added up).
    pub(super) fn hero_traits(&self) -> Option<ClassTraits> {
        let hc = self.world.class.as_ref()?;
        Some(ClassTraits::of(self.project.db.class(&hc.class)?, self.world.player_level(), hc))
    }

    /// The hero's class feature ids at their level (chosen options included).
    pub(super) fn hero_features(&self) -> Vec<String> {
        match (&self.world.class, self.hero_class()) {
            (Some(hc), Some(c)) => c.feature_ids(self.world.player_level(), &hc.subclass, &hc.picks),
            _ => Vec::new(),
        }
    }

    /// Whether a combatant took the Grappler feat at an ability score improvement.
    pub(super) fn has_grappler(&self, who: Who) -> bool {
        let hc = match who {
            Who::Player => self.world.class.clone(),
            Who::Npc(uid) => self.member_class(uid).map(|(hc, _)| hc),
        };
        hc.is_some_and(|hc| hc.picks.values().flatten().any(|p| p == "feat:grappler"))
    }

    /// The hero's hit die (a d10 without a class).
    pub(super) fn hit_die(&self) -> i32 {
        self.hero_class().map_or(crate::progression::HIT_DIE, |c| c.hit_die as i32)
    }

    /// A combatant's class features: the hero's, a party member's.
    pub(super) fn who_traits(&self, who: Who) -> Option<ClassTraits> {
        match who {
            Who::Player => self.hero_traits(),
            Who::Npc(uid) => {
                let (hc, level) = self.member_class(uid)?;
                Some(ClassTraits::of(self.project.db.class(&hc.class)?, level, &hc))
            }
        }
    }

    /// A party member's key in pending level-ups: its home map and uid (`village:130`; its
    /// runtime uid changes from map to map).
    pub(super) fn member_key(&self, uid: u32) -> Option<String> {
        let l = self.loaded.entities.iter().find(|e| e.uid == uid)?.companion.as_ref()?;
        Some(format!("{}:{}", l.map, l.uid))
    }

    /// The runtime uid of the party member with that key, when it is on this map.
    pub(super) fn member_uid(&self, key: &str) -> Option<u32> {
        let (map, uid) = key.rsplit_once(':')?;
        let uid: u32 = uid.parse().ok()?;
        self.loaded.entities.iter().find(|e| e.companion.as_ref().is_some_and(|l| l.map == map && l.uid == uid)).map(|e| e.uid)
    }

    /// A party member's class and level: the saved one, else its definition's.
    pub(super) fn member_class(&self, uid: u32) -> Option<(HeroClass, u32)> {
        let e = self.loaded.entities.iter().find(|e| e.uid == uid)?;
        let link = e.companion.as_ref()?;
        let saved = self.world.companions.iter().find(|c| c.uid == link.uid && c.map == link.map);
        let def = self.project.db.character(&e.def);
        let hc = saved.and_then(|c| c.class.clone()).or_else(|| def.and_then(|d| d.class.clone()))?;
        let level = saved.map(|c| c.level).filter(|l| *l > 0).or_else(|| def.map(|d| d.level.max(1))).unwrap_or(1);
        Some((hc, level))
    }

    /// Gives the hero a class (the creator, *Set class*): its saving throws and armor, weapon and
    /// tool proficiencies; the level's choices wait for the level-up window.
    pub(super) fn set_hero_class(&mut self, class: &str) {
        let Some(c) = self.project.db.class(class).cloned() else {
            eprintln!("[classes] no class '{class}'");
            return;
        };
        self.world.class = Some(HeroClass { class: class.into(), ..Default::default() });
        // The choices of the levels it has now (level 1's fighting style, expertise, a cleric's domain).
        for level in 1..=self.world.player_level() {
            if self.level_has_choices(&c, level) {
                self.world.levelups.push(crate::state::PendingLevel { who: "player".into(), level });
            }
        }
        self.refresh_training();
    }

    /// *Set class* / `set_class`: the hero's (`target` "player") or a party member's class; `level`
    /// 0 keeps the level (a party member's is set when above 0).
    pub(super) fn set_class_of(&mut self, target: &str, class: &str, level: u32, owner: Option<u32>) {
        if target.eq_ignore_ascii_case("player") {
            self.set_hero_class(class);
            return;
        }
        let Some(c) = self.project.db.class(class).cloned() else {
            eprintln!("[classes] no class '{class}'");
            return;
        };
        let Some(i) = self.entity_index(target, owner) else {
            eprintln!("[classes] Set class: no character '{target}' on this map");
            return;
        };
        let e = &self.loaded.entities[i];
        let (uid, def_level) = (e.uid, self.project.db.character(&e.def).map_or(1, |d| d.level.max(1)));
        let Some(link) = e.companion.clone() else {
            eprintln!("[classes] Set class: '{target}' is not a party member");
            return;
        };
        let Some(m) = self.world.companions.iter_mut().find(|m| m.uid == link.uid && m.map == link.map) else { return };
        m.class = Some(HeroClass { class: class.into(), ..Default::default() });
        if level > 0 {
            m.level = level;
        }
        // Experience and hit point rolls to match the level (averages for the levels it skipped).
        if m.level > 0 {
            m.xp = m.xp.max(self.project.config.progression.xp_for_level(m.level));
            m.hp_rolls.resize((m.level as usize).saturating_sub(1).max(m.hp_rolls.len()), c.hit_die as i32 / 2 + 1);
        }
        let at = if m.level > 0 { m.level } else { def_level };
        let key = format!("{}:{}", link.map, link.uid);
        for l in 1..=at {
            if self.level_has_choices(&c, l) {
                self.world.levelups.push(crate::state::PendingLevel { who: key.clone(), level: l });
            }
        }
        self.refresh_member(uid);
    }

    /// Whether reaching `level` of class `c` asks for choices (subclass, ability scores,
    /// expertise, a fighting style, skills).
    pub(super) fn level_has_choices(&self, c: &ClassDef, level: u32) -> bool {
        level == c.subclass_level || c.levels.get(level as usize - 1).is_some_and(|l| l.features.iter().any(|f| c.feature(f).is_some_and(|f| f.choice.is_some()))) || Self::spell_gains(c, level) != (0, 0)
    }

    /// What the hero's class adds to checks (after every change of class, level or choice, and
    /// after loading).
    pub(super) fn refresh_training(&mut self) {
        let Some(t) = self.hero_traits() else {
            self.world.training = Default::default();
            return;
        };
        // The class's saving throws and proficiencies (also for a save that was given its class
        // by hand), then those its features add.
        if let Some(c) = self.hero_class() {
            let base: Vec<String> = c.saves.iter().map(|s| format!("save:{s}")).chain(c.proficiencies.iter().cloned()).collect();
            for p in base {
                self.world.set_proficient(&p, true);
            }
        }
        for p in &t.proficiencies {
            self.world.set_proficient(p, true);
        }
        self.world.training = crate::srd::check::Training { expertise: t.expertise.iter().map(|e| crate::srd::abilities::proficiency_id(e)).collect(), half: t.half_prof.clone(), half_all: t.all_half, reliable: t.reliable, save_advantage: t.save_advantage.clone() };
    }

    /// The hero's armor class with the class: an unarmored formula when no body armor is worn
    /// (the best of them), the Defense style's +1 in armor.
    pub(super) fn class_armor_class(&self, worn: i32, body_armor: bool, shield: i32) -> i32 {
        let Some(t) = self.hero_traits() else { return worn };
        let a = &self.world.abilities;
        let mut ac = worn;
        if !body_armor {
            for (base, abilities, shield_ok) in &t.armor {
                if !shield_ok && shield > 0 {
                    continue;
                }
                ac = ac.max(base + abilities.iter().map(|x| a.modifier(*x)).sum::<i32>() + shield);
            }
        } else if t.styles.contains(&FightingStyle::Defense) {
            ac += 1;
        }
        ac
    }

    /// Feet the class adds to the hero's speed (`unarmored`: without armor and shield).
    pub(super) fn class_speed_ft(&self, unarmored: bool) -> u32 {
        self.hero_traits().map_or(0, |t| t.speed_ft + if unarmored { t.speed_unarmored_ft } else { 0 })
    }

    /// The party members on this map that level (they have a class).
    pub(super) fn levelling_members(&self) -> Vec<u32> {
        self.loaded.entities.iter().filter(|e| e.active && e.companion.is_some() && self.member_class(e.uid).is_some()).map(|e| e.uid).collect()
    }

    /// `class=` for status: `fighter/champion:5` (the hero's), and the party's with a class.
    pub(super) fn class_status(&self) -> String {
        let hero = match &self.world.class {
            Some(hc) => format!("{}{}:{}", hc.class, if hc.subclass.is_empty() { String::new() } else { format!("/{}", hc.subclass) }, self.world.player_level()),
            None => "-".into(),
        };
        let party: Vec<String> = self
            .loaded
            .entities
            .iter()
            .filter(|e| e.companion.is_some())
            .filter_map(|e| {
                let (hc, level) = self.member_class(e.uid)?;
                Some(format!("{}={}{}:{level}", if e.name.is_empty() { &e.def } else { &e.name }, hc.class, if hc.subclass.is_empty() { String::new() } else { format!("/{}", hc.subclass) }))
            })
            .collect();
        if party.is_empty() {
            hero
        } else {
            format!("{hero}[{}]", party.join(" "))
        }
    }

    /// The choices a level of a class asks for, as (feature id, choice): the subclass (feature id
    /// `subclass`) and the features of the level with a choice.
    pub(super) fn level_choices(c: &ClassDef, level: u32) -> Vec<(String, FeatureChoice)> {
        let mut out = Vec::new();
        if level == c.subclass_level && !c.subclasses.is_empty() {
            out.push(("subclass".to_string(), FeatureChoice::Options { choose: 1, options: c.subclasses.iter().map(|s| s.id.clone()).collect() }));
        }
        if let Some(l) = c.levels.get(level as usize - 1) {
            for f in &l.features {
                if let Some(ch) = c.feature(f).and_then(|x| x.choice.clone()) {
                    out.push((f.clone(), ch));
                }
            }
        }
        out
    }

    /// An ability score improvement's or a feat's pick applied: `ability:str` raises STR by one
    /// (at most 20), `feat:grappler` is kept as the pick.
    pub(super) fn apply_ability_pick(abilities: &mut crate::srd::Abilities, pick: &str) {
        if let Some(a) = pick.strip_prefix("ability:").and_then(Ability::parse) {
            let v = abilities.score(a);
            abilities.set(a, (v + 1).min(20));
        }
    }

    /// A party member's stat block from its class and level: its definition's scores (with its
    /// ability score improvements), the class's saving throws and proficiencies, hit points by
    /// the hit die, armor class from its gear (or an unarmored formula), speed, and an attack with
    /// its weapon (proficiency + STR or DEX, the dice + the modifier; Extra Attack as Multiattack).
    pub(super) fn member_block(&self, uid: u32) -> Option<crate::srd::StatBlock> {
        let (hc, level) = self.member_class(uid)?;
        let c = self.project.db.class(&hc.class)?;
        let e = self.loaded.entities.iter().find(|e| e.uid == uid)?;
        let def = self.project.db.character(&e.def);
        let mut b = def.and_then(|d| d.stats.clone()).unwrap_or_default();
        for v in hc.picks.values() {
            for x in v {
                Self::apply_ability_pick(&mut b.abilities, x);
            }
        }
        let t = ClassTraits::of(c, level, &hc);
        for (a, amount, max) in &t.ability_bonus {
            let v = b.abilities.score(*a);
            b.abilities.set(*a, (v + amount).min(*max));
        }
        let m = |a: Ability| b.abilities.modifier(a);
        // Proficiencies: the class's saves and its own, its features'.
        let mut profs: Vec<String> = b.proficiencies.clone();
        profs.extend(c.saves.iter().map(|s| format!("save:{s}")));
        profs.extend(c.proficiencies.iter().cloned());
        profs.extend(t.proficiencies.iter().cloned());
        profs.sort();
        profs.dedup();
        b.proficiencies = profs;
        let rolls = self.world.companions.iter().find(|x| e.companion.as_ref().is_some_and(|l| l.uid == x.uid && l.map == x.map)).map(|x| x.hp_rolls.clone()).unwrap_or_default();
        b.hp = crate::progression::max_hp_die(c.hit_die as i32, level, m(Ability::Con), &rolls, t.hp_per_level);
        b.hit_dice = format!("{level}d{}", c.hit_die);
        // Armor class: its gear's armor, else the best unarmored formula.
        let pieces: Vec<(crate::defs::EquipSlot, i32)> = [crate::defs::EquipSlot::Clothing, crate::defs::EquipSlot::Head, crate::defs::EquipSlot::Body, crate::defs::EquipSlot::Offhand]
            .into_iter()
            .filter_map(|s| e.equipment.item(s).and_then(|id| self.project.db.item(id)?.armor.as_ref()).map(|a| (s, a.defense)))
            .collect();
        let worn = 10 + m(Ability::Dex) + pieces.iter().map(|p| p.1).sum::<i32>();
        let body = pieces.iter().any(|p| p.0 == crate::defs::EquipSlot::Body);
        let shield = pieces.iter().find(|p| p.0 == crate::defs::EquipSlot::Offhand).map_or(0, |p| p.1);
        let mut ac = worn;
        if !body {
            for (base, abilities, shield_ok) in &t.armor {
                if *shield_ok || shield == 0 {
                    ac = ac.max(base + abilities.iter().map(|a| m(*a)).sum::<i32>() + shield);
                }
            }
        } else if t.styles.contains(&FightingStyle::Defense) {
            ac += 1;
        }
        b.ac = ac;
        b.speed_ft = b.speed_ft.max(5) + t.speed_ft + if body || shield > 0 { 0 } else { t.speed_unarmored_ft };
        // (Its proficiency bonus follows its level: `proficiency_level` reads `cr`.)
        b.cr = level.to_string();
        let prof = crate::srd::abilities::proficiency_bonus(level);
        let w = e.weapon.clone();
        let ranged = w.as_ref().is_some_and(|w| w.ranged.is_some());
        let unarmed = w.is_none();
        let mut ability = if ranged { m(Ability::Dex) } else { m(Ability::Str) };
        let mut dice = w.as_ref().map_or_else(|| "1".to_string(), |w| w.damage_dice());
        if unarmed && t.martial_arts {
            ability = ability.max(m(Ability::Dex));
            dice = format!("1d{}", c.number(level, "martial_arts.dice_value").max(4));
        }
        let mut bonus = prof + ability;
        if ranged && t.styles.contains(&FightingStyle::Archery) {
            bonus += 2;
        }
        let dmg = ability + if !ranged && !unarmed && t.styles.contains(&FightingStyle::Dueling) { 2 } else { 0 };
        let dice = if dmg == 0 { dice } else { format!("{dice}{dmg:+}") };
        let (range_ft, long_ft) = w.as_ref().and_then(|w| w.ranged.as_ref()).map_or((0, 0), |r| ((r.range.round() as u32) * 5, (if r.long_range > 0.0 { r.long_range } else { r.range * 2.0 }).round() as u32 * 5));
        let reach_ft = if ranged { 0 } else { w.as_ref().map_or(5, |w| (w.reach.round().max(1.0) as u32) * 5) };
        let name = w.as_ref().map_or("Unarmed strike".to_string(), |_| e.equipment.item(crate::defs::EquipSlot::Weapon).and_then(|i| self.project.db.item(i)).map_or("Attack".into(), |i| i.name.clone()));
        b.attacks = vec![crate::srd::StatAttack { name, bonus, dice, damage_type: w.as_ref().and_then(|w| w.damage_type), reach_ft, range_ft, long_ft }];
        b.multiattack = t.attacks;
        b.resistances.extend(t.resistances.iter().cloned());
        b.immunities.extend(t.immunities.iter().cloned());
        b.condition_immunities.extend(t.condition_immunities.iter().cloned());
        Some(b)
    }

    /// A party member's hit points follow its class and level (after joining, a level-up, a
    /// choice): the max from its stat block, healed by as much as the max grew.
    pub(super) fn refresh_member(&mut self, uid: u32) {
        let Some(b) = self.member_block(uid) else { return };
        if let Some(e) = self.loaded.entities.iter_mut().find(|e| e.uid == uid) {
            let grew = (b.hp - e.max_hp).max(0);
            e.max_hp = b.hp;
            e.hp = (e.hp + grew).min(e.max_hp);
        }
    }

}
