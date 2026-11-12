//! 3D character models: rigged glTF binaries (`.glb` under `models/`) as Meshy, Blender and most
//! tools write them. A [`Model`] keeps the skinned mesh (positions, normals, UVs, four joints and
//! weights per vertex), the node tree with the skin's joints, the animation clips (more can come
//! from other files of the same rig: [`Model::add_clips`]) and the base-colour texture, downsized
//! for the pixel look. [`Model::pose`] samples a clip into node transforms (root motion removed),
//! [`Model::skin`] deforms the mesh. Drawing is `render/model.rs`.

use anyhow::{anyhow, bail, Context, Result};
use macroquad::math::{Mat4, Quat, Vec2, Vec3};
use macroquad::texture::Image;

/// One vertex of the skinned mesh in its bind pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinVertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

/// A node of the model's tree (bones are nodes) with its rest transform.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    pub t: Vec3,
    pub r: Quat,
    pub s: Vec3,
}

/// What a track animates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Translation,
    Rotation,
    Scale,
}

/// The keyframes of one node property.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub node: usize,
    pub path: Path,
    /// Hold each key until the next (else linear: lerp, slerp for rotations).
    pub step: bool,
    pub times: Vec<f32>,
    /// x, y, z(, w) per key.
    pub values: Vec<[f32; 4]>,
}

/// An animation clip.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: String,
    /// Seconds.
    pub duration: f32,
    pub tracks: Vec<Track>,
}

/// A loaded model.
#[derive(Clone, Debug)]
pub struct Model {
    pub vertices: Vec<SkinVertex>,
    pub indices: Vec<u16>,
    pub nodes: Vec<Node>,
    /// Node indices, parents before children.
    order: Vec<usize>,
    /// The skin's joints (node indices) and their inverse bind matrices.
    pub joints: Vec<usize>,
    pub inverse_bind: Vec<Mat4>,
    pub clips: Vec<Clip>,
    /// The base-colour texture (at most [`TEXTURE_MAX`] pixels on a side), if any.
    pub texture: Option<Image>,
}

/// Textures are downsized to this: figures are drawn a few dozen pixels tall.
pub const TEXTURE_MAX: u16 = 256;

