// Hard mode plugin: the damage the player's side takes and the healing it gets are scaled by the
// plugin's parameters (Assets › Plugins…). Priority 100: after the project's own hooks.

// The player and their companions (characters that aren't the player's side are left alone).
fn hard_mode_ours(target) {
    target == "player" || is_companion(target)
}

// `amount` times the parameter `param`, at least 1 (whole numbers come as integers).
fn hard_mode_scaled(amount, param) {
    let times = plugin_param("hard_mode", param);
    if type_of(times) == "i64" {
        times = times.to_float();
    }
    if type_of(times) != "f64" {
        return amount;
    }
    let v = (amount.to_float() * times).round().to_int();
    if v < 1 { 1 } else { v }
}

fn hard_mode_damage(m) {
    if m.amount > 0 && hard_mode_ours(m.target) {
        m.amount = hard_mode_scaled(m.amount, "damage");
        return m;
    }
}

fn hard_mode_heal(m) {
    if m.amount > 0 && hard_mode_ours(m.target) {
        m.amount = hard_mode_scaled(m.amount, "healing");
        return m;
    }
}
