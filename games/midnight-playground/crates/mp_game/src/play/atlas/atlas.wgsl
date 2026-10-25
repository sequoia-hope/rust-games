// The atlas (play/atlas): the level's map in a UI node. The roads come
// from offset fields (play/atlas/field.rs): each texel holds the offset in
// metres to the nearest road, so one filtered sample gives the distance
// to it at any zoom, and the road is stroked in screen pixels, a dark
// casing under a light core as the minimap strokes its road (hud.wgsl).
// Then the water under them, an event's route over them, the markers and
// the player's arrow, pointing along the car's heading, north up.
// Colours are composited in sRGB and handed to the UI pass in linear, as
// hud.wgsl does.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct AtlasParams {
    head: vec4<f32>,
    view: vec4<f32>,
    size: vec4<f32>,
    net: vec4<f32>,
    net_dim: vec4<f32>,
    route: vec4<f32>,
    route_dim: vec4<f32>,
    water: vec4<f32>,
    water_dim: vec4<f32>,
    player: vec4<f32>,
    route_color: vec4<f32>,
    shapes: array<vec4<f32>, 144>,
};

@group(1) @binding(0)
var<uniform> p: AtlasParams;
@group(1) @binding(1)
var net_tex: texture_2d<f32>;
@group(1) @binding(2)
var samp: sampler;
@group(1) @binding(3)
var route_tex: texture_2d<f32>;
@group(1) @binding(4)
var water_tex: texture_2d<f32>;

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn cov(d: f32, aa: f32) -> f32 {
    return clamp(0.5 - d / aa, 0.0, 1.0);
}

fn over(acc: vec4<f32>, rgb: vec3<f32>, a: f32) -> vec4<f32> {
    return vec4<f32>(rgb * a + acc.rgb * (1.0 - a), a + acc.a * (1.0 - a));
}

fn lin1(v: f32) -> f32 {
    return select(pow((v + 0.055) / 1.055, 2.4), v / 12.92, v <= 0.04045);
}

// hud.wgsl's `lin_alpha` (D823): the alpha that lands a translucent colour
// where the page's sRGB blend would.
fn lin_alpha(l: f32, a: f32) -> f32 {
    if (a <= 0.0 || a >= 1.0) {
        return a;
    }
    let b = 0.25;
    let t = lin1((1.0 - a) * b + a * l);
    let bl = lin1(b);
    let cl = lin1(l);
    if (abs(cl - bl) < 0.0001) {
        return a;
    }
    return clamp((t - bl) / (cl - bl), 0.0, 1.0);
}

fn done(acc: vec4<f32>) -> vec4<f32> {
    if (acc.a <= 0.0001) {
        return vec4<f32>(0.0);
    }
    let c = acc.rgb / acc.a;
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(to_linear(c), lin_alpha(l, acc.a));
}

// A field's texture coordinates for world point w: texel (i, j)'s centre is
// at (x0 + i texel, z0 + j texel). Off the field: negative.
fn field_uv(w: vec2<f32>, f: vec4<f32>, dim: vec4<f32>) -> vec2<f32> {
    let g = (w - f.xy) / f.z + vec2<f32>(0.5);
    let uv = g / dim.xy;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return vec2<f32>(-1.0);
    }
    return uv;
}

// One polygon edge (hud.wgsl's `poly_edge`).
fn poly_edge(q: vec2<f32>, vi: vec2<f32>, vj: vec2<f32>, d: ptr<function, f32>, s: ptr<function, f32>) {
    let e = vj - vi;
    let w = q - vi;
    let b = w - e * clamp(dot(w, e) / dot(e, e), 0.0, 1.0);
    *d = min(*d, dot(b, b));
    let c1 = q.y >= vi.y;
    let c2 = q.y < vj.y;
    let c3 = e.x * w.y > e.y * w.x;
    if ((c1 && c2 && c3) || (!c1 && !c2 && !c3)) {
        *s = -*s;
    }
}

