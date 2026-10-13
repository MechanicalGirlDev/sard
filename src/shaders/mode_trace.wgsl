struct Node {
    min: vec4<f32>,
    max: vec4<f32>,
    // `meta` is reserved in WGSL; the byte layout matches GpuNode::meta.
    links: vec4<u32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> triangles: array<Triangle>;
@group(0) @binding(2) var<storage, read> nodes: array<Node>;
@group(0) @binding(3) var<storage, read_write> pixels: array<vec4<f32>>;

const RAY_FAR: f32 = 3.402823e38;
const NO_TRIANGLE: u32 = 0xffffffffu;

struct Hit {
    distance: f32,
    triangle: u32,
    barycentric: vec2<f32>,
}

fn random(state: ptr<function, u32>) -> f32 {
    // PCG hash: samples are independent of invocation scheduling.
    *state = *state * 747796405u + 2891336453u;
    let word = ((*state >> ((*state >> 28u) + 4u)) ^ *state) * 277803737u;
    let value = (word >> 22u) ^ word;
    return f32(value >> 8u) * (1.0 / 16777216.0);
}

fn box_hit(origin: vec3<f32>, direction: vec3<f32>, node: Node, limit: f32) -> bool {
    var near = 0.0;
    var far = limit;
    for (var axis = 0u; axis < 3u; axis++) {
        // Do not form 0 * infinity for rays on a slab.
        if direction[axis] == 0.0 {
            if origin[axis] < node.min[axis] || origin[axis] > node.max[axis] {
                return false;
            }
        } else {
            let a = (node.min[axis] - origin[axis]) / direction[axis];
            let b = (node.max[axis] - origin[axis]) / direction[axis];
            near = max(near, min(a, b));
            far = min(far, max(a, b));
            if far < near {
                return false;
            }
        }
    }
    return true;
}

fn triangle_hit(origin: vec3<f32>, direction: vec3<f32>, index: u32, limit: f32) -> Hit {
    let triangle = triangles[index];
    let a = triangle.positions[0].xyz;
    let e1 = triangle.positions[1].xyz - a;
    let e2 = triangle.positions[2].xyz - a;
    let p = cross(direction, e2);
    let determinant = dot(e1, p);
    var result = Hit(limit, NO_TRIANGLE, vec2<f32>(0.0));
    // Relative threshold also rejects degenerate triangles.
    if abs(determinant) <= 1e-7 * length(e1) * length(e2) {
        return result;
    }
    let inverse = 1.0 / determinant;
    let s = origin - a;
    let u = dot(s, p) * inverse;
    let q = cross(s, e1);
    let v = dot(direction, q) * inverse;
    let distance = dot(e2, q) * inverse;
    // Dot-product contraction can make an exact shared-edge coordinate slightly
    // negative. A small dimensionless tolerance prevents cracks between faces.
    if u >= -1e-6 && v >= -1e-6 && u + v <= 1.0 + 1e-6 && distance > 0.0 && distance < limit {
        let barycentric = max(vec2<f32>(u, v), vec2<f32>(0.0));
        result = Hit(distance, index, barycentric / max(1.0, barycentric.x + barycentric.y));
    }
    return result;
}

fn trace(origin: vec3<f32>, direction: vec3<f32>, limit: f32, any_hit: bool) -> Hit {
    var closest = Hit(limit, NO_TRIANGLE, vec2<f32>(0.0));
    var index = 0u;
    loop {
        if index >= frame.counts.z {
            break;
        }
        let node = nodes[index];
        if !box_hit(origin, direction, node, closest.distance) {
            index = node.links.z;
            continue;
        }
        if node.links.y == 0u {
            index++;
            continue;
        }
        for (var offset = 0u; offset < node.links.y; offset++) {
            let hit = triangle_hit(origin, direction, node.links.x + offset, closest.distance);
            if hit.triangle != NO_TRIANGLE {
                closest = hit;
                if any_hit {
                    return closest;
                }
            }
        }
        index = node.links.z;
    }
    return closest;
}

fn geometric_normal(triangle: Triangle, direction: vec3<f32>) -> vec3<f32> {
    var normal = normalize(cross(
        triangle.positions[1].xyz - triangle.positions[0].xyz,
        triangle.positions[2].xyz - triangle.positions[0].xyz));
    if dot(normal, direction) > 0.0 {
        normal = -normal;
    }
    return normal;
}

fn hit_normal(triangle: Triangle, hit: Hit, direction: vec3<f32>) -> vec3<f32> {
    let geometric = geometric_normal(triangle, direction);
    let interpolated = triangle.normals[0].xyz * (1.0 - hit.barycentric.x - hit.barycentric.y)
        + triangle.normals[1].xyz * hit.barycentric.x
        + triangle.normals[2].xyz * hit.barycentric.y;
    var normal = geometric;
    if dot(interpolated, interpolated) > 1e-20 {
        normal = normalize(interpolated);
    }
    if dot(normal, direction) > 0.0 {
        normal = -normal;
    }
    return normal;
}

fn offset_origin(position: vec3<f32>, geometric: vec3<f32>, direction: vec3<f32>) -> vec3<f32> {
    let scale = max(1.0, max(abs(position.x), max(abs(position.y), abs(position.z))));
    return position + geometric * select(-1.0, 1.0, dot(geometric, direction) >= 0.0)
        * (2e-6 * scale);
}

fn local_to_world(sample: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let reference = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(normal.z) > 0.999);
    let tangent = normalize(cross(reference, normal));
    return normalize(tangent * sample.x + cross(normal, tangent) * sample.y + normal * sample.z);
}

