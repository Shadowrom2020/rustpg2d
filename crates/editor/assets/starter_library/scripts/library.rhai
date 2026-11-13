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

// The signpost the player stands at (its index in signposts()), -1 if none is near.
fn signpost_here() {
    let list = signposts();
    let best = -1;
    let best_d = 3.0;
    for i in 0..list.len() {
        let s = list[i];
        if s.map == current_map() {
            let dx = player_x() - (s.x + 0.5);
            let dy = player_y() - (s.y + 1.0);
            let d = (dx * dx + dy * dy).sqrt();
            if d < best_d {
                best = i;
                best_d = d;
            }
        }
    }
    best
}

fn signpost_hours(a, b) {
    let h = a.hours - b.hours;
    if h < 0 { h = -h; }
    if h < 1 { h = 1; }
    h
}

fn signpost_menu() {
    let list = signposts();
    let here = signpost_here();
    if here < 0 {
        say(text("Signpost"), "The arrows point down the roads.");
        return;
    }
    let me = list[here];
    set_flag("signpost_found_" + me.id, true);
    let h = hour();
    if h >= 21.0 || h < 5.0 {
        say(me.name, "It is too dark to travel the roads. Wait for the morning.");
        return;
    }
    let options = [];
    let n = 0;
    for i in 0..list.len() {
        if i != here && flag("signpost_found_" + list[i].id) {
            let there = list[i];
            let hours = signpost_hours(me, there);
            let label = there.name;
            label += " (";
            label += hours;
            label += " h)";
            options.push(label);
            set_var("signpost_option_" + n, i);
            n += 1;
        }
    }
    if n == 0 {
        say(me.name, "Other signposts you find on your way will show up here.");
        return;
    }
    options.push(text("Stay here"));
    set_var("signpost_from", here);
    set_var("signpost_options", n);
    choice("Where to?", options, "signpost_pick");
    then_call("signpost_travel");
}

fn signpost_travel() {
    let k = get_var("signpost_pick");
    if k >= get_var("signpost_options") {
        return;
    }
    let list = signposts();
    let from = list[get_var("signpost_from")];
    let to = list[get_var("signpost_option_" + k)];
    let arrive = hour() + signpost_hours(from, to);
    if arrive >= 24.0 {
        say(from.name, "You wouldn't get there before midnight. Better rest first.");
        return;
    }
    set_hour(arrive);
    teleport(to.map, to.arrive_at);
}

// ---- Fast travel by signposts ------------------------------------------------------------
// The Signpost prop's Interact trigger runs signpost_menu(). Touching a signpost marks it found;
// it offers the other signposts found so far, and travelling takes hours on the clock (the
// difference of their `hours` along the roads, at least one). No travel at night (21:00-5:00).
// Every signpost of the game, one entry each: its name, map, tile (where the prop stands) and
// the spawn point travellers arrive at (`arrive_at`).
fn signposts() {
    [
        #{ id: "square", name: text("Village square"), map: "village", x: 37.0, y: 17.0, arrive_at: "signpost_square", hours: 0 },
        #{ id: "whisperwood", name: text("Whisperwood clearing"), map: "forest", x: 46.0, y: 25.0, arrive_at: "signpost_whisperwood", hours: 2 },
        #{ id: "mine", name: text("The old mine"), map: "forest", x: 49.0, y: 8.0, arrive_at: "signpost_mine", hours: 3 },
        #{ id: "wilds", name: text("The Wilds"), map: "wilds", x: 510.0, y: 511.0, arrive_at: "signpost_wilds", hours: 5 },
    ]
}