const FAR: f32 = 1e6;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    if (p.head.x < 0.5) {
        return vec4<f32>(0.0);
    }
    let size = p.size.xy;
    let q = in.uv * size;
    let aa = max(fwidth(q.x), 0.0001);
    let mpp = p.view.z;
    let world = p.view.xy + (q - size * 0.5) * mpp;
    var acc = vec4<f32>(0.0);
    // The ground: the minimap's dark glass.
    acc = over(acc, vec3<f32>(10.0, 14.0, 24.0) / 255.0, p.head.z);
    // The water (R) and the built-up land (G): the water's coverage filtered, edged sharp to a pixel once zoomed
    // in past its texels, soft when out.
    if (p.size.z > 0.5) {
        let uv = field_uv(world, p.water, p.water_dim);
        if (uv.x >= 0.0) {
            // Four taps round the point: the cover is a grid of classes
            // looked up nearest, and blurred so the edge at half follows a
            // smooth shore, not its cells' steps.
            let d = vec2<f32>(0.75) / p.water_dim.xy;
            let v = 0.25 * (textureSampleLevel(water_tex, samp, uv + d, 0.0)
                + textureSampleLevel(water_tex, samp, uv - d, 0.0)
                + textureSampleLevel(water_tex, samp, uv + vec2<f32>(d.x, -d.y), 0.0)
                + textureSampleLevel(water_tex, samp, uv + vec2<f32>(-d.x, d.y), 0.0));
            let k = clamp(0.5 * mpp / p.water.z, 0.04, 0.5);
            // Both faded out at the grid's edge, past which nothing is known.
            let fade = smoothstep(0.0, 0.04, min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)));
            // The built-up land a shade lighter, softly.
            acc = over(acc, vec3<f32>(52.0, 58.0, 76.0) / 255.0, 0.35 * v.g * fade);
            let wet = smoothstep(0.5 - k, 0.5 + k, v.r) * fade;
            acc = over(acc, vec3<f32>(22.0, 52.0, 84.0) / 255.0, 0.85 * wet);
        }
    }
    // The roads. Half widths in CSS px: at least a hairline's, their real
    // width when zoomed in that far, never past what the field reaches.
    if (p.head.w > 0.5) {
        let uv = field_uv(world, p.net, p.net_dim);
        if (uv.x >= 0.0) {
            let o = textureSampleLevel(net_tex, samp, uv, 0.0);
            let d_all = length(o.xy) / mpp;
            let d_maj = length(o.zw) / mpp;
            let reach = (p.net.w - 2.0 * p.net.z) / mpp;
            let w_min = min(max(0.9, 7.0 / mpp), reach - 1.5);
            let w_maj = min(max(1.6, 9.4 / mpp), reach - 1.5);
            acc = over(acc, vec3<f32>(0.0), 0.55 * cov(d_all - (w_min + 1.2), aa));
            acc = over(acc, vec3<f32>(0.0), 0.6 * cov(d_maj - (w_maj + 1.4), aa));
            acc = over(acc, vec3<f32>(170.0, 180.0, 205.0) / 255.0, 0.85 * cov(d_all - w_min, aa));
            acc = over(acc, vec3<f32>(236.0, 240.0, 255.0) / 255.0, 0.95 * cov(d_maj - w_maj, aa));
        }
    }
    // An event's route, over the road: a glow and a core in its colour.
    if (p.view.w > 0.5) {
        let uv = field_uv(world, p.route, p.route_dim);
        if (uv.x >= 0.0) {
            let o = textureSampleLevel(route_tex, samp, uv, 0.0);
            let d = length(o.xy) / mpp;
            let reach = (p.route.w - 2.0 * p.route.z) / mpp;
            let w = min(max(2.2, 9.4 / mpp), reach - 4.0);
            let c = p.route_color.rgb;
            acc = over(acc, c, 0.3 * cov(d - (w + 3.5), aa * 4.0));
            acc = over(acc, vec3<f32>(0.0), 0.5 * cov(d - (w + 1.0), aa));
            acc = over(acc, c, cov(d - w, aa));
        }
    }
    // The markers, in order: a filled circle and a stroke on its edge.
    let m = i32(p.head.y);
    for (var i = 0; i < m; i = i + 1) {
        let g = p.shapes[i * 3];
        let f = p.shapes[i * 3 + 1];
        let k = p.shapes[i * 3 + 2];
        let dist = length(q - g.xy);
        if (dist > g.z + g.w + 2.0) {
            continue;
        }
        acc = over(acc, f.rgb, f.a * cov(dist - g.z, aa));
        if (g.w > 0.0) {
            acc = over(acc, k.rgb, k.a * cov(abs(dist - g.z) - g.w * 0.5, aa));
        }
    }
    // The player's arrow (the minimap's, a size up), along the heading.
    if (p.size.w > 0.5) {
        let o = q - p.player.xy;
        if (dot(o, o) < 900.0) {
            let f = p.player.zw;
            let l = vec2<f32>(dot(o, vec2<f32>(-f.y, f.x)), -dot(o, f)) / 1.3;
            var d2 = dot(l - vec2<f32>(0.0, -9.0), l - vec2<f32>(0.0, -9.0));
            var sg = 1.0;
            poly_edge(l, vec2<f32>(0.0, -9.0), vec2<f32>(-6.5, 7.0), &d2, &sg);
            poly_edge(l, vec2<f32>(6.5, 7.0), vec2<f32>(0.0, -9.0), &d2, &sg);
            poly_edge(l, vec2<f32>(0.0, 3.5), vec2<f32>(6.5, 7.0), &d2, &sg);
            poly_edge(l, vec2<f32>(-6.5, 7.0), vec2<f32>(0.0, 3.5), &d2, &sg);
            let sd = sg * sqrt(d2) * 1.3;
            acc = over(acc, vec3<f32>(0.0), 0.45 * cov(sd - 3.0, aa * 3.0));
            acc = over(acc, vec3<f32>(1.0), cov(sd, aa));
            acc = over(acc, vec3<f32>(0.0), cov(abs(sd) - 1.0, aa));
        }
    }
    // The panel's rounded box (in the node's own pixels) and its 1 px
    // edge, as the radio's pill is edged.
    let px = in.uv * in.size;
    let half = in.size * 0.5;
    let r = in.border_radius.x;
    let e = abs(px - half) - half + vec2<f32>(r);
    let sdb = length(max(e, vec2<f32>(0.0))) + min(max(e.x, e.y), 0.0) - r;
    let k = in.size.x / max(size.x, 1.0);
    acc = over(acc, vec3<f32>(1.0), 0.22 * cov(abs(sdb + 0.5 * k) - 0.5 * k, 1.0));
    acc = acc * cov(sdb, 1.0);
    return done(acc);
}