impl Model {
    /// Reads a `.glb` (one skinned mesh; several primitives are merged, the first textured
    /// material gives the texture).
    pub fn load(glb: &[u8]) -> Result<Model> {
        let g = gltf::Gltf::from_slice(glb).context("not a glTF binary")?;
        let blob = g.blob.as_deref();
        let buf = |b: gltf::Buffer| match b.source() {
            gltf::buffer::Source::Bin => blob,
            gltf::buffer::Source::Uri(_) => None,
        };
        let nodes = read_nodes(&g.document);
        let order = topo_order(&nodes);
        let mesh_node = g.document.nodes().find(|n| n.mesh().is_some() && n.skin().is_some()).or_else(|| g.document.nodes().find(|n| n.mesh().is_some())).ok_or_else(|| anyhow!("the file has no mesh"))?;
        let mesh = mesh_node.mesh().unwrap();
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        let mut texture = None;
        // Parts without a texture (models made of plain-coloured materials) get one texel each
        // of a palette texture with their material's colour.
        let prims: Vec<_> = mesh.primitives().filter(|p| p.mode() == gltf::mesh::Mode::Triangles).collect();
        let untextured = prims.iter().all(|p| p.material().pbr_metallic_roughness().base_color_texture().is_none());
        let n_parts = prims.len().max(1);
        let mut palette = Vec::new();
        for (part, p) in prims.iter().enumerate() {
            let r = p.reader(buf);
            let pos: Vec<[f32; 3]> = r.read_positions().ok_or_else(|| anyhow!("a mesh part without positions"))?.collect();
            let n = pos.len();
            let normals: Vec<[f32; 3]> = r.read_normals().map(|i| i.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; n]);
            let uvs: Vec<[f32; 2]> = if untextured {
                let c = p.material().pbr_metallic_roughness().base_color_factor();
                palette.extend(c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
                vec![[(part as f32 + 0.5) / n_parts as f32, 0.5]; n]
            } else {
                r.read_tex_coords(0).map(|i| i.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; n])
            };
            let joints: Vec<[u16; 4]> = r.read_joints(0).map(|i| i.into_u16().collect()).unwrap_or_else(|| vec![[0; 4]; n]);
            let weights: Vec<[f32; 4]> = r.read_weights(0).map(|i| i.into_f32().collect()).unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 0.0]; n]);
            let base = vertices.len() as u32;
            if base as usize + n > u16::MAX as usize {
                bail!("the model has more than {} vertices: reduce it to about 5000 triangles", u16::MAX);
            }
            for k in 0..n {
                let w = weights[k];
                let sum = (w[0] + w[1] + w[2] + w[3]).max(1e-6);
                vertices.push(SkinVertex { pos: pos[k].into(), normal: normals[k].into(), uv: uvs[k].into(), joints: joints[k], weights: w.map(|x| x / sum) });
            }
            match r.read_indices() {
                Some(ix) => indices.extend(ix.into_u32().map(|i| (i + base) as u16)),
                None => indices.extend((0..n as u32).map(|i| (i + base) as u16)),
            }
            if texture.is_none() {
                if let Some(t) = p.material().pbr_metallic_roughness().base_color_texture() {
                    texture = read_image(&t.texture().source(), blob).ok();
                }
            }
        }
        if vertices.is_empty() {
            bail!("the mesh has no triangles");
        }
        if untextured && !palette.is_empty() {
            let mut im = Image::gen_image_color(n_parts as u16, 1, macroquad::color::WHITE);
            im.bytes.copy_from_slice(&palette);
            texture = Some(im);
        }
        let (joints, inverse_bind) = match mesh_node.skin() {
            Some(skin) => {
                let joints: Vec<usize> = skin.joints().map(|j| j.index()).collect();
                let ib: Vec<Mat4> = match skin.reader(buf).read_inverse_bind_matrices() {
                    Some(m) => m.map(|c| Mat4::from_cols_array_2d(&c)).collect(),
                    None => vec![Mat4::IDENTITY; joints.len()],
                };
                (joints, ib)
            }
            // Unrigged: one joint, the mesh's own node (rig templates add bones later).
            None => (vec![mesh_node.index()], vec![Mat4::IDENTITY]),
        };
        let mut m = Model { vertices, indices, nodes, order, joints, inverse_bind, clips: Vec::new(), texture };
        m.clips = read_clips(&g.document, blob, &|i| Some(i));
        Ok(m)
    }

    /// Adds the clips of another `.glb` of the same rig (its nodes matched by name). Returns
    /// how many were added; clips named like an existing one replace it.
    pub fn add_clips(&mut self, glb: &[u8]) -> Result<usize> {
        let g = gltf::Gltf::from_slice(glb).context("not a glTF binary")?;
        let names: Vec<String> = g.document.nodes().map(|n| n.name().unwrap_or("").to_string()).collect();
        let map = |i: usize| self.nodes.iter().position(|n| !n.name.is_empty() && n.name == names[i]);
        let clips = read_clips(&g.document, g.blob.as_deref(), &map);
        let n = clips.len();
        for c in clips {
            self.clips.retain(|o| o.name != c.name);
            self.clips.push(c);
        }
        Ok(n)
    }

    pub fn clip(&self, name: &str) -> Option<&Clip> {
        self.clips.iter().find(|c| c.name == name)
    }

    /// The node whose name contains `part` (case-insensitive), the shortest such name first
    /// (`"hand"` finds `RightHand` before `RightHandThumb1`).
    pub fn node(&self, part: &str) -> Option<usize> {
        let p = part.to_lowercase();
        self.nodes.iter().enumerate().filter(|(_, n)| n.name.to_lowercase().contains(&p)).min_by_key(|(_, n)| n.name.len()).map(|(i, _)| i)
    }

    /// The root bone (the hips): the skin's first joint.
    pub fn root_joint(&self) -> Option<usize> {
        self.node("hips").or_else(|| self.joints.first().copied())
    }

    /// World transforms of every node for `clip` at `t` seconds (looping; None = the rest pose).
    /// The root bone's horizontal movement is removed, so walking stays in place.
    pub fn pose(&self, clip: Option<&Clip>, t: f32) -> Vec<Mat4> {
        let mut local: Vec<(Vec3, Quat, Vec3)> = self.nodes.iter().map(|n| (n.t, n.r, n.s)).collect();
        if let Some(c) = clip {
            let t = if c.duration > 0.0 { t.rem_euclid(c.duration) } else { 0.0 };
            for tr in &c.tracks {
                let v = sample(tr, t);
                let l = &mut local[tr.node];
                match tr.path {
                    Path::Translation => l.0 = Vec3::new(v[0], v[1], v[2]),
                    Path::Rotation => l.1 = Quat::from_xyzw(v[0], v[1], v[2], v[3]).normalize(),
                    Path::Scale => l.2 = Vec3::new(v[0], v[1], v[2]),
                }
            }
        }
        let mut world = vec![Mat4::IDENTITY; self.nodes.len()];
        for &i in &self.order {
            let (t, r, s) = local[i];
            let m = Mat4::from_scale_rotation_translation(s, r, t);
            world[i] = match self.nodes[i].parent {
                Some(p) => world[p] * m,
                None => m,
            };
        }
        if let (Some(_), Some(root)) = (clip, self.root_joint()) {
            let rest = self.pose(None, 0.0)[root].w_axis.truncate();
            let now = world[root].w_axis.truncate();
            let shift = Mat4::from_translation(Vec3::new(rest.x - now.x, 0.0, rest.z - now.z));
            for w in &mut world {
                *w = shift * *w;
            }
        }
        world
    }

    /// The deformed mesh for node transforms `world` (from [`Model::pose`]): positions and
    /// normals per vertex, in the model's space.
    pub fn skin(&self, world: &[Mat4], out: &mut Vec<(Vec3, Vec3)>) {
        let mats: Vec<Mat4> = self.joints.iter().zip(&self.inverse_bind).map(|(&j, ib)| world[j] * *ib).collect();
        out.clear();
        out.extend(self.vertices.iter().map(|v| {
            let mut m = Mat4::ZERO;
            for k in 0..4 {
                if v.weights[k] > 0.0 {
                    if let Some(j) = mats.get(v.joints[k] as usize) {
                        m += *j * v.weights[k];
                    }
                }
            }
            (m.transform_point3(v.pos), m.transform_vector3(v.normal).normalize_or_zero())
        }));
    }

    /// The direction a clip's body faces on average, as a turn about the up axis (radians, 0 =
    /// facing +Z, the glTF front): square to the line through the hips and the shoulders. None
    /// without such bones. Turning by its negative makes the clip face the front.
    pub fn facing_of(&self, clip: Option<&Clip>) -> Option<f32> {
        let bones = ["leftupleg", "rightupleg", "leftarm", "rightarm"].map(|b| self.node(b));
        let [Some(lu), Some(ru), Some(la), Some(ra)] = bones else { return None };
        let n = if clip.is_some() { 16 } else { 1 };
        let mut acc = Vec3::ZERO;
        for k in 0..n {
            let t = clip.map_or(0.0, |c| c.duration * k as f32 / n as f32);
            let w = self.pose(clip, t);
            let p = |i: usize| w[i].w_axis.truncate();
            let lateral = (p(lu) - p(ru)) + (p(la) - p(ra));
            acc += lateral.cross(Vec3::Y).normalize_or_zero();
        }
        (acc.length() > 1e-4).then(|| acc.x.atan2(acc.z))
    }

    /// The skin's joints at rest: (node index, name, position, parent joint node) in model space.
    pub fn joints_at_rest(&self) -> Vec<(usize, String, Vec3, Option<usize>)> {
        let w = self.pose(None, 0.0);
        let joint_parent = |mut i: usize| loop {
            match self.nodes[i].parent {
                Some(p) if self.joints.contains(&p) => return Some(p),
                Some(p) => i = p,
                None => return None,
            }
        };
        self.joints.iter().map(|&j| (j, self.nodes[j].name.clone(), w[j].w_axis.truncate(), joint_parent(j))).collect()
    }

    /// This (unrigged) model with `template`'s skeleton and clips: the mesh is turned by `turn`
    /// degrees about the up axis, the template's joints are stretched from the template mesh's
    /// bounds over this mesh's bounds (each axis on its own), the `nudges` (joint name, offset in
    /// parts of the height) move single joints, and every vertex is bound to the four bones
    /// nearest to it. Rotations of the template's clips play as they are; their moves (the body
    /// bobbing) scale with the height.
    pub fn fit_rig(&self, template: &Model, turn: f32, nudges: &[(String, [f32; 3])]) -> Result<Model> {
        let tpl = template;
        if tpl.joints.len() < 2 {
            bail!("the rig template has no skeleton");
        }
        // The mesh at rest, turned, in model space.
        let rot = Quat::from_rotation_y(turn.to_radians());
        let mut pts = Vec::new();
        self.skin(&self.pose(None, 0.0), &mut pts);
        for (p, n) in &mut pts {
            (*p, *n) = (rot * *p, rot * *n);
        }
        let bounds = |pts: &[(Vec3, Vec3)]| pts.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), (p, _)| (lo.min(*p), hi.max(*p)));
        // Standing on the ground at its middle, as rigged characters do (models made from
        // pictures have their origin at their centre: half of them would be below the ground).
        let (lo, hi) = bounds(&pts);
        let shift = Vec3::new(-(lo.x + hi.x) / 2.0, -lo.y, -(lo.z + hi.z) / 2.0);
        for (p, _) in &mut pts {
            *p += shift;
        }
        let (mlo, mhi) = bounds(&pts);
        let tw = tpl.pose(None, 0.0);
        let mut tp = Vec::new();
        tpl.skin(&tw, &mut tp);
        let (tlo, thi) = bounds(&tp);
        let scale = (mhi - mlo) / (thi - tlo).max(Vec3::splat(1e-5));
        let height = (mhi.y - mlo.y).max(1e-5);
        let target = |j: usize| -> Vec3 {
            let nudge = nudges.iter().find(|(n, _)| *n == tpl.nodes[j].name).map_or(Vec3::ZERO, |(_, o)| Vec3::from(*o) * height);
            mlo + (tw[j].w_axis.truncate() - tlo) * scale + nudge
        };
        // The template's tree with its joints moved to their targets (rotations kept, so the
        // clips' turns still mean the same).
        let mut nodes = tpl.nodes.clone();
        let mut world = vec![Mat4::IDENTITY; nodes.len()];
        for &i in &tpl.order {
            let parent = nodes[i].parent.map_or(Mat4::IDENTITY, |p| world[p]);
            if tpl.joints.contains(&i) {
                nodes[i].t = parent.inverse().transform_point3(target(i));
            }
            world[i] = parent * Mat4::from_scale_rotation_translation(nodes[i].s, nodes[i].r, nodes[i].t);
        }
        let inverse_bind: Vec<Mat4> = tpl.joints.iter().map(|&j| world[j].inverse()).collect();
        // Bones as segments from a joint to each of its child joints; a last joint (a head, a
        // tail's tip) goes on in its parent bone's direction for half that bone's length.
        let pos = |j: usize| world[j].w_axis.truncate();
        let mut bones: Vec<(usize, Vec3, Vec3)> = Vec::new();
        for (k, &j) in tpl.joints.iter().enumerate() {
            let kids: Vec<usize> = tpl.joints.iter().copied().filter(|&c| nodes[c].parent == Some(j)).collect();
            if kids.is_empty() {
                let from = nodes[j].parent.filter(|p| tpl.joints.contains(p)).map_or(pos(j), pos);
                bones.push((k, pos(j), pos(j) + (pos(j) - from) * 0.5));
            }
            bones.extend(kids.into_iter().map(|c| (k, pos(j), pos(c))));
        }
        let near = 0.02 * height;
        let vertices = self
            .vertices
            .iter()
            .zip(&pts)
            .map(|(v, (p, n))| {
                // The nearest distance to each joint's bones, then the four nearest joints.
                let mut d = vec![f32::MAX; tpl.joints.len()];
                for (k, a, b) in &bones {
                    d[*k] = d[*k].min(segment_distance(*p, *a, *b));
                }
                let mut order: Vec<usize> = (0..d.len()).collect();
                order.sort_by(|a, b| d[*a].total_cmp(&d[*b]));
                let (mut joints, mut weights) = ([0u16; 4], [0f32; 4]);
                for (s, &k) in order.iter().take(4).enumerate() {
                    joints[s] = k as u16;
                    weights[s] = 1.0 / (d[k] + near).powi(4);
                }
                let sum: f32 = weights.iter().sum();
                SkinVertex { pos: *p, normal: *n, uv: v.uv, joints, weights: weights.map(|w| w / sum) }
            })
            .collect();
        // Clips: moves of joints are relative to the template's rest, scaled with the height.
        let k = scale.y;
        let mut clips = tpl.clips.clone();
        for c in &mut clips {
            for t in c.tracks.iter_mut().filter(|t| t.path == Path::Translation && tpl.joints.contains(&t.node)) {
                let (rest, fit) = (tpl.nodes[t.node].t, nodes[t.node].t);
                for v in &mut t.values {
                    let moved = fit + (Vec3::new(v[0], v[1], v[2]) - rest) * k;
                    v[..3].copy_from_slice(&moved.to_array());
                }
            }
        }
        Ok(Model { vertices, indices: self.indices.clone(), nodes, order: tpl.order.clone(), joints: tpl.joints.clone(), inverse_bind, clips, texture: self.texture.clone() })
    }

    /// The rest pose's height (model units) and the lowest point: for framing.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let world = self.pose(None, 0.0);
        let mut out = Vec::new();
        self.skin(&world, &mut out);
        out.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), (p, _)| (lo.min(*p), hi.max(*p)))
    }
}

