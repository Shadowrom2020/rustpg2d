//! The example's music, composed in a small note notation and rendered by a tiny synthesizer
//! (`music.rs`, from the example generator of 0.10): *Market Day* (village), *Whisperwood*
//! (forest), *The Copper Kettle* (tavern), *Steel in the Woods* (battle), *The Old Mine*
//! (dungeons), *Lanterns at Night* (towns at night), *The Open Road* (travel) and *Grumbold's
//! Wrath* (a boss fight).
//!
//! `cargo run --release -- OUT_DIR [ID…]` (in this folder) writes `<id>.ogg` for the given ids,
//! default all eight (`music::TRACKS`).
mod music;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().expect("usage: music OUT_DIR [ID…]"));
    let mut ids: Vec<String> = args.collect();
    if ids.is_empty() {
        ids = music::TRACKS.iter().map(|t| t.0.to_string()).collect();
    }
    std::fs::create_dir_all(&out).unwrap();
    for id in ids {
        let (l, r) = music::render(&id);
        let path = out.join(format!("{id}.ogg"));
        std::fs::write(&path, music::ogg(&id, &l, &r)).unwrap();
        println!("wrote {}: {:.1} s", path.display(), l.len() as f32 / music::SR as f32);
    }
}
