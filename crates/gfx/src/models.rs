//! Meshes: triangles of your own.

use crate::batch::DrawMode;
use crate::get_context;
use crate::texture::Texture2D;

pub use crate::batch::Vertex;

/// Triangles with one texture. At most 65535 vertices.
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub texture: Option<Texture2D>,
}

pub fn draw_mesh(mesh: &Mesh) {
    let context = get_context();
    context.gl.texture(mesh.texture.as_ref());
    context.gl.draw_mode(DrawMode::Triangles);
    context.gl.geometry(&mesh.vertices[..], &mesh.indices[..]);
}
