// Screen-space tapered segments, with a black border and one-pixel analytic AA.

struct FrameUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    eye: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    up: vec4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    viewport: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct VertexInput {
    @location(0) start: vec3<f32>,
    @location(1) start_width: f32,
    @location(2) end: vec3<f32>,
    @location(3) end_width: f32,
    @location(4) start_color: vec4<f32>,
    @location(5) end_color: vec4<f32>,
    @location(6) outline: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) start_color: vec4<f32>,
    @location(1) @interpolate(linear) local: vec2<f32>,
    // Projected length, start/end half-width, outline (all physical pixels).
    @location(2) @interpolate(flat) shape: vec4<f32>,
    @location(3) @interpolate(flat) end_color: vec4<f32>,
    @location(4) @interpolate(flat) inverse_w: vec2<f32>,
    @location(5) @interpolate(flat) depth: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput, @builtin(vertex_index) vertex: u32) -> VertexOutput {
    var out: VertexOutput;
    // Rejected segments produce a degenerate triangle outside the clip volume.
    out.clip_position = vec4<f32>(0.0, 0.0, -1.0, 1.0);
    out.start_color = vec4<f32>(0.0);
    out.local = vec2<f32>(0.0);
    out.shape = vec4<f32>(0.0);
    out.end_color = vec4<f32>(0.0);
    out.inverse_w = vec2<f32>(1.0);
    out.depth = vec2<f32>(0.0);

    let original_a = frame.view_proj * vec4<f32>(input.start, 1.0);
    let original_b = frame.view_proj * vec4<f32>(input.end, 1.0);
    // Clip the centreline BEFORE dividing by w. Clipping the expanded quad alone
    // lets an endpoint behind the eye reverse the direction or fill the viewport.
    let planes = array<vec2<f32>, 2>(
        vec2<f32>(original_a.z, original_b.z),
        vec2<f32>(original_a.w - original_a.z, original_b.w - original_b.z)
    );
    var interval = vec2<f32>(0.0, 1.0);
    for (var i = 0u; i < 2u; i += 1u) {
        let d = planes[i];
        if d.x < 0.0 && d.y < 0.0 { return out; }
        if d.x < 0.0 { interval.x = max(interval.x, d.x / (d.x - d.y)); }
        if d.y < 0.0 { interval.y = min(interval.y, d.x / (d.x - d.y)); }
    }
    if interval.x >= interval.y { return out; }
    var a = mix(original_a, original_b, interval.x);
    var b = mix(original_a, original_b, interval.y);
    if a.w <= 0.0 || b.w <= 0.0 { return out; }
    a.z = clamp(a.z, 0.0, a.w);
    b.z = clamp(b.z, 0.0, b.w);
    let delta = (b.xy / b.w - a.xy / a.w) * frame.viewport.xy * 0.5;
    let length = length(delta);
    // A zero projected length is not a point sprite.
    if length < 0.0001 { return out; }
    let direction = delta / length;
    let normal = vec2<f32>(-direction.y, direction.x);
    // Widths are screen-linear, unlike perspective-correct vertex colours.
    // With both original endpoints in front of the eye, convert the clip
    // parameters to projected fractions so near/far clipping preserves taper.
    // A segment crossing the eye has no finite original screen-space span;
    // interpolate its endpoint widths on the world segment before projection.
    var width_interval = interval;
    if original_a.w > 0.0 && original_b.w > 0.0 {
        width_interval = interval * original_b.w / vec2<f32>(a.w, b.w);
    }
    let widths = vec2<f32>(
        mix(input.start_width, input.end_width, width_interval.x),
        mix(input.start_width, input.end_width, width_interval.y)
    ) * 0.5;
    if max(widths.x, widths.y) + input.outline <= 0.0 { return out; }

    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0)
    );
    let corner = corners[vertex];
    let pad = input.outline + 0.5;
    let slope = (widths.y - widths.x) / length;
    // Nonzero coverage requires x in (-pad, length + pad) and
    // abs(y) < widths.x + slope*x + pad*sqrt(1 + slope*slope).
    // Maximizing over the padded x interval includes both the perpendicular
    // edge offset and the taper's extra width beyond the wider endpoint.
    let across = max(widths.x, widths.y)
        + pad * (sqrt(1.0 + slope * slope) + abs(slope));
    let along = mix(-pad, length + pad, corner.x);
    let clip = mix(a, b, corner.x);
    let offset = direction * mix(-pad, pad, corner.x) + normal * corner.y * across;
    out.clip_position = vec4<f32>(clip.xy + offset * 2.0 / frame.viewport.xy * clip.w, clip.zw);
    out.local = vec2<f32>(along, corner.y * across);
    out.shape = vec4<f32>(length, widths, input.outline);
    out.start_color = mix(input.start_color, input.end_color, interval.x);
    out.end_color = mix(input.start_color, input.end_color, interval.y);
    out.inverse_w = 1.0 / vec2<f32>(a.w, b.w);
    out.depth = vec2<f32>(a.z / a.w, b.z / b.w);
    return out;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    let slope = (input.shape.z - input.shape.y) / input.shape.x;
    let half_width = input.shape.y + input.local.x * slope;
    let side_distance = (abs(input.local.y) - half_width) / sqrt(1.0 + slope * slope);
    let distance = max(side_distance, max(-input.local.x, input.local.x - input.shape.x));
    let outer = clamp(0.5 - distance + input.shape.w, 0.0, 1.0);
    let inner = clamp(0.5 - distance, 0.0, 1.0);
    // Preserve the original endpoints' perspective-correct colours. The padded
    // quad is only coverage geometry and must not shift the colour gradient.
    let screen_t = clamp(input.local.x / input.shape.x, 0.0, 1.0);
    let color_t = screen_t * input.inverse_w.y / mix(input.inverse_w.x, input.inverse_w.y, screen_t);
    let color = mix(input.start_color, input.end_color, color_t);
    let alpha = color.a * outer;
    // Transparent bounding-quad pixels must not write depth and hide later geometry.
    if alpha <= 0.0 { discard; }
    var out: FragmentOutput;
    out.color = vec4<f32>(color.rgb * (inner / outer), alpha);
    // Like colour, depth belongs to the original centreline, not the padded quad.
    out.depth = mix(input.depth.x, input.depth.y, screen_t);
    return out;
}
