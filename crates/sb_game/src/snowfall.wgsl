// Falling snow (`fx.rs`, `SnowfallMaterial`): a mesh written once, every
// flake moved here from the time and the camera Bevy already gives every
// shader, so nothing is uploaded per frame. A flake falls and sways from
// its place at time 0, wraps round a box about the eye (so the snow is
// world-fixed as the camera rides through it), faces the camera, and fades
// as it nears the lens.

#import bevy_pbr::mesh_view_bindings::{view, globals}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var puff_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var puff_sampler: sampler;

struct Vertex {
    // The flake's place at time 0, anywhere: the wrap brings it to the box.
    @location(0) position: vec3<f32>,
    // The quad's corner, -1 or 1 each way.
    @location(2) corner: vec2<f32>,
    // The sway's phase and the fall's speed, m/s.
    @location(3) motion: vec2<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

// The box round the eye, as `fx::BOX_LO` and `fx::BOX_SIZE`.
const LO: vec3<f32> = vec3<f32>(-30.0, -12.0, -30.0);
const SIZE: vec3<f32> = vec3<f32>(60.0, 30.0, 60.0);
const FLAKE: f32 = 0.06;

@vertex
fn vertex(v: Vertex) -> VOut {
    let t = globals.time;
    let eye = view.world_position;
    var p = v.position;
    p.y -= v.motion.y * t;
    p.x -= cos(t * 0.8 + v.motion.x) * 0.75;
    let d = p - eye - LO;
    let off = d - SIZE * floor(d / SIZE) + LO;
    // Fade out what is about to hit the lens; a flake gone is a point.
    let alpha = 0.9 * clamp((length(off) - 0.8) / 2.5, 0.0, 1.0);
    let size = select(0.0, FLAKE, alpha > 0.01);
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    let world = eye + off + (right * v.corner.x + up * v.corner.y) * size;
    var out: VOut;
    out.clip = view.clip_from_world * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>((v.corner.x + 1.0) * 0.5, (1.0 - v.corner.y) * 0.5);
    out.alpha = alpha;
    return out;
}

@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    let a = textureSample(puff_texture, puff_sampler, in.uv).a * in.alpha;
    return vec4<f32>(1.0, 1.0, 1.0, a);
}
