//! Pipeline warm-up (Midnight Playground SPEC 6.3, roadmap WP 2.6, DECISIONS
//! D390, D459, D1005; its `warmup.rs`). On the web, Bevy creates a render
//! pipeline synchronously the first time a material and mesh layout is
//! drawn, which costs tens to hundreds of milliseconds a pipeline: a hitch
//! whenever the camera first sees a combination. So while the loading
//! screen is still up, the client draws one degenerate triangle with every
//! material × mesh-layout combination the scene uses, off screen, in the
//! main pass and (for shadow casters) the shadow pass, until every pipeline
//! has compiled; then the stand-ins are despawned and the scene is shown.
//!
//! Combinations are the pipeline's inputs: the material's key (whatever
//! picks its pipeline: shader defs, blending, culling, depth state; the
//! game's type `K`), the mesh's vertex attributes and topology, and
//! whether it casts a shadow. Materials with the same key share one
//! pipeline, so one stand-in covers them all. A material drawn instanced
//! says so when noted, and its stand-in gets whatever the game's `extra`
//! closure adds (an instance stream of one instance, drawn as the real ones
//! are).

use crate::status::LoadStatus;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::system::EntityCommands;
use bevy::light::NotShadowCaster;
use bevy::mesh::{
    Mesh, MeshVertexAttribute, MeshVertexAttributeId, PrimitiveTopology, VertexAttributeValues,
};
use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::VertexFormat;
use std::collections::HashMap;
use std::hash::Hash;

/// Where the stand-ins sit: 10,000 km below the origin, past the far plane
/// and the shadow box whatever the camera does.
pub const FAR_BELOW: f32 = -1.0e7;

/// A mesh's vertex layout, as far as it picks a pipeline.
#[derive(Clone, Debug)]
pub struct Layout {
    pub topology: PrimitiveTopology,
    pub attributes: Vec<MeshVertexAttribute>,
}

impl Layout {
    pub fn of(mesh: &Mesh) -> Layout {
        Layout {
            topology: mesh.primitive_topology(),
            attributes: mesh.attributes().map(|(a, _)| *a).collect(),
        }
    }

    /// What tells one pipeline's vertex input from another's.
    pub fn signature(&self) -> Signature {
        let mut ids: Vec<(MeshVertexAttributeId, VertexFormat)> =
            self.attributes.iter().map(|a| (a.id, a.format)).collect();
        ids.sort_by_key(|x| x.0);
        (self.topology, ids)
    }

    /// Six vertices of zeros with the same attributes: two triangles (or
    /// three lines) of no size, which rasterise nothing.
    pub fn degenerate(&self) -> Mesh {
        let mut mesh = Mesh::new(self.topology, RenderAssetUsages::RENDER_WORLD);
        for a in &self.attributes {
            if let Some(v) = zeros(a.format, 6) {
                mesh.insert_attribute(*a, v);
            }
        }
        mesh
    }
}

fn zeros(format: VertexFormat, n: usize) -> Option<VertexAttributeValues> {
    Some(match format {
        VertexFormat::Float32 => VertexAttributeValues::Float32(vec![0.0; n]),
        VertexFormat::Float32x2 => VertexAttributeValues::Float32x2(vec![[0.0; 2]; n]),
        VertexFormat::Float32x3 => VertexAttributeValues::Float32x3(vec![[0.0; 3]; n]),
        VertexFormat::Float32x4 => VertexAttributeValues::Float32x4(vec![[0.0; 4]; n]),
        VertexFormat::Uint32 => VertexAttributeValues::Uint32(vec![0; n]),
        VertexFormat::Uint16x4 => VertexAttributeValues::Uint16x4(vec![[0; 4]; n]),
        VertexFormat::Unorm8x4 => VertexAttributeValues::Unorm8x4(vec![[0; 4]; n]),
        _ => return None,
    })
}

pub type Signature = (
    PrimitiveTopology,
    Vec<(MeshVertexAttributeId, VertexFormat)>,
);

/// The combinations a scene draws, gathered while it is built: material
/// key × vertex layout × casts a shadow.
pub struct Combos<M: Material, K> {
    seen: HashMap<(K, Signature, bool), usize>,
    list: Vec<(Handle<M>, Layout, bool, bool)>,
}

impl<M: Material, K> Default for Combos<M, K> {
    fn default() -> Self {
        Combos {
            seen: HashMap::new(),
            list: Vec::new(),
        }
    }
}

