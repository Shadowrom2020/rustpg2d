//! Prints the JSON Schema of a kind of data: `cargo run -p rustpg2d-engine --features schema
//! --example schema -- item|prop|…|project|entity|zone|trigger`.

fn main() {
    let what = std::env::args().nth(1).unwrap_or_default();
    let schema = match what.as_str() {
        "project" => Some(rustpg2d_engine::schema::project()),
        "entity" => Some(rustpg2d_engine::schema::entity()),
        "zone" => Some(rustpg2d_engine::schema::zone()),
        "trigger" => Some(rustpg2d_engine::schema::trigger()),
        kind => rustpg2d_engine::refactor::RefKind::parse(kind).and_then(rustpg2d_engine::schema::of_kind),
    };
    match schema {
        Some(s) => println!("{}", serde_json::to_string_pretty(&s).unwrap_or_default()),
        None => eprintln!("usage: schema KIND|project|entity|zone|trigger"),
    }
}