fn ggx_distribution(nh: f32, roughness: f32) -> f32 {
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let denominator = nh * nh * (alpha2 - 1.0) + 1.0;
    return alpha2 / (PI * denominator * denominator);
}

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= frame.settings.x || id.y >= frame.settings.y {
        return;
    }
    let pixel = id.y * frame.settings.x + id.x;
    let path_mode = frame.counts.x == 2u;
    var state = pixel ^ (frame.settings.z * 277803737u) ^ frame.counts.w ^ 0x9e3779b9u;
    var sample_position = vec2<f32>(f32(id.x) + 0.5, f32(id.y) + 0.5);
    if path_mode {
        sample_position = vec2<f32>(f32(id.x) + random(&state), f32(id.y) + random(&state));
    }
    let ndc = vec2<f32>(
        2.0 * sample_position.x / f32(frame.settings.x) - 1.0,
        1.0 - 2.0 * sample_position.y / f32(frame.settings.y));
    let near_h = frame.inverse_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let far_h = frame.inverse_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    var origin = near_h.xyz / near_h.w;
    let far_point = far_h.xyz / far_h.w;
    var direction = normalize(far_point - origin);
    var limit = length(far_point - origin);
    var throughput = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    var depth = 1.0;
    let bounce_count = max(frame.settings.w, 1u);
    for (var bounce = 0u; bounce < bounce_count; bounce++) {
        let hit = trace(origin, direction, limit, false);
        if hit.triangle == NO_TRIANGLE {
            radiance += throughput * frame.environment.xyz;
            break;
        }
        let position = origin + direction * hit.distance;
        if bounce == 0u {
            let projected = frame.view_proj * vec4<f32>(position, 1.0);
            depth = clamp(projected.z / projected.w, 0.0, 1.0);
        }
        let triangle = triangles[hit.triangle];
        let normal = hit_normal(triangle, hit, direction);
        let geometric = geometric_normal(triangle, direction);
        let view = -direction;
        let albedo = max(triangle.base_color.xyz, vec3<f32>(0.0));
        let metallic = clamp(triangle.emissive_metallic.w, 0.0, 1.0);
        let roughness = clamp(triangle.parameters.x, 0.045, 1.0);
        let f0 = mix(vec3<f32>(0.04), albedo, metallic);
        radiance += throughput * triangle.emissive_metallic.xyz;
        let sun_length = length(frame.sun_direction.xyz);
        if sun_length > 0.0 {
            let light = -frame.sun_direction.xyz / sun_length;
            let cosine = max(dot(normal, light), 0.0);
            if cosine > 0.0 && dot(geometric, light) > 0.0 {
                let shadow_origin = offset_origin(position, geometric, light);
                if trace(shadow_origin, light, RAY_FAR, true).triangle == NO_TRIANGLE {
                    radiance += throughput * brdf(albedo, metallic, roughness, normal, view, light)
                        * frame.sun_color.xyz * cosine;
                }
            }
        }
        if bounce + 1u >= bounce_count {
            break;
        }
        var outgoing: vec3<f32>;
        if !path_mode {
            // Whitted-style deterministic mirror continuation. Rough direct
            // highlights still use the common microfacet BRDF.
            outgoing = reflect(direction, normal);
            throughput *= fresnel(max(dot(normal, view), 0.0), f0);
        } else {
            // Sample either cosine-weighted Lambert or the GGX NDF, then
            // evaluate the full BRDF with the full mixture PDF.
            let specular_probability = clamp(0.25 + 0.5 * metallic, 0.25, 0.75);
            let choose_specular = random(&state) < specular_probability;
            let u1 = random(&state);
            let u2 = random(&state);
            let phi = 2.0 * PI * u1;
            if choose_specular {
                let alpha = roughness * roughness;
                let cosine = sqrt((1.0 - u2) / (1.0 + (alpha * alpha - 1.0) * u2));
                let sine = sqrt(max(0.0, 1.0 - cosine * cosine));
                let halfway = local_to_world(vec3<f32>(cos(phi) * sine, sin(phi) * sine, cosine), normal);
                outgoing = reflect(direction, halfway);
            } else {
                let radius = sqrt(u2);
                outgoing = local_to_world(
                    vec3<f32>(radius * cos(phi), radius * sin(phi), sqrt(1.0 - u2)), normal);
            }
            let nl = dot(normal, outgoing);
            if nl <= 0.0 || dot(geometric, outgoing) <= 0.0 {
                break;
            }
            let halfway = normalize(view + outgoing);
            let nh = max(dot(normal, halfway), 0.0);
            let vh = abs(dot(view, halfway));
            let diffuse_pdf = nl / PI;
            let specular_pdf = ggx_distribution(nh, roughness) * nh / max(4.0 * vh, 1e-20);
            let pdf = mix(diffuse_pdf, specular_pdf, specular_probability);
            if pdf <= 0.0 {
                break;
            }
            throughput *= brdf(albedo, metallic, roughness, normal, view, outgoing) * nl / pdf;
        }
        if max(throughput.x, max(throughput.y, throughput.z)) <= 0.0 {
            break;
        }
        origin = offset_origin(position, geometric, outgoing);
        direction = normalize(outgoing);
        limit = RAY_FAR;
    }
    var mean = radiance;
    if path_mode && frame.settings.z > 0u {
        let previous = pixels[pixel].xyz;
        mean = previous + (radiance - previous) / (f32(frame.settings.z) + 1.0);
    }
    pixels[pixel] = vec4<f32>(mean, depth);
}
