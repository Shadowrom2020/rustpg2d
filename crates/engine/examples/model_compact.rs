//! Writes a smaller copy of a model file (`model::compact`): textures downsized, and with `--clips`
//! only the animations. `cargo run -p rustpg2d-engine --example model_compact -- IN.glb OUT.glb [--clips]`
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(src), Some(dst)) = (args.first(), args.get(1)) else { anyhow::bail!("usage: model_compact IN.glb OUT.glb [--clips]") };
    let bytes = std::fs::read(src)?;
    let out = rustpg2d_engine::model::compact(&bytes, !args.iter().any(|a| a == "--clips"))?;
    std::fs::write(dst, &out)?;
    println!("{src}: {} KB -> {} KB", bytes.len() / 1024, out.len() / 1024);
    Ok(())
}