/// The distance from `p` to the segment `a`-`b`.
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 1e-12 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (p - (a + ab * t)).length()
}

/// Where a held item's model is held and which way its handle runs, guessed from its shape:
/// along its longest side, the hold point is its thinnest part (a sword's grip, an axe's or
/// spear's shaft: there the part nearest an end), the handle runs from there toward the farther
/// end. `indices`: its triangles (widths are measured over them, so long faces without inner
/// points count). Returns (hold point, direction toward the business end).
pub fn estimate_grip(points: &[Vec3], indices: &[u16]) -> ([f32; 3], [f32; 3]) {
    if points.is_empty() {
        return ([0.0; 3], [0.0, 1.0, 0.0]);
    }
    let (lo, hi) = points.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let ext = hi - lo;
    let k = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
    let (a, len) = (lo[k], ext[k].max(1e-6));
    let mid = (lo + hi) / 2.0;
    const BINS: usize = 40;
    let bin = |v: f32| (((v - a) / len * BINS as f32) as usize).min(BINS - 1);
    let across = |p: &Vec3| {
        let mut d = *p - mid;
        d[k] = 0.0;
        d.length()
    };
    // The widest point of anything covering each slice of the length.
    let mut width = [0.0f32; BINS];
    let mut covered = [false; BINS];
    let tris: Vec<[Vec3; 3]> = if indices.len() >= 3 {
        indices.as_chunks::<3>().0.iter().filter_map(|t| Some([*points.get(t[0] as usize)?, *points.get(t[1] as usize)?, *points.get(t[2] as usize)?])).collect()
    } else {
        points.iter().map(|p| [*p; 3]).collect()
    };
    for t in &tris {
        let (b0, b1) = (bin(t.iter().map(|p| p[k]).fold(f32::MAX, f32::min)), bin(t.iter().map(|p| p[k]).fold(f32::MIN, f32::max)));
        let w = t.iter().map(across).fold(0.0f32, f32::max);
        for i in b0..=b1 {
            width[i] = width[i].max(w);
            covered[i] = true;
        }
    }
    // The thinnest slices (within 10 % of the thinnest), not the outermost tips; of those the
    // one nearest an end.
    let inner: Vec<usize> = (1..BINS - 1).filter(|i| covered[*i]).collect();
    let thin = inner.iter().map(|i| width[*i]).fold(f32::MAX, f32::min);
    let near_end = |i: &usize| i.min(&(BINS - 1 - i)).to_owned();
    let g = inner.iter().copied().filter(|i| width[*i] <= thin * 1.1 + 1e-6).min_by_key(near_end).unwrap_or(BINS / 2);
    let at = a + (g as f32 + 0.5) / BINS as f32 * len;
    let mut axis = [0.0f32; 3];
    axis[k] = if g < BINS / 2 { 1.0 } else { -1.0 };
    let mut grip = mid.to_array();
    grip[k] = at;
    (grip, axis)
}

