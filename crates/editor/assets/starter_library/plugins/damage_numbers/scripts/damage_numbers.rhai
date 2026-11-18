// Damage number styles plugin: the damage_number hook gets #{value, kind, text, color, size}
// once per number (kind: "hit", "crit" or "hurt") and returns it restyled.

fn damage_numbers_style(m) {
    let style = plugin_param("damage_numbers", "style");
    let v = "" + m.value;
    if m.kind == "hurt" && plugin_param("damage_numbers", "minus") != false {
        v = "-" + v;
    }
    if style == "comic" {
        if m.kind == "crit" {
            m.text = "POW! " + v;
            m.color = "#ff8a1c";
            m.size = 1.3;
        } else if m.kind == "hurt" {
            m.text = "OUCH " + v;
            m.color = "#ff4d6d";
        } else {
            m.text = v;
            m.color = "#ffe45c";
            m.size = 1.1;
        }
    } else if style == "quiet" {
        m.text = v;
        m.size = 0.75;
        m.color = if m.kind == "hurt" { "#e8a0a0" } else { "#e8e8e8" };
    } else {
        // bold (also for an unknown style)
        m.text = if m.kind == "crit" { v + "!" } else { v };
        m.size = if m.kind == "crit" { 1.6 } else { 1.35 };
    }
    m
}
