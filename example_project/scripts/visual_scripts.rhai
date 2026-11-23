// Made by R(ust)PG2D from the visual scripts (the editor's Nodes tab, defs/visual_scripts.ron).
// Don't edit it: it is written again whenever the definitions are saved.

// Brother Ansel's blessing
// Brother Ansel is a cleric of the Life domain (Database > Characters > Priest: class cleric):
// once a day he heals a hurt hero with Cure Wounds (the spell cast for the story: free).
fn ansel_blessing() {
    if (player_hp() >= player_max_hp()) {
        return;
    } else {
        if (get_var("ansel_blessing_day") == (day() + 1)) {
            return;
        } else {
            set_var("ansel_blessing_day", (day() + 1));
            say("Brother Ansel", "You are hurt, {player}. Be still - the light mends what it can.");
            cast_spell("cure-wounds", "player", true);
        }
    }
}

// A giant rat defeated
// Called by the giant rats' "Defeated" trigger (what they drop is set in their loot table).
// Tilda's quest counts giant rats once it was accepted (its first step is done at five).
fn rat_defeated() {
    add_var("rats_defeated", 1);
    if (quest_active("pest_control") && (quest_stage("pest_control") == 0)) {
        add_var("pest_rats", 1);
    }
    if (get_var("rats_defeated") == 5) {
        say("", "You have defeated 5 giant rats! You feel stronger... (not really)");
    }
}
