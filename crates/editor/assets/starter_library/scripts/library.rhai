// Script functions that came with assets from the asset library or asset packages
// (spells, skills, triggers). Edit them like any other script.

// Fireball, a scripted spell (Database > Spells > Fireball, effect "Script"). Casting it calls
// cast_fireball(); aim() lets the player click a target and calls fireball_at(x, y) with it (in
// tiles). The spell's mana is only spent once a target is chosen.
fn cast_fireball() {
    aim("fireball_at");
}

fn fireball_at(x, y) {
    play_sound("fireball");
    // An animated sprite (all frames of the "fireball" sheet) flies there and glows on the way.
    fly("fireball", x, y, "fireball_hit", #{ speed: 9, fps: 14, light: 2.5 });
}

fn fireball_hit(x, y) {
    play_sound("explosion");
    effect("explosion", x, y, #{ fps: 14, scale: 2.4, light: 5, shake: 0.3 });
    damage_area(x, y, 2, 5);
    // Embers glow on for a few seconds.
    light_at(x, y, 2, 4, #{ color: [255, 120, 50], flicker: true });
}

// Called by the slime's "Defeated" trigger (what slimes drop is set in their loot table).
// A scripted skill effect (Database > Skills > Scavenging, effect "Script"): the engine calls
// scavenging(level) whenever the level changes, and once when a game starts or loads.
fn scavenging(level) {
    set_var("scavenging", level);
}

// The baker's apple pies, baked at dawn.
fn restock_baking() {
    restock_from_activity(SELF, "apple_pie", 2, 6);
}

// The fisher's catch of the day before (he fishes at dawn and in the afternoon).
fn restock_catch() {
    restock_from_activity(SELF, "trout", 3, 8);
}

fn restock_from_activity(owner, item, per_day, max) {
    let key = "stocked_day_" + owner + "_" + item;
    let days = day() - get_var(key);
    if days > 0 {
        let add = days * per_day;
        let room = max - stock_of("self", item);
        if add > room {
            add = room;
        }
        if add > 0 {
            add_stock("self", item, add);
        }
        set_var(key, day());
    }
}
