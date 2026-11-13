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

// Adds friendship points (0 to 100); true when the character now counts the player a friend for
// the first time (five hearts).
fn friend_add(name, points) {
    let key = "friend_" + name;
    let v = get_var(key) + points;
    if v < 0 { v = 0; }
    if v > 100 { v = 100; }
    set_var(key, v);
    if v >= 50 && !flag("friend_reward_" + name) {
        set_flag("friend_reward_" + name, true);
        return true;
    }
    false
}

// A character definition's 'Interact' trigger: the first chat of the day counts a little.
fn friend_chat() {
    let name = self_name();
    if !(name in friend_tastes()) { return; }
    let key = "chat_day_" + name;
    if get_var(key) == day() + 1 { return; }
    set_var(key, day() + 1);
    if friend_add(name, 2) {
        say(friend_speaker(), "You're a true friend, {player}. Here, I want you to have this.");
        friend_reward(name);
    }
}

// The reward at five hearts (the line is said by the caller: helpers don't see SELF).
fn friend_reward(name) {
    give_item(friend_tastes()[name].reward, 1);
}

// The character's name as a speaker: "tomas" -> "Tomas" (a definition's name would be "Villager").
fn friend_speaker() {
    let n = self_name();
    if n.len() == 0 { return ""; }
    n.sub_string(0, 1).to_upper() + n.sub_string(1)
}

// Condition of the gift trigger: does this character take gifts?
fn friend_takes_gifts() {
    self_name() in friend_tastes()
}

// ---- Friendship and gifts ----------------------------------------------------------------
// Villagers like the player more for a chat a day and for gifts (an item used on them). Their
// friendship is the variable friend_<name> (0 to 100 points; the journal's People page shows ten
// hearts). Everyone who takes gifts has an entry here, by their name on the map: the gifts they
// love, like and dislike (anything else is still a kind thought) and what they give back once
// they count the player a friend (five hearts).
fn friend_tastes() {
    #{
        tomas: #{ loved: ["pumpkin"], liked: ["apple", "carrot", "cheese"], disliked: ["old_boot", "goblin_rags"], reward: "apple_pie" },
        ida: #{ loved: ["honey"], liked: ["bread", "milk", "moonpetal"], disliked: ["old_boot", "wolf_pelt"], reward: "tunic_green" },
        bram: #{ loved: ["honey"], liked: ["flour", "egg", "milk", "wheat"], disliked: ["mushroom", "old_boot"], reward: "festival_bread" },
        brom: #{ loved: ["ruby"], liked: ["iron_ore", "coal", "silver_ore", "ale"], disliked: ["herb", "moonpetal"], reward: "iron_ingot" },
        wren: #{ loved: ["moonpetal"], liked: ["herb", "feverleaf", "bitterroot", "mushroom"], disliked: ["ale", "wolf_pelt"], reward: "potion" },
        finn: #{ loved: ["ale"], liked: ["bread", "carrot_soup", "cheese"], disliked: ["trout", "old_boot"], reward: "grilled_trout" },
        greta: #{ loved: ["cheese"], liked: ["egg", "honey", "pumpkin"], disliked: ["old_boot", "goblin_rags"], reward: "pumpkin_stew" },
        elder: #{ loved: ["apple_pie"], liked: ["honey", "milk", "scroll_light"], disliked: ["ale"], reward: "scroll_light" },
        oswin: #{ loved: ["ale"], liked: ["bread", "apple", "cheese"], disliked: ["old_boot"], reward: "bread" },
        dagny: #{ loved: ["grilled_trout"], liked: ["bread", "ale", "cheese"], disliked: ["moonpetal"], reward: "bandage" },
        jorun: #{ loved: ["wheat"], liked: ["bread", "ale", "apple"], disliked: ["mushroom"], reward: "flour" },
        beth: #{ loved: ["carrot_seeds"], liked: ["egg", "milk", "pumpkin_seeds"], disliked: ["wolf_pelt"], reward: "pumpkin_seeds" },
        hal: #{ loved: ["pumpkin_stew"], liked: ["carrot", "apple", "ale"], disliked: ["old_boot"], reward: "wheat_seeds" },
        ansel: #{ loved: ["festival_bread"], liked: ["honey", "bread", "herb"], disliked: ["ale"], reward: "scroll_heal" },
    }
}

// A character definition's 'Use item' trigger (any item, if friend_takes_gifts()): one gift a day.
fn give_gift() {
    let name = self_name();
    let item = used_item();
    let t = friend_tastes()[name];
    let key = "gift_day_" + name;
    if get_var(key) == day() + 1 {
        // The game took a consumable already: give it back.
        if is_consumable(item) { give_item(item, 1); }
        say(friend_speaker(), "That's kind, but one gift a day is plenty!");
        return;
    }
    set_var(key, day() + 1);
    if !is_consumable(item) { take_item(item, 1); }
    let points = 3;
    if item in t.loved {
        points = 15;
        say(friend_speaker(), "Oh! I love this! Thank you so much, {player}!");
        emote("self", "love");
    } else if item in t.liked {
        points = 8;
        say(friend_speaker(), "How kind of you. Thank you!");
        emote("self", "joy");
    } else if item in t.disliked {
        points = -8;
        say(friend_speaker(), "Er... thanks, I suppose.");
        emote("self", "annoyed");
    } else {
        say(friend_speaker(), "For me? Thank you.");
    }
    if friend_add(name, points) {
        say(friend_speaker(), "You're a true friend, {player}. Here, I want you to have this.");
        friend_reward(name);
    }
}