/// Every animation a model can map a clip to, in the editor's order: the four every character
/// uses, furniture, then the activities.
pub const ANIMS: [&str; 13] = ["walk", "idle", "attack", "hurt", "sit", "sleep", "fish", "farm", "craft", "chop", "mine", "sweep", "read"];

/// Clips matched to animations by their names ("Walking_2" -> walk, "Right_Hand_Sword_Slash" ->
/// attack…): a model imported from Meshy, Mixamo or Blender plays something at once. Each
/// animation takes the first clip whose name has one of its words; the creator corrects them.
pub fn guess_clips(names: &[String]) -> Vec<crate::defs::ClipMap> {
    const WORDS: [(&str, &[&str]); 13] = [
        ("walk", &["walk"]),
        ("idle", &["idle", "stand", "breath"]),
        ("attack", &["attack", "slash", "strike", "swing", "punch", "stab", "thrust", "shoot", "spell", "cast", "axe_chop", "smash"]),
        ("hurt", &["hit", "hurt", "damage", "react"]),
        ("sit", &["sit"]),
        ("sleep", &["sleep", "lying", "lie"]),
        ("fish", &["fish"]),
        ("farm", &["farm", "dig", "hoe", "plant", "bend_over", "pick_fruit"]),
        ("craft", &["hammer", "craft", "forge", "work"]),
        ("chop", &["chop", "axe", "wood"]),
        ("mine", &["mine", "mining", "pickaxe"]),
        ("sweep", &["sweep", "broom"]),
        ("read", &["read", "book"]),
    ];
    let mut out = Vec::new();
    for (anim, words) in WORDS {
        // A plain "walk" beats "walk backward"; the shortest matching name wins.
        let best = names.iter().filter(|n| {
            let l = n.to_lowercase();
            words.iter().any(|w| l.contains(w)) && !(anim == "walk" && (l.contains("back") || l.contains("run")))
        });
        if let Some(clip) = best.min_by_key(|n| n.len()) {
            out.push(crate::defs::ClipMap { anim: anim.to_string(), clip: clip.clone(), fps: if anim == "idle" { 8.0 } else { 12.0 } });
        }
    }
    out
}

fn read_nodes(doc: &gltf::Document) -> Vec<Node> {
    let mut nodes: Vec<Node> = doc
        .nodes()
        .map(|n| {
            let (t, r, s) = n.transform().decomposed();
            Node { name: n.name().unwrap_or("").to_string(), parent: None, t: t.into(), r: Quat::from_array(r), s: s.into() }
        })
        .collect();
    for n in doc.nodes() {
        for c in n.children() {
            nodes[c.index()].parent = Some(n.index());
        }
    }
    nodes
}

fn topo_order(nodes: &[Node]) -> Vec<usize> {
    let mut order = Vec::with_capacity(nodes.len());
    let mut done = vec![false; nodes.len()];
    fn visit(i: usize, nodes: &[Node], done: &mut [bool], order: &mut Vec<usize>, depth: usize) {
        if done[i] || depth > nodes.len() {
            return;
        }
        if let Some(p) = nodes[i].parent {
            visit(p, nodes, done, order, depth + 1);
        }
        if !done[i] {
            done[i] = true;
            order.push(i);
        }
    }
    for i in 0..nodes.len() {
        visit(i, nodes, &mut done, &mut order, 0);
    }
    order
}

/// The clips of a document; `map` turns its node indices into the model's (None: skipped).
fn read_clips(doc: &gltf::Document, blob: Option<&[u8]>, map: &dyn Fn(usize) -> Option<usize>) -> Vec<Clip> {
    let buf = |b: gltf::Buffer| match b.source() {
        gltf::buffer::Source::Bin => blob,
        gltf::buffer::Source::Uri(_) => None,
    };
    let mut clips = Vec::new();
    for (k, a) in doc.animations().enumerate() {
        let mut tracks = Vec::new();
        let mut duration: f32 = 0.0;
        for ch in a.channels() {
            let Some(node) = map(ch.target().node().index()) else { continue };
            let r = ch.reader(buf);
            let Some(times) = r.read_inputs().map(|i| i.collect::<Vec<f32>>()) else { continue };
            use gltf::animation::util::ReadOutputs;
            let (path, mut values): (Path, Vec<[f32; 4]>) = match r.read_outputs() {
                Some(ReadOutputs::Translations(v)) => (Path::Translation, v.map(|p| [p[0], p[1], p[2], 0.0]).collect()),
                Some(ReadOutputs::Rotations(v)) => (Path::Rotation, v.into_f32().collect()),
                Some(ReadOutputs::Scales(v)) => (Path::Scale, v.map(|p| [p[0], p[1], p[2], 0.0]).collect()),
                _ => continue,
            };
            let interp = ch.sampler().interpolation();
            if interp == gltf::animation::Interpolation::CubicSpline {
                // In-tangent, value, out-tangent per key: keep the values.
                values = values.chunks(3).filter_map(|c| c.get(1).copied()).collect();
            }
            if times.is_empty() || values.len() < times.len() {
                continue;
            }
            duration = duration.max(*times.last().unwrap());
            tracks.push(Track { node, path, step: interp == gltf::animation::Interpolation::Step, times, values });
        }
        if !tracks.is_empty() {
            clips.push(Clip { name: a.name().map_or_else(|| format!("clip{k}"), str::to_string), duration, tracks });
        }
    }
    clips
}

