//! A streamed level's warm-up scene (DECISIONS D1245): one node for every
//! material × mesh-layout × shadow combination any of its sections draws,
//! on geometry of no size and textures of 4 × 4, so the level's warm-up
//! compiles every pipeline the drive will need behind the loading screen.
//! Without it the warm-up covered the base and the start's ring only, and
//! each combination first met on the road compiled as its section came up:
//! a frame of 100 to 300 ms (D1238).
//!
//! The scene is made by the stream's budget test, which builds every
//! section (`stream::budget`), and committed under
//! `assets/warmup/<level>.mrscene`; the test fails when it is out of date
//! and `MP_WARMUP_WRITE=1` writes it again.

use mp_scene::Scene;
#[cfg(test)]
use mp_scene::{
    AttributeRef, Buffer, BufferData, DrawRange, GroupDesc, InstanceDesc, MeshDesc, NodeDesc,
    TextureDesc,
};

static PENINSULA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/warmup/peninsula.mrscene"
));

/// The warm-up scene of level `id`, if it streams and has one.
pub fn scene(id: &str) -> Option<Scene> {
    let bytes = match id {
        "peninsula" => PENINSULA,
        _ => return None,
    };
    match mp_scene::read(bytes) {
        Ok(s) => Some(s),
        Err(e) => {
            bevy::log::warn!("stream: the warm-up scene of {id}: {e}");
            None
        }
    }
}

/// Where level `id`'s warm-up scene is kept, under the game's folder.
#[cfg(test)]
pub fn path(id: &str) -> Option<std::path::PathBuf> {
    (id == "peninsula").then(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/warmup")
            .join(format!("{id}.mrscene"))
    })
}

/// The vertices (and indices) a warm-up mesh has: two triangles, three
/// line segments, a strip or six points, all at the origin.
#[cfg(test)]
const VERTICES: usize = 6;
/// A warm-up texture's side.
#[cfg(test)]
const SIDE: u32 = 4;

/// `n` elements of `item_size` zeros of `like`'s type.
#[cfg(test)]
fn zeros(like: &BufferData, item_size: u32, normalized: bool, n: usize) -> Buffer {
    let len = n * item_size.max(1) as usize;
    let data = match like {
        BufferData::F32(_) => BufferData::F32(vec![0.0; len]),
        BufferData::F64(_) => BufferData::F64(vec![0.0; len]),
        BufferData::U8(_) => BufferData::U8(vec![0; len]),
        BufferData::U16(_) => BufferData::U16(vec![0; len]),
        BufferData::U32(_) => BufferData::U32(vec![0; len]),
        BufferData::I8(_) => BufferData::I8(vec![0; len]),
        BufferData::I16(_) => BufferData::I16(vec![0; len]),
        BufferData::I32(_) => BufferData::I32(vec![0; len]),
    };
    Buffer {
        item_size,
        normalized,
        data,
    }
}

