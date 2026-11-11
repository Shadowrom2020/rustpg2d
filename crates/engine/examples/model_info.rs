//! Prints what the engine reads from a `.glb`: mesh, bones, clips (with the direction each faces),
//! texture. `cargo run -p rustpg2d-engine --example model_info -- MODEL.glb [CLIPS.glb…]`
use rustpg2d_engine::model::Model;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().ok_or_else(|| anyhow::anyhow!("usage: model_info MODEL.glb [CLIPS.glb…]"))?;
    let mut m = Model::load(&std::fs::read(first)?)?;
    for extra in &args[1..] {
        println!("{extra}: {} clip(s)", m.add_clips(&std::fs::read(extra)?)?);
    }
    let (lo, hi) = m.bounds();
    println!("vertices {} triangles {} joints {} nodes {}", m.vertices.len(), m.indices.len() / 3, m.joints.len(), m.nodes.len());
    println!("bounds {lo:?} .. {hi:?} (height {:.3})", hi.y - lo.y);
    println!("texture {:?}", m.texture.as_ref().map(|t| (t.width, t.height)));
    println!("bones {:?}", m.joints.iter().map(|&j| m.nodes[j].name.as_str()).collect::<Vec<_>>());
    println!("rest facing {:?}", m.facing_of(None).map(f32::to_degrees));
    for c in &m.clips {
        println!("clip {:<28} {:.2}s tracks {:>3} facing {:?}", c.name, c.duration, c.tracks.len(), m.facing_of(Some(c)).map(|d| d.to_degrees().round()));
    }
    Ok(())
}