/// A track's value at `t`.
fn sample(tr: &Track, t: f32) -> [f32; 4] {
    let n = tr.times.len();
    let i = tr.times.partition_point(|&x| x <= t);
    if i == 0 {
        return tr.values[0];
    }
    if i >= n || tr.step {
        return tr.values[(i - 1).min(n - 1)];
    }
    let (t0, t1) = (tr.times[i - 1], tr.times[i]);
    let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
    let (a, b) = (tr.values[i - 1], tr.values[i]);
    match tr.path {
        Path::Rotation => {
            let q = Quat::from_array(a).slerp(Quat::from_array(b), f);
            q.to_array()
        }
        _ => [0, 1, 2, 3].map(|k| a[k] + (b[k] - a[k]) * f),
    }
}

/// A texture image of the file (PNG or JPEG), downsized to [`TEXTURE_MAX`].
fn read_image(img: &gltf::Image, blob: Option<&[u8]>) -> Result<Image> {
    let gltf::image::Source::View { view, mime_type } = img.source() else { bail!("textures in separate files aren't supported") };
    let data = blob.ok_or_else(|| anyhow!("no binary chunk"))?;
    let bytes = data.get(view.offset()..view.offset() + view.length()).ok_or_else(|| anyhow!("texture outside the file"))?;
    let format = if mime_type.contains("jpeg") || mime_type.contains("jpg") { macroquad::prelude::ImageFormat::Jpeg } else { macroquad::prelude::ImageFormat::Png };
    let im = Image::from_file_with_format(bytes, Some(format)).map_err(|e| anyhow!("texture: {e}"))?;
    let mut im = downsize(&im, TEXTURE_MAX);
    quantize(&mut im, TEXTURE_COLORS);
    Ok(im)
}

/// The names of a `.glb`'s animation clips (also of a file with clips only).
pub fn clip_names(glb: &[u8]) -> Result<Vec<String>> {
    let g = gltf::Gltf::from_slice(glb).context("not a glTF binary")?;
    Ok(g.document.animations().enumerate().map(|(k, a)| a.name().map_or_else(|| format!("clip{k}"), str::to_string)).collect())
}

