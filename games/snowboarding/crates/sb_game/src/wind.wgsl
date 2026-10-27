// High-speed air streaks (`fx.rs`, `WindMaterial`): a peripheral tunnel of
// thin wind-sheared ice streaks written once at startup and driven entirely
// through the entity's transform, so zero vertex or uniform buffers are
// re-created per frame.
//
// `world_from_local` carries:
// - `translation`: the tunnel's origin at the camera,
// - `rotation`: local -Z aligned with the airflow direction,
// - `scale`: `(1 + intensity, 1 + phase, 1 + speed)`.

#import bevy_pbr::{mesh_functions, mesh_view_bindings::view}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var puff_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var puff_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // (angle around tunnel, radial ring 0..1, depth phase 0..1).
    @location(0) position: vec3<f32>,
    // Quad corner in [-1, 1]^2: x across width, y along streak length.
    @location(2) corner: vec2<f32>,
    // Per-streak (speed multiplier, width multiplier).
    @location(3) motion: vec2<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vertex(v: Vertex) -> VOut {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    let sx = max(length(world_from_local[0].xyz), 1e-4);
    let sy = max(length(world_from_local[1].xyz), 1e-4);
    let sz = max(length(world_from_local[2].xyz), 1e-4);

    let intensity = clamp(sx - 1.0, 0.0, 1.0);
    let phase = sy - 1.0;
    let speed = max(sz - 1.0, 0.0);

    let right = world_from_local[0].xyz / sx;
    let up = world_from_local[1].xyz / sy;
    let back = world_from_local[2].xyz / sz;
    let origin = world_from_local[3].xyz;

    let z_frac = fract(v.position.z - phase * v.motion.x);
    let z_ahead = 1.0 + z_frac * 18.5;
    let radius = mix(1.85, 5.2, v.position.y) * (0.72 + 0.42 * z_frac);
    let radial = right * (cos(v.position.x) * 1.22) + up * (sin(v.position.x) * 0.82);
    let center = origin - back * z_ahead + radial * radius;

    let to_eye = view.world_position - center;
    let c = cross(back, to_eye);
    let cl = length(c);
    let side = select(right, c / max(cl, 1e-4), cl > 1e-4);

    let half_len = (0.35 + speed * 0.048) * v.motion.x * (0.45 + 0.55 * intensity);
    let half_wid = (0.013 + 0.006 * intensity) * v.motion.y;

    let fade = smoothstep(0.0, 0.16, z_frac) * smoothstep(1.0, 0.72, z_frac);
    let alpha = intensity * fade * 0.34;
    let show = select(0.0, 1.0, alpha > 0.004);
    let world = center + (side * (v.corner.x * half_wid) + back * (v.corner.y * half_len)) * show;

    var out: VOut;
    out.clip = view.clip_from_world * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>((v.corner.x + 1.0) * 0.5, (1.0 - v.corner.y) * 0.5);
    out.alpha = alpha;
    return out;
}

@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    let a = textureSample(puff_texture, puff_sampler, in.uv).a * in.alpha;
    return vec4<f32>(0.92, 0.97, 1.0, a);
}
