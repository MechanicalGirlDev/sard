const PI: f32 = 3.14159265359;

struct Triangle {
    positions: array<vec4<f32>, 3>,
    normals: array<vec4<f32>, 3>,
    base_color: vec4<f32>,
    emissive_metallic: vec4<f32>,
    parameters: vec4<f32>,
}

struct Frame {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    environment: vec4<f32>,
    settings: vec4<u32>,
    counts: vec4<u32>,
}

fn fresnel(cosine: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - clamp(cosine, 0.0, 1.0), 5.0);
}

fn brdf(albedo: vec3<f32>, metallic: f32, roughness: f32,
        n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    let nv = max(dot(n, v), 0.0);
    let nl = max(dot(n, l), 0.0);
    if nv <= 0.0 || nl <= 0.0 { return vec3<f32>(0.0); }
    let h = normalize(v + l);
    let nh = max(dot(n, h), 0.0);
    let alpha = max(roughness * roughness, 0.0004);
    let a2 = alpha * alpha;
    let denominator = nh * nh * (a2 - 1.0) + 1.0;
    let distribution = a2 / max(PI * denominator * denominator, 1e-10);
    // Smith's separable GGX masking-shadowing function.
    let gv = 2.0 * nv / max(nv + sqrt(a2 + (1.0 - a2) * nv * nv), 1e-6);
    let gl = 2.0 * nl / max(nl + sqrt(a2 + (1.0 - a2) * nl * nl), 1e-6);
    let f = fresnel(max(dot(h, v), 0.0), mix(vec3<f32>(0.04), albedo, metallic));
    let specular = distribution * gv * gl * f / max(4.0 * nv * nl, 1e-6);
    let diffuse = (vec3<f32>(1.0) - f) * (1.0 - metallic) * albedo / PI;
    return diffuse + specular;
}

fn tone_map(color: vec3<f32>) -> vec3<f32> {
    return max(color, vec3<f32>(0.0)) / (vec3<f32>(1.0) + max(color, vec3<f32>(0.0)));
}