/// A smaller copy of a model file: textures downsized to [`TEXTURE_MAX`] and reduced to
/// [`TEXTURE_COLORS`] colours (as they are when loaded), stored as PNG; without `keep_mesh` (a
/// file used only for its clips) also without meshes, skins and materials. Data nothing uses any
/// more leaves the binary chunk. Meshy writes 2048-pixel textures and repeats the mesh in every
/// animation file: a figure's two files shrink from about 7 MB to well under 1 MB.
pub fn compact(glb: &[u8], keep_mesh: bool) -> Result<Vec<u8>> {
    use gltf::json::validation::USize64;
    use gltf::json::Index;
    let g = gltf::Gltf::from_slice(glb).context("not a glTF binary")?;
    let blob: Vec<u8> = g.blob.clone().unwrap_or_default();
    let mut root = g.document.into_json();
    if !keep_mesh {
        root.meshes.clear();
        root.skins.clear();
        root.materials.clear();
        root.textures.clear();
        root.images.clear();
        root.samplers.clear();
        for n in &mut root.nodes {
            n.mesh = None;
            n.skin = None;
        }
    }
    // Tangents aren't drawn (cel shading has no normal maps).
    use gltf::json::validation::Checked;
    for m in &mut root.meshes {
        for p in &mut m.primitives {
            p.attributes.retain(|k, _| !matches!(k, Checked::Valid(gltf::json::mesh::Semantic::Tangents)));
        }
    }
    // The accessors something still reads.
    let mut used = vec![false; root.accessors.len()];
    let mut mark = |i: Index<gltf::json::Accessor>| {
        if let Some(u) = used.get_mut(i.value()) {
            *u = true;
        }
    };
    for m in &root.meshes {
        for p in &m.primitives {
            p.attributes.values().for_each(|a| mark(*a));
            if let Some(i) = p.indices {
                mark(i);
            }
            for t in p.targets.iter().flatten() {
                [t.positions, t.normals, t.tangents].into_iter().flatten().for_each(&mut mark);
            }
        }
    }
    for sk in &root.skins {
        if let Some(i) = sk.inverse_bind_matrices {
            mark(i);
        }
    }
    for a in &root.animations {
        for s in &a.samplers {
            mark(s.input);
            mark(s.output);
        }
    }
    // Accessors nothing reads go, and the references to the others are renumbered.
    let mut acc_map = vec![u32::MAX; used.len()];
    let mut n = 0u32;
    for (k, u) in used.iter().enumerate() {
        if *u {
            acc_map[k] = n;
            n += 1;
        }
    }
    let re = |i: &mut Index<gltf::json::Accessor>| *i = Index::new(acc_map[i.value()]);
    let mut k = 0;
    root.accessors.retain(|_| {
        k += 1;
        used[k - 1]
    });
    for m in &mut root.meshes {
        for p in &mut m.primitives {
            p.attributes.values_mut().for_each(re);
            if let Some(i) = &mut p.indices {
                re(i);
            }
            for t in p.targets.iter_mut().flatten() {
                [&mut t.positions, &mut t.normals, &mut t.tangents].into_iter().flatten().for_each(re);
            }
        }
    }
    for sk in &mut root.skins {
        if let Some(i) = &mut sk.inverse_bind_matrices {
            re(i);
        }
    }
    for a in &mut root.animations {
        for s in &mut a.samplers {
            re(&mut s.input);
            re(&mut s.output);
        }
    }
    // Textures, downsized.
    let view_bytes = |v: &gltf::json::buffer::View| -> Result<Vec<u8>> {
        let off = v.byte_offset.map_or(0, |o| o.0 as usize);
        blob.get(off..off + v.byte_length.0 as usize).map(<[u8]>::to_vec).ok_or_else(|| anyhow!("data outside the file"))
    };
    let mut new_views: Vec<(Option<usize>, Vec<u8>)> = Vec::new(); // (old view, bytes)
    let mut image_view = Vec::new();
    for im in &root.images {
        let Some(v) = im.buffer_view else {
            image_view.push(None);
            continue;
        };
        let bytes = view_bytes(&root.buffer_views[v.value()])?;
        let small = Image::from_file_with_format(&bytes, None).map_err(|e| anyhow!("texture: {e}")).map(|i| {
            let mut i = downsize(&i, TEXTURE_MAX);
            quantize(&mut i, TEXTURE_COLORS);
            crate::imageio::Rgba { w: i.width as u32, h: i.height as u32, px: i.bytes }.encode()
        });
        let png = small.and_then(|r| r).unwrap_or(bytes);
        image_view.push(Some(new_views.len()));
        new_views.push((None, png));
    }
    // Float skin weights become normalized bytes (the loader sums them to one again).
    let mut weight_views: Vec<(usize, usize)> = Vec::new(); // (accessor, new view)
    let weights: Vec<usize> = root.meshes.iter().flat_map(|m| m.primitives.iter()).filter_map(|p| p.attributes.get(&Checked::Valid(gltf::json::mesh::Semantic::Weights(0))).map(|a| a.value())).collect();
    for a in weights {
        let acc = &root.accessors[a];
        let (Some(v), Checked::Valid(gltf::json::accessor::GenericComponentType(gltf::json::accessor::ComponentType::F32))) = (acc.buffer_view, &acc.component_type) else { continue };
        if acc.sparse.is_some() || weight_views.iter().any(|w| w.0 == a) {
            continue;
        }
        let view = &root.buffer_views[v.value()];
        let data = view_bytes(view)?;
        let (stride, start, count) = (view.byte_stride.map_or(16, |s| s.0), acc.byte_offset.map_or(0, |o| o.0 as usize), acc.count.0 as usize);
        let mut out = Vec::with_capacity(count * 4);
        for k in 0..count {
            let at = start + k * stride;
            let w: Vec<f32> = (0..4).map(|c| data.get(at + c * 4..at + c * 4 + 4).map_or(0.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))).collect();
            out.extend(w.iter().map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
        weight_views.push((a, new_views.len()));
        new_views.push((None, out));
    }
    // The views still read, each once, in their old order.
    let mut keep = vec![false; root.buffer_views.len()];
    for (k, a) in root.accessors.iter().enumerate() {
        if weight_views.iter().any(|w| w.0 == k) {
            continue;
        }
        if let Some(v) = a.buffer_view {
            keep[v.value()] = true;
        }
        if let Some(sp) = &a.sparse {
            keep[sp.indices.buffer_view.value()] = true;
            keep[sp.values.buffer_view.value()] = true;
        }
    }
    let mut remap = vec![usize::MAX; root.buffer_views.len()];
    for (k, _) in keep.iter().enumerate().filter(|(_, k)| **k) {
        remap[k] = new_views.len();
        new_views.push((Some(k), view_bytes(&root.buffer_views[k])?));
    }
    let mut bin = Vec::new();
    let mut views = Vec::new();
    for (old, bytes) in &new_views {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let mut v = match old {
            Some(k) => root.buffer_views[*k].clone(),
            None => gltf::json::buffer::View { buffer: Index::new(0), byte_length: USize64(0), byte_offset: None, byte_stride: None, target: None, extensions: None, extras: Default::default(), name: None },
        };
        v.buffer = Index::new(0);
        v.byte_offset = Some(USize64(bin.len() as u64));
        v.byte_length = USize64(bytes.len() as u64);
        bin.extend_from_slice(bytes);
        views.push(v);
    }
    for (k, a) in root.accessors.iter_mut().enumerate() {
        if let Some(&(_, nv)) = weight_views.iter().find(|w| w.0 == k) {
            a.buffer_view = Some(Index::new(nv as u32));
            a.byte_offset = None;
            a.component_type = Checked::Valid(gltf::json::accessor::GenericComponentType(gltf::json::accessor::ComponentType::U8));
            a.normalized = true;
            (a.min, a.max) = (None, None);
            continue;
        }
        if let Some(v) = a.buffer_view {
            a.buffer_view = Some(Index::new(remap[v.value()] as u32));
        }
        if let Some(sp) = &mut a.sparse {
            sp.indices.buffer_view = Index::new(remap[sp.indices.buffer_view.value()] as u32);
            sp.values.buffer_view = Index::new(remap[sp.values.buffer_view.value()] as u32);
        }
    }
    for (im, v) in root.images.iter_mut().zip(&image_view) {
        if let Some(v) = v {
            im.buffer_view = Some(Index::new(*v as u32));
            im.mime_type = Some(gltf::json::image::MimeType("image/png".into()));
            im.uri = None;
        }
    }
    root.buffer_views = views;
    root.buffers.truncate(1);
    if let Some(b) = root.buffers.first_mut() {
        b.byte_length = USize64(bin.len() as u64);
        b.uri = None;
    }
    // GLB: header, JSON chunk (padded with spaces), BIN chunk (padded with zeros).
    let mut json = root.to_vec().map_err(|e| anyhow!("writing the model: {e}"))?;
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + if bin.is_empty() { 0 } else { 8 + bin.len() };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
    }
    Ok(out)
}

/// Colours a model's texture is reduced to: the pixel look, and colour features can be picked as
/// exact shades.
pub const TEXTURE_COLORS: usize = 32;

/// Reduces an image to at most `n` colours (median cut over the opaque pixels; each pixel takes
/// the average colour of its box).
pub fn quantize(im: &mut Image, n: usize) {
    let px: Vec<[u8; 3]> = im.bytes.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).map(|p| [p[0], p[1], p[2]]).collect();
    if px.is_empty() {
        return;
    }
    let mut boxes: Vec<Vec<[u8; 3]>> = vec![px];
    while boxes.len() < n {
        // Split the box with the widest channel range at its median.
        let (k, ch, range) = boxes
            .iter()
            .enumerate()
            .map(|(k, b)| {
                let r = (0..3).map(|c| (c, b.iter().map(|p| p[c]).max().unwrap() - b.iter().map(|p| p[c]).min().unwrap())).max_by_key(|x| x.1).unwrap();
                (k, r.0, r.1)
            })
            .max_by_key(|x| x.2)
            .unwrap();
        if range == 0 {
            break;
        }
        let mut b = boxes.swap_remove(k);
        b.sort_unstable_by_key(|p| p[ch]);
        let tail = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(tail);
    }
    let palette: Vec<[u8; 3]> = boxes
        .iter()
        .map(|b| {
            let s = b.iter().fold([0u32; 3], |a, p| [a[0] + p[0] as u32, a[1] + p[1] as u32, a[2] + p[2] as u32]);
            let n = b.len() as u32;
            [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8]
        })
        .collect();
    let mut cache = std::collections::HashMap::new();
    for p in im.bytes.as_chunks_mut::<4>().0 {
        if p[3] == 0 {
            continue;
        }
        let c = [p[0], p[1], p[2]];
        let q = *cache.entry(c).or_insert_with(|| {
            *palette.iter().min_by_key(|q| (0..3).map(|i| (q[i] as i32 - c[i] as i32).pow(2)).sum::<i32>()).unwrap()
        });
        p[..3].copy_from_slice(&q);
    }
}