/// Renumbers the textures a material's value names (`{"texture": i}`).
#[cfg(test)]
fn remap_textures(v: &mut serde_json::Value, map: &mut dyn FnMut(u32) -> u32) {
    match v {
        serde_json::Value::Object(o) => {
            if o.len() == 1
                && let Some(serde_json::Value::Number(n)) = o.get("texture")
                && let Some(i) = n.as_u64()
            {
                o.insert("texture".into(), map(i as u32).into());
                return;
            }
            for x in o.values_mut() {
                remap_textures(x, map);
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(|x| remap_textures(x, map)),
        _ => {}
    }
}

/// Copies nodes of `src` into `out` as the warm-up draws them: each a
/// root, visible, at the origin, with its materials (their textures
/// shrunk to [`SIDE`] square) and a mesh of the same attributes with
/// [`VERTICES`] zero vertices, one instance if instanced.
#[cfg(test)]
pub fn extract(src: &Scene, nodes: &[usize], out: &mut Scene) {
    let mut textures: std::collections::BTreeMap<u32, u32> = Default::default();
    let mut materials: std::collections::BTreeMap<u32, u32> = Default::default();
    const IDENTITY: [f64; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    for &i in nodes {
        let n = &src.nodes[i];
        let Some(m) = n.mesh else { continue };
        let desc = &src.meshes[m as usize];
        // The mesh.
        let mut attributes = Vec::new();
        for a in &desc.attributes {
            let count = if a.instanced { 1 } else { VERTICES };
            let b = &src.buffers[a.accessor as usize];
            out.buffers
                .push(zeros(&b.data, b.item_size, b.normalized, count));
            attributes.push(AttributeRef {
                name: a.name.clone(),
                accessor: out.buffers.len() as u32 - 1,
                instanced: a.instanced,
                mesh_per_attribute: a.mesh_per_attribute,
            });
        }
        let index = desc.index.map(|ix| {
            let like = &src.buffers[ix as usize];
            let mut b = zeros(&like.data, like.item_size, like.normalized, VERTICES);
            for k in 0..VERTICES {
                match &mut b.data {
                    BufferData::U16(v) => v[k] = k as u16,
                    BufferData::U32(v) => v[k] = k as u32,
                    BufferData::U8(v) => v[k] = k as u8,
                    _ => {}
                }
            }
            out.buffers.push(b);
            out.buffers.len() as u32 - 1
        });
        out.meshes.push(MeshDesc {
            name: desc.name.clone(),
            attributes,
            index,
            groups: desc
                .groups
                .iter()
                .map(|g| GroupDesc {
                    start: 0,
                    count: VERTICES as f64,
                    material_index: g.material_index,
                })
                .collect(),
            draw_range: DrawRange {
                start: 0,
                count: None,
            },
            bounding_box: None,
            bounding_sphere: None,
        });
        let mesh = out.meshes.len() as u32 - 1;
        // Its materials, and theirs textures.
        let mut mats = Vec::new();
        for &mi in &n.materials {
            if let Some(&k) = materials.get(&mi) {
                mats.push(k);
                continue;
            }
            let mut md = src.materials[mi as usize].clone();
            let mut map = |t: u32| -> u32 {
                *textures.entry(t).or_insert_with(|| {
                    let td = &src.textures[t as usize];
                    let px = &src.buffers[td.pixels as usize];
                    out.buffers.push(zeros(
                        &px.data,
                        td.channels,
                        px.normalized,
                        (SIDE * SIDE) as usize,
                    ));
                    out.textures.push(TextureDesc {
                        width: SIDE,
                        height: SIDE,
                        pixels: out.buffers.len() as u32 - 1,
                        url: None,
                        ..td.clone()
                    });
                    out.textures.len() as u32 - 1
                })
            };
            for v in md.params.values_mut() {
                remap_textures(v, &mut map);
            }
            if let Some(u) = md.uniforms.as_mut() {
                for v in u.values_mut() {
                    remap_textures(v, &mut map);
                }
            }
            out.materials.push(md);
            let k = out.materials.len() as u32 - 1;
            materials.insert(mi, k);
            mats.push(k);
        }
        let node = out.nodes.len() as u32;
        let instances = n.instances.map(|k| {
            let inst = &src.instances[k as usize];
            out.buffers.push(Buffer {
                item_size: 16,
                normalized: false,
                data: BufferData::F32(IDENTITY.iter().map(|&x| x as f32).collect()),
            });
            let matrices = out.buffers.len() as u32 - 1;
            let colors = inst.colors.map(|_| {
                out.buffers.push(Buffer {
                    item_size: 3,
                    normalized: false,
                    data: BufferData::F32(vec![1.0; 3]),
                });
                out.buffers.len() as u32 - 1
            });
            out.instances.push(InstanceDesc {
                node,
                count: 1,
                capacity: 1,
                matrices,
                colors,
                bounding_sphere: None,
            });
            out.instances.len() as u32 - 1
        });
        out.nodes.push(NodeDesc {
            name: n.name.clone(),
            ty: n.ty,
            parent: None,
            children: Vec::new(),
            matrix: IDENTITY,
            matrix_world: IDENTITY,
            visible: true,
            matrix_auto_update: n.matrix_auto_update,
            frustum_culled: false,
            render_order: n.render_order,
            cast_shadow: n.cast_shadow,
            receive_shadow: n.receive_shadow,
            layers: n.layers,
            user_data: Default::default(),
            mesh: Some(mesh),
            materials: mats,
            multi_material: n.multi_material,
            instances,
            center: n.center,
            light: None,
        });
        out.roots.push(node);
    }
}
