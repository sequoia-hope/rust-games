//! The atlas's UI material (`atlas.wgsl`): the level's fields as textures
//! (the network's offsets, an event's route, the water) and the view and
//! the markers as uniforms.
//!
//! The view changes every frame the atlas is open (it follows the car), so
//! the uniforms are written into one buffer in place ([`AtlasWrites`]), as
//! the HUD's are (`hud/material.rs`, D865), rather than by editing the
//! asset, which would make a new buffer and bind group each frame. The
//! asset changes only when a field's texture does.

use bevy::ecs::system::SystemParamItem;
use bevy::ecs::system::lifetimeless::SRes;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    AsBindGroup, AsBindGroupError, BindGroupLayout, BindGroupLayoutEntry, BindingResources, Buffer,
    BufferInitDescriptor, BufferUsages, OwnedBindingResource, SamplerBindingType, ShaderType,
    TextureViewDimension, UnpreparedBindGroup, encase,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::{MainWorld, Render, RenderApp, RenderSystems};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::UiMaterial;
use std::sync::Mutex;

/// The most markers (dots) the shader takes.
pub const MAX_SHAPES: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct AtlasParams {
    /// x: drawn (0: the 1 px node that keeps the pipeline warm), y: the
    /// markers, z: the panel's alpha, w: whether the network's field is in.
    pub head: Vec4,
    /// The view: centre x, z, metres per CSS px; w: the route's field is in.
    pub view: Vec4,
    /// The panel, CSS px: w, h; z: the water is in; w: the player shows.
    pub size: Vec4,
    /// The network's field: x0, z0, texel, reach; and its w, h.
    pub net: Vec4,
    pub net_dim: Vec4,
    /// The route's field likewise.
    pub route: Vec4,
    pub route_dim: Vec4,
    /// The water: x0, z0, texel; and its w, h.
    pub water: Vec4,
    pub water_dim: Vec4,
    /// The player: panel x, y; heading as a panel direction.
    pub player: Vec4,
    /// The route's colour (sRGB).
    pub route_color: Vec4,
    /// Per marker: (x, y, r, stroke width), fill, stroke.
    pub shapes: [Vec4; MAX_SHAPES * 3],
}

impl Default for AtlasParams {
    fn default() -> AtlasParams {
        AtlasParams {
            head: Vec4::ZERO,
            view: Vec4::new(0.0, 0.0, 1.0, 0.0),
            size: Vec4::new(1.0, 1.0, 0.0, 0.0),
            net: Vec4::new(0.0, 0.0, 1.0, 1.0),
            net_dim: Vec4::ONE,
            route: Vec4::new(0.0, 0.0, 1.0, 1.0),
            route_dim: Vec4::ONE,
            water: Vec4::new(0.0, 0.0, 1.0, 0.0),
            water_dim: Vec4::ONE,
            player: Vec4::ZERO,
            route_color: Vec4::ONE,
            shapes: [Vec4::ZERO; MAX_SHAPES * 3],
        }
    }
}

/// The atlas's material: its first uniforms (later ones go through
/// [`AtlasWrites`]) and its three textures.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct AtlasMaterial {
    pub p: AtlasParams,
    pub net: Handle<Image>,
    pub route: Handle<Image>,
    pub water: Handle<Image>,
}

/// The layout the derive gives these bindings, which [`AtlasMaterial`]
/// keeps.
#[derive(AsBindGroup)]
struct AtlasLayout {
    #[uniform(0)]
    _p: AtlasParams,
    #[texture(1)]
    #[sampler(2)]
    _net: Handle<Image>,
    #[texture(3)]
    _route: Handle<Image>,
    #[texture(4)]
    _water: Handle<Image>,
}

fn bytes(p: &AtlasParams) -> Vec<u8> {
    let mut b = encase::UniformBuffer::new(Vec::new());
    b.write(p).expect("atlas uniforms");
    b.into_inner()
}

/// Render world: the uniform buffer, made when the material is first
/// prepared.
#[derive(Resource, Default)]
pub struct AtlasBuffer(Mutex<Option<Buffer>>);

