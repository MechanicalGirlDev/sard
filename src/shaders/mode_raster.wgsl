@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> triangles: array<Triangle>;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) base_color: vec3<f32>,
    @location(3) @interpolate(flat) emissive_metallic: vec4<f32>,
    @location(4) @interpolate(flat) parameters: vec2<f32>,
}

@vertex
fn vs_shadow(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    return frame.light_view_proj * triangles[index / 3u].positions[index % 3u];
}

@fragment
fn fs_shadow() {}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let triangle = triangles[index / 3u];
    let p = triangle.positions[index % 3u];
    var output: VertexOutput;
    output.position = frame.view_proj * p;
    output.world_position = p.xyz;
    output.normal = triangle.normals[index % 3u].xyz;
    output.base_color = triangle.base_color.xyz;
    output.emissive_metallic = triangle.emissive_metallic;
    output.parameters = triangle.parameters.xy;
    return output;
}

fn visibility(position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let light = frame.light_view_proj * vec4<f32>(position, 1.0);
    let ndc = light.xyz / light.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || ndc.z < 0.0 || ndc.z > 1.0 {
        return 1.0;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_map));
    let bias = max(0.00005, 0.0004 * (1.0 - max(dot(normal, -frame.sun_direction.xyz), 0.0)));
    var value = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            value += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2<f32>(f32(x), f32(y)) * texel, ndc.z - bias);
        }
    }
    return value / 9.0;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.position.xy / vec2<f32>(frame.settings.xy);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let near = frame.inverse_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let far = frame.inverse_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let v = normalize(near.xyz / near.w - far.xyz / far.w);
    let normal = normalize(input.normal);
    let n = normal * select(-1.0, 1.0, dot(normal, v) >= 0.0);
    let l = -frame.sun_direction.xyz;
    let metallic = input.emissive_metallic.w;
    let f = fresnel(max(dot(n, v), 0.0), mix(vec3<f32>(0.04), input.base_color, metallic));
    let ambient = frame.environment.xyz * input.parameters.y *
        ((vec3<f32>(1.0) - f) * (1.0 - metallic) * input.base_color + f);
    let direct = brdf(input.base_color, metallic, input.parameters.x, n, v, l) *
        max(dot(n, l), 0.0) * frame.sun_color.xyz * visibility(input.world_position, n);
    return vec4<f32>(tone_map(input.emissive_metallic.xyz + ambient + direct), 1.0);
}