/// Box-filters an image down so its longer side is at most `max`.
pub fn downsize(im: &Image, max: u16) -> Image {
    let (w, h) = (im.width, im.height);
    let k = (w.max(h) as f32 / max as f32).ceil().max(1.0) as u16;
    if k <= 1 {
        return im.clone();
    }
    let (nw, nh) = (w / k, h / k);
    let mut out = Image::gen_image_color(nw, nh, macroquad::color::BLANK);
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0u32; 4];
            for dy in 0..k {
                for dx in 0..k {
                    let i = (((y * k + dy) as usize) * w as usize + (x * k + dx) as usize) * 4;
                    for (c, a) in acc.iter_mut().enumerate() {
                        *a += im.bytes[i + c] as u32;
                    }
                }
            }
            let o = ((y as usize) * nw as usize + x as usize) * 4;
            let n = (k as u32) * (k as u32);
            for (c, a) in acc.iter().enumerate() {
                out.bytes[o + c] = (a / n) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/model_two_bones.glb")).unwrap()
    }

    #[test]
    fn a_two_bone_model_loads_poses_and_skins() {
        let m = Model::load(&fixture()).unwrap();
        assert_eq!((m.vertices.len(), m.indices.len(), m.joints.len()), (3, 3, 2));
        assert_eq!(m.clips.len(), 1);
        let c = m.clip("bend").unwrap();
        assert!((c.duration - 1.0).abs() < 1e-6);
        // Rest pose: the tip vertex (bound to the upper bone) stays at (0, 2, 0).
        let mut out = Vec::new();
        m.skin(&m.pose(None, 0.0), &mut out);
        assert!((out[2].0 - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-4, "{:?}", out[2]);
        // Halfway the upper bone has turned 45 degrees about Z (linear keys 0 -> 90 degrees);
        // the root moved 1 along X, which is removed (walking stays in place).
        m.skin(&m.pose(Some(c), 0.5), &mut out);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((out[2].0 - Vec3::new(-s, 1.0 + s, 0.0)).length() < 1e-3, "{:?}", out[2]);
        // The lower vertices follow the root bone only.
        assert!((out[0].0 - Vec3::new(-0.5, 0.0, 0.0)).length() < 1e-4, "{:?}", out[0]);
        // Looping.
        m.skin(&m.pose(Some(c), 1.5), &mut out);
        assert!((out[2].0 - Vec3::new(-s, 1.0 + s, 0.0)).length() < 1e-3);
        assert_eq!(m.node("upper"), Some(2));
        // No texture: a palette of its material colours (one part, the default white).
        assert_eq!(m.texture.as_ref().map(|t| (t.width, t.height, t.bytes.clone())), Some((1, 1, vec![255, 255, 255, 255])));
    }

    /// An unrigged column of points from y = 0 to 4 (one joint: its own node).
    fn column() -> Model {
        let vertices: Vec<SkinVertex> = (0..=8).map(|i| SkinVertex { pos: Vec3::new(if i % 2 == 0 { 0.5 } else { -0.5 }, i as f32 * 0.5, 0.0), normal: Vec3::Z, uv: Vec2::ZERO, joints: [0; 4], weights: [1.0, 0.0, 0.0, 0.0] }).collect();
        let nodes = vec![Node { name: "Column".into(), parent: None, t: Vec3::ZERO, r: Quat::IDENTITY, s: Vec3::ONE }];
        Model { vertices, indices: vec![0, 1, 2], nodes, order: vec![0], joints: vec![0], inverse_bind: vec![Mat4::IDENTITY], clips: Vec::new(), texture: None }
    }

    #[test]
    fn rig_templates_fit_unrigged_models() {
        let tpl = Model::load(&fixture()).unwrap();
        let m = column();
        let fit = m.fit_rig(&tpl, 0.0, &[]).unwrap();
        assert_eq!(fit.joints, tpl.joints);
        assert_eq!(fit.clips.len(), 1);
        // At rest the mesh stays where it was.
        let mut out = Vec::new();
        fit.skin(&fit.pose(None, 0.0), &mut out);
        for (v, (p, _)) in m.vertices.iter().zip(&out) {
            assert!((v.pos - *p).length() < 1e-3, "{:?} -> {p:?}", v.pos);
        }
        // The template is 2 tall with its upper joint at 1: stretched over 4, the joint is at 2.
        let upper = fit.joints_at_rest().into_iter().find(|j| j.1 == "Upper").unwrap();
        assert!((upper.2.y - 2.0).abs() < 1e-3, "{upper:?}");
        // The top follows the upper bone, the bottom the root.
        let top = fit.vertices.last().unwrap();
        assert_eq!(top.joints[0] as usize, fit.joints.iter().position(|&j| j == upper.0).unwrap());
        assert!(top.weights[0] > 0.9, "{top:?}");
        // Bent by the clip, the top moves; the bottom doesn't.
        fit.skin(&fit.pose(fit.clips.first(), 0.5), &mut out);
        assert!((out[8].0 - Vec3::new(0.5, 4.0, 0.0)).length() > 0.5, "{:?}", out[8]);
        assert!((out[0].0.y).abs() < 0.05, "{:?}", out[0]);
        // A nudge moves the joint (parts of the height), a turn the mesh.
        let nudged = m.fit_rig(&tpl, 0.0, &[("Upper".into(), [0.0, 0.25, 0.0])]).unwrap();
        let upper = nudged.joints_at_rest().into_iter().find(|j| j.1 == "Upper").unwrap();
        assert!((upper.2.y - 3.0).abs() < 1e-3, "{upper:?}");
        let turned = m.fit_rig(&tpl, 90.0, &[]).unwrap();
        assert!(turned.vertices[0].pos.x.abs() < 1e-4 && (turned.vertices[0].pos.z + 0.5).abs() < 1e-4, "{:?}", turned.vertices[0].pos);
        assert!(m.fit_rig(&column(), 0.0, &[]).is_err(), "a template needs a skeleton");
        // A model centred on its middle (as from pictures) stands on the ground once fitted.
        let mut floating = column();
        for v in &mut floating.vertices {
            v.pos += Vec3::new(3.0, -2.0, 1.0);
        }
        let fit = floating.fit_rig(&tpl, 0.0, &[]).unwrap();
        fit.skin(&fit.pose(None, 0.0), &mut out);
        let lo = out.iter().fold(Vec3::splat(f32::MAX), |lo, (p, _)| lo.min(*p));
        let hi = out.iter().fold(Vec3::splat(f32::MIN), |hi, (p, _)| hi.max(*p));
        assert!(lo.y.abs() < 1e-4 && (lo.x + hi.x).abs() < 1e-4 && (lo.z + hi.z).abs() < 1e-4, "{lo:?} .. {hi:?}");
    }

    #[test]
    fn compacted_models_load_the_same() {
        let glb = fixture();
        let small = compact(&glb, true).unwrap();
        let (a, b) = (Model::load(&glb).unwrap(), Model::load(&small).unwrap());
        assert_eq!((a.vertices.len(), a.indices.clone(), a.joints.clone(), a.clips.len()), (b.vertices.len(), b.indices.clone(), b.joints.clone(), b.clips.len()));
        assert_eq!(a.vertices, b.vertices);
        // Clips only: the animation still plays on the full model.
        let clips = compact(&glb, false).unwrap();
        assert!(clips.len() < glb.len(), "{} < {}", clips.len(), glb.len());
        assert_eq!(clip_names(&clips).unwrap(), vec!["bend".to_string()]);
        assert!(Model::load(&clips).is_err(), "a clips-only file is no model");
        let mut c = Model::load(&glb).unwrap();
        c.clips.clear();
        assert_eq!(c.add_clips(&clips).unwrap(), 1);
        let mut out = Vec::new();
        c.skin(&c.pose(c.clip("bend"), 0.5), &mut out);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((out[2].0 - Vec3::new(-s, 1.0 + s, 0.0)).length() < 1e-3, "{:?}", out[2]);
    }

    #[test]
    fn grips_are_estimated_at_the_thinnest_part() {
        // A "sword" along +Y made of flat boxes (corners only): a pommel at 0, a thin grip to
        // 0.2, a wide guard, a blade to 1.
        let quad = |y0: f32, y1: f32, w: f32| [Vec3::new(-w, y0, 0.0), Vec3::new(w, y0, 0.0), Vec3::new(w, y1, 0.0), Vec3::new(-w, y1, 0.0)];
        let parts = [quad(0.0, 0.03, 0.04), quad(0.03, 0.19, 0.015), quad(0.19, 0.23, 0.11), quad(0.23, 1.0, 0.035)];
        let points: Vec<Vec3> = parts.iter().flatten().copied().collect();
        let indices: Vec<u16> = (0..4u16).flat_map(|q| [0, 1, 2, 0, 2, 3].map(|i| q * 4 + i)).collect();
        let (grip, axis) = estimate_grip(&points, &indices);
        assert_eq!(axis, [0.0, 1.0, 0.0]);
        assert!((0.03..0.19).contains(&grip[1]), "{grip:?}");
        // Upside down: the hold point and direction follow.
        let flipped: Vec<Vec3> = points.iter().map(|p| Vec3::new(p.x, 1.0 - p.y, p.z)).collect();
        let (grip, axis) = estimate_grip(&flipped, &indices);
        assert_eq!(axis, [0.0, -1.0, 0.0]);
        assert!((0.81..0.97).contains(&grip[1]), "{grip:?}");
    }

    #[test]
    fn clips_are_guessed_from_their_names() {
        let names: Vec<String> = ["Armature|clip0|baselayer", "Walk_Backward", "walking_2", "Casual_Walk", "Idle_02", "Right_Hand_Sword_Slash", "Hit_Reaction", "Chair_Sit_Idle_Male"].map(String::from).to_vec();
        let g = guess_clips(&names);
        let of = |a: &str| g.iter().find(|c| c.anim == a).map(|c| c.clip.as_str());
        assert_eq!((of("walk"), of("idle"), of("attack"), of("hurt"), of("sit")), (Some("walking_2"), Some("Idle_02"), Some("Right_Hand_Sword_Slash"), Some("Hit_Reaction"), Some("Chair_Sit_Idle_Male")));
        assert_eq!(of("fish"), None);
        // Bows, spells and axes attack too (an axe chop is also the chopping activity).
        let g = guess_clips(&["Walking_2", "Draw_and_Shoot_from_Back"].map(String::from));
        assert_eq!(g.iter().find(|c| c.anim == "attack").map(|c| c.clip.as_str()), Some("Draw_and_Shoot_from_Back"));
        let g = guess_clips(&["Charged_Axe_Chop".to_string()]);
        assert_eq!(g.iter().filter(|c| c.clip == "Charged_Axe_Chop").map(|c| c.anim.as_str()).collect::<Vec<_>>(), vec!["attack", "chop"]);
    }

    #[test]
    fn clips_come_from_other_files_and_textures_downsize() {
        let mut m = Model::load(&fixture()).unwrap();
        // The same file again: its clip replaces the one of the same name.
        assert_eq!(m.add_clips(&fixture()).unwrap(), 1);
        assert_eq!(m.clips.len(), 1);
        let im = Image::gen_image_color(512, 300, macroquad::color::Color::from_rgba(255, 0, 0, 255));
        let d = downsize(&im, 256);
        assert_eq!((d.width, d.height), (256, 150));
        assert_eq!(&d.bytes[..4], &[255, 0, 0, 255]);
        // A gradient of 256 greys reduced to 8 colours, still dark to light.
        let mut g = Image::gen_image_color(256, 1, macroquad::color::BLANK);
        for x in 0..256u32 {
            g.set_pixel(x, 0, macroquad::color::Color::from_rgba(x as u8, x as u8, x as u8, 255));
        }
        quantize(&mut g, 8);
        let colors: std::collections::BTreeSet<u8> = g.bytes.chunks(4).map(|p| p[0]).collect();
        assert_eq!(colors.len(), 8);
        assert!(g.bytes[0] < 32 && g.bytes[255 * 4] > 224);
        assert!(Model::load(b"not a model").is_err());
    }
}