impl AsBindGroup for AtlasMaterial {
    type Data = ();
    type Param = (SRes<AtlasBuffer>, SRes<RenderAssets<GpuImage>>);

    fn label() -> &'static str {
        "atlas_material"
    }

    fn bind_group_data(&self) -> Self::Data {}

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        render_device: &RenderDevice,
        (buffer, images): &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        let image = |h: &Handle<Image>| images.get(h).ok_or(AsBindGroupError::RetryNextUpdate);
        let (net, route, water) = (image(&self.net)?, image(&self.route)?, image(&self.water)?);
        let mut b = buffer.0.lock().unwrap_or_else(|e| e.into_inner());
        let buffer = b
            .get_or_insert_with(|| {
                render_device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("atlas_material"),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    contents: &bytes(&self.p),
                })
            })
            .clone();
        let view = |g: &GpuImage| {
            OwnedBindingResource::TextureView(TextureViewDimension::D2, g.texture_view.clone())
        };
        Ok(UnpreparedBindGroup {
            bindings: BindingResources(vec![
                (0, OwnedBindingResource::Buffer(buffer)),
                (1, view(net)),
                (
                    2,
                    OwnedBindingResource::Sampler(
                        SamplerBindingType::Filtering,
                        net.sampler.clone(),
                    ),
                ),
                (3, view(route)),
                (4, view(water)),
            ]),
        })
    }

    fn bind_group_layout_entries(
        render_device: &RenderDevice,
        force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry> {
        AtlasLayout::bind_group_layout_entries(render_device, force_no_bindless)
    }
}

impl UiMaterial for AtlasMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://mp_game/play/atlas/atlas.wgsl".into()
    }
}

/// Main world: the uniforms for this frame, if they changed.
#[derive(Resource, Default)]
pub struct AtlasWrites(pub Option<AtlasParams>);

/// Render world: a write waiting for its buffer.
#[derive(Resource, Default)]
struct Pending(Option<AtlasParams>);

fn extract_writes(mut main: ResMut<MainWorld>, mut pending: ResMut<Pending>) {
    let Some(mut w) = main.get_resource_mut::<AtlasWrites>() else {
        return;
    };
    if let Some(p) = w.0.take() {
        pending.0 = Some(p);
    }
}

fn write_uniforms(buffer: Res<AtlasBuffer>, mut pending: ResMut<Pending>, queue: Res<RenderQueue>) {
    if pending.0.is_none() {
        return;
    }
    let b = buffer.0.lock().unwrap_or_else(|e| e.into_inner());
    if let (Some(buf), Some(p)) = (b.as_ref(), pending.0.as_ref()) {
        queue.write_buffer(buf, 0, &bytes(p));
        pending.0 = None;
    }
}

pub fn plugin(app: &mut App) {
    bevy::asset::embedded_asset!(app, "atlas.wgsl");
    app.init_resource::<AtlasWrites>()
        .add_plugins(bevy::ui_render::UiMaterialPlugin::<AtlasMaterial>::default());
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .init_resource::<AtlasBuffer>()
            .init_resource::<Pending>()
            .add_systems(bevy::render::ExtractSchedule, extract_writes)
            .add_systems(
                Render,
                write_uniforms.in_set(RenderSystems::PrepareResources),
            );
    }
}

/// A marker, panel CSS px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub fill: [f32; 4],
    pub stroke: Option<(f64, [f32; 4])>,
}

/// Puts the markers into the uniforms, the first [`MAX_SHAPES`].
pub fn set_shapes(p: &mut AtlasParams, dots: &[Dot]) {
    let n = dots.len().min(MAX_SHAPES);
    for (i, d) in dots.iter().take(n).enumerate() {
        let (sw, sc) = d.stroke.unwrap_or((0.0, [0.0; 4]));
        p.shapes[i * 3] = Vec4::new(d.x as f32, d.y as f32, d.r as f32, sw as f32);
        p.shapes[i * 3 + 1] = Vec4::from_array(d.fill);
        p.shapes[i * 3 + 2] = Vec4::from_array(sc);
    }
    p.head.y = n as f32;
}
