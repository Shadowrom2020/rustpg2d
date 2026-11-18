// Calendar plugin: the date as the text variable calendar_text (its HUD line shows it).
// The map_enter hook starts it; its common event "calendar_tick" refreshes it every two seconds
// while the flag calendar_on is set (midnight passes, sleeping skips days).

fn calendar_enter(m) {
    set_flag("calendar_on", true);
    calendar_update();
}

// The names of a comma-separated parameter (empty parts left out).
fn calendar_names(param, fallback) {
    let names = [];
    let text = plugin_param("calendar", param);
    if type_of(text) != "string" || text == "" {
        text = fallback;
    }
    for part in text.split(",") {
        let n = part;
        n.trim();
        if n != "" {
            names.push(n);
        }
    }
    names
}

fn calendar_update() {
    let per = plugin_param("calendar", "days_per_season");
    if type_of(per) == "f64" {
        per = per.to_int();
    }
    if type_of(per) != "i64" || per < 1 {
        per = 28;
    }
    let seasons = calendar_names("seasons", "Spring,Summer,Autumn,Winter");
    let weekdays = calendar_names("weekdays", "Moonday,Tideday,Windday,Thunderday,Fireday,Starday,Sunday");
    let d = day();
    let season = seasons[(d / per) % seasons.len()];
    let weekday = weekdays[d % weekdays.len()];
    let text = weekday + ", " + season + " " + (d % per + 1);
    if plugin_param("calendar", "show_year") == true {
        text += ", year " + (d / (per * seasons.len()) + 1);
    }
    set_text("calendar_text", text);
}
