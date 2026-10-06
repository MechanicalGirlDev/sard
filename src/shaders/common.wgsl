// Common shader definitions for sard
// This file provides shared structures and functions used across multiple shaders.

// =============================================================================
// Constants
// =============================================================================

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;
const HALF_PI: f32 = 1.57079632679;
const INV_PI: f32 = 0.31830988618;

// Maximum number of lights per type
const MAX_DIRECTIONAL_LIGHTS: u32 = 4u;
const MAX_POINT_LIGHTS: u32 = 8u;
const MAX_SPOT_LIGHTS: u32 = 8u;

// =============================================================================
// Light Structures
// =============================================================================

struct AmbientLight {
    color: vec3<f32>,
    intensity: f32,
}

struct DirectionalLight {
    direction: vec3<f32>,
    _padding0: f32,
    color: vec3<f32>,
    intensity: f32,
}

struct PointLight {
    position: vec3<f32>,
    _padding0: f32,
    color: vec3<f32>,
    intensity: f32,
    attenuation: vec3<f32>,  // constant, linear, quadratic
    _padding1: f32,
}

struct SpotLight {
    position: vec3<f32>,
    _padding0: f32,
    direction: vec3<f32>,
    _padding1: f32,
    color: vec3<f32>,
    intensity: f32,
    inner_angle: f32,
    outer_angle: f32,
    attenuation: vec2<f32>,  // linear, quadratic (constant = 1.0)
}

struct LightUniform {
    ambient: AmbientLight,
    directional_count: u32,
    point_count: u32,
    spot_count: u32,
    _padding: u32,
    // Note: Arrays would be defined in the actual uniform, not here
}

// =============================================================================
// PBR Helper Functions
// =============================================================================

// Normal Distribution Function (GGX/Trowbridge-Reitz)
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

// Geometry function (Schlick-GGX)
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

// Smith's method for geometry term
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let ggx1 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx2 = geometry_schlick_ggx(n_dot_l, roughness);
    return ggx1 * ggx2;
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

// Fresnel-Schlick with roughness for IBL
fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return f0 + (max(vec3<f32>(1.0 - roughness), f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Calculate F0 (reflectance at normal incidence) for metals
fn calculate_f0(base_color: vec3<f32>, metallic: f32) -> vec3<f32> {
    return mix(vec3<f32>(0.04), base_color, metallic);
}

// =============================================================================
// Lighting Calculations
// =============================================================================

// Point light attenuation
fn point_light_attenuation(distance: f32, attenuation: vec3<f32>) -> f32 {
    return 1.0 / (attenuation.x + attenuation.y * distance + attenuation.z * distance * distance);
}

// Spotlight intensity falloff
fn spotlight_intensity(light_dir: vec3<f32>, spot_dir: vec3<f32>, inner_angle: f32, outer_angle: f32) -> f32 {
    let cos_theta = dot(light_dir, spot_dir);
    let cos_inner = cos(inner_angle);
    let cos_outer = cos(outer_angle);
    return clamp((cos_theta - cos_outer) / (cos_inner - cos_outer), 0.0, 1.0);
}

// =============================================================================
// Shadow Mapping Functions
// =============================================================================

// Basic shadow calculation
fn calculate_shadow(
    shadow_pos: vec4<f32>,
    shadow_map: texture_depth_2d,
    shadow_sampler: sampler_comparison,
) -> f32 {
    // Perspective divide
    let proj_coords = shadow_pos.xyz / shadow_pos.w;

    // Transform to [0, 1] range for texture lookup
    let shadow_uv = vec2<f32>(
        proj_coords.x * 0.5 + 0.5,
        proj_coords.y * -0.5 + 0.5  // Flip Y for texture coordinates
    );

    // Check if outside shadow map
    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 {
        return 1.0;
    }

    // Current depth
    let current_depth = proj_coords.z;

    // Sample shadow map with comparison
    return textureSampleCompare(shadow_map, shadow_sampler, shadow_uv, current_depth);
}

// PCF shadow (Percentage Closer Filtering)
fn calculate_shadow_pcf(
    shadow_pos: vec4<f32>,
    shadow_map: texture_depth_2d,
    shadow_sampler: sampler_comparison,
    texel_size: vec2<f32>,
) -> f32 {
    // Perspective divide
    let proj_coords = shadow_pos.xyz / shadow_pos.w;

    // Transform to [0, 1] range
    let shadow_uv = vec2<f32>(
        proj_coords.x * 0.5 + 0.5,
        proj_coords.y * -0.5 + 0.5
    );

    // Check bounds
    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 {
        return 1.0;
    }

    let current_depth = proj_coords.z;

    // 3x3 PCF kernel
    var shadow: f32 = 0.0;
    for (var x: i32 = -1; x <= 1; x++) {
        for (var y: i32 = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompare(shadow_map, shadow_sampler, shadow_uv + offset, current_depth);
        }
    }

    return shadow / 9.0;
}

// =============================================================================
// Utility Functions
// =============================================================================

// Linear to sRGB conversion
fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
    let cutoff = step(linear, vec3<f32>(0.0031308));
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return mix(high, low, cutoff);
}

// sRGB to linear conversion
fn srgb_to_linear(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = step(srgb, vec3<f32>(0.04045));
    let low = srgb / 12.92;
    let high = pow((srgb + 0.055) / 1.055, vec3<f32>(2.4));
    return mix(high, low, cutoff);
}

// Tonemap (ACES approximation)
fn tonemap_aces(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((color * (a * color + b)) / (color * (c * color + d) + e));
}

// Reinhard tonemap
fn tonemap_reinhard(color: vec3<f32>) -> vec3<f32> {
    return color / (color + vec3<f32>(1.0));
}

// Calculate fog factor (linear)
fn fog_factor_linear(distance: f32, fog_start: f32, fog_end: f32) -> f32 {
    return clamp((fog_end - distance) / (fog_end - fog_start), 0.0, 1.0);
}

// Calculate fog factor (exponential)
fn fog_factor_exp(distance: f32, density: f32) -> f32 {
    return exp(-density * distance);
}

// Calculate fog factor (exponential squared)
fn fog_factor_exp2(distance: f32, density: f32) -> f32 {
    let f = density * distance;
    return exp(-f * f);
}

// Pack normal to octahedron encoding (for G-buffer)
fn encode_normal_octahedron(n: vec3<f32>) -> vec2<f32> {
    let l1_norm = abs(n.x) + abs(n.y) + abs(n.z);
    var result = n.xy / l1_norm;
    if n.z < 0.0 {
        result = (1.0 - abs(result.yx)) * sign(result);
    }
    return result * 0.5 + 0.5;
}

// Unpack normal from octahedron encoding
fn decode_normal_octahedron(encoded: vec2<f32>) -> vec3<f32> {
    var n = encoded * 2.0 - 1.0;
    let z = 1.0 - abs(n.x) - abs(n.y);
    if z < 0.0 {
        n = (1.0 - abs(n.yx)) * sign(n);
    }
    return normalize(vec3<f32>(n, z));
}