impl<M: Material, K: Hash + Eq + Clone> Combos<M, K> {
    /// Notes that `material` (whose pipeline key is `key`, drawn instanced
    /// or not) is drawn on a mesh of `layout`, casting a shadow or not.
    pub fn note(
        &mut self,
        key: K,
        material: &Handle<M>,
        layout: &Layout,
        casts: bool,
        instanced: bool,
    ) {
        let k = (key, layout.signature(), casts);
        if self.seen.contains_key(&k) {
            return;
        }
        self.seen.insert(k, self.list.len());
        self.list
            .push((material.clone(), layout.clone(), casts, instanced));
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Spawns one stand-in per combination. They sit far outside every
    /// view ([`FAR_BELOW`]) and are never culled, so each is specialised
    /// for the main view and the shadow view. `extra` adds the game's own
    /// components to each (its scene marker; for an instanced one, the
    /// stream of one instance).
    pub fn spawn(
        &self,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        extra: &mut dyn FnMut(&mut EntityCommands, bool),
    ) -> usize {
        self.spawn_each(commands, meshes, extra).len()
    }

    /// [`Combos::spawn`], returning the stand-ins.
    pub fn spawn_each(
        &self,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        extra: &mut dyn FnMut(&mut EntityCommands, bool),
    ) -> Vec<Entity> {
        let mut out = Vec::with_capacity(self.list.len());
        let mut cache: HashMap<Signature, Handle<Mesh>> = HashMap::new();
        for (material, layout, casts, instanced) in &self.list {
            let mesh = cache
                .entry(layout.signature())
                .or_insert_with(|| meshes.add(layout.degenerate()))
                .clone();
            let mut e = commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(0.0, FAR_BELOW, 0.0),
                NoFrustumCulling,
                WarmUp,
                Name::new("warm-up"),
            ));
            if !casts {
                e.insert(NotShadowCaster);
            }
            extra(&mut e, *instanced);
            out.push(e.id());
        }
        out
    }
}

/// A warm-up stand-in.
#[derive(Component)]
pub struct WarmUp;

/// Lays out every printable ASCII character in each of the given text
/// sizes (bold), hidden, during the warm-up (D459), so the glyph atlas
/// holds them before play starts. Bevy adds a glyph to its atlas the first
/// time a text shows it and then uploads the whole 512 × 512 atlas again:
/// a timer's new digits in the first half second meant one or two
/// such uploads a frame, which in Chrome's WebGPU took 30 to 80 ms frames.
/// The sizes are the HUD's; a HUD that shows more than ASCII adds its own.
pub fn prewarm_glyphs(
    commands: &mut Commands,
    sizes: &[f32],
    extra: &mut dyn FnMut(&mut EntityCommands),
) {
    let chars: String = (0x20u8..0x7f).map(char::from).collect();
    for &size in sizes {
        let mut e = commands.spawn((
            Text::new(chars.clone()),
            TextFont::from_font_size(size).with_font_weight(FontWeight::BOLD),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-100_000.0),
                ..default()
            },
            Visibility::Hidden,
            WarmUp,
            Name::new("warm-up glyphs"),
        ));
        extra(&mut e);
    }
}

/// A UI node, off screen, during the warm-up: a background colour, a
/// gradient and a box shadow, the three Bevy UI pipelines menu screens draw
/// with. Bevy compiles each the first time one is drawn, so a menu's first
/// frames queued them after the warm-up (D1005): the box shadow natively,
/// all three on the web, whose loading screen is the page's and draws no
/// Bevy UI. Bevy draws a node wherever it is; the keys are the window's
/// format, the shadow samples and the gradient's colour space (sRGB), the
/// same for every node. Spawned with the first scene's warm-up; the
/// pipelines stay compiled after that.
pub fn prewarm_ui(commands: &mut Commands, extra: &mut dyn FnMut(&mut EntityCommands)) {
    let mut e = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(-100_000.0),
            width: Val::Px(4.0),
            height: Val::Px(4.0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        BackgroundGradient::from(
            LinearGradient::to_right(vec![
                ColorStop::auto(Color::BLACK),
                ColorStop::auto(Color::WHITE),
            ])
            .in_srgb(),
        ),
        BoxShadow(vec![ShadowStyle {
            color: Color::BLACK,
            x_offset: Val::Px(0.0),
            y_offset: Val::Px(0.0),
            spread_radius: Val::Px(0.0),
            blur_radius: Val::Px(2.0),
        }]),
        WarmUp,
        Name::new("warm-up menu node"),
    ));
    extra(&mut e);
}

/// Once every pipeline has compiled (`LoadStatus::ready`), the stand-ins go.
pub fn end_warm_up(
    mut commands: Commands,
    status: Res<LoadStatus>,
    q: Query<Entity, With<WarmUp>>,
) {
    if !status.ready || q.is_empty() {
        return;
    }
    let mut n = 0;
    for e in &q {
        commands.entity(e).despawn();
        n += 1;
    }
    info!("warm-up done: {n} stand-ins removed");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(colors: bool) -> Layout {
        let mut m = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
        if colors {
            m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; 3]);
        }
        Layout::of(&m)
    }

    #[test]
    fn one_stand_in_per_pipeline() {
        let mut c: Combos<StandardMaterial, (u8, bool)> = Combos::default();
        let a = Handle::<StandardMaterial>::default();
        let key = (0, false);
        let fog = (1, false);
        c.note(key, &a, &layout(false), true, false);
        // Another material with the same key on the same layout: the same
        // pipeline.
        c.note(key, &a, &layout(false), true, false);
        assert_eq!(c.len(), 1);
        c.note(key, &a, &layout(true), true, false);
        c.note(fog, &a, &layout(false), true, false);
        c.note(key, &a, &layout(false), false, false);
        assert_eq!(c.len(), 4);
        assert!(!c.is_empty());
    }

    #[test]
    fn degenerate_mesh_keeps_the_layout() {
        let l = layout(true);
        let m = l.degenerate();
        assert_eq!(m.count_vertices(), 6);
        assert_eq!(Layout::of(&m).signature(), l.signature());
    }
}
