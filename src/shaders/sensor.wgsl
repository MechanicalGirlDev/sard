struct Camera {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    eye: vec4<f32>,
};
struct Model {
    transform: mat4x4<f32>,
    color: vec4<f32>,
    segmentation_id: u32,
    textured: u32,
    billboard: u32,
    padding2: u32,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> model: Model;
@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) metric_depth: f32,
    @location(1) uv: vec2<f32>,
};
@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) offset: vec2<f32>,
) -> VertexOutput {
    let world_center = model.transform * vec4<f32>(position, 1.0);
    var world_position = world_center;
    if model.billboard != 0u {
        let forward = normalize(camera.eye.xyz - world_center.xyz);
        var world_up = vec3<f32>(0.0, 1.0, 0.0);
        if abs(dot(forward, world_up)) > 0.999 {
            world_up = vec3<f32>(0.0, 0.0, 1.0);
        }
        let right = normalize(cross(world_up, forward));
        let up = cross(forward, right);
        world_position = vec4<f32>(world_center.xyz + right * offset.x + up * offset.y, 1.0);
    }
    let view_position = camera.view * world_position;
    var output: VertexOutput;
    output.position = camera.projection * view_position;
    output.metric_depth = -view_position.z;
    output.uv = uv;
    return output;
}
struct SensorOutput {
    @location(0) rgb: vec4<f32>,
    @location(1) depth: f32,
    @location(2) segmentation: u32,
};
@fragment
fn fs_main(input: VertexOutput) -> SensorOutput {
    var output: SensorOutput;
    output.rgb = model.color;
    if model.textured != 0u {
        output.rgb *= vec4<f32>(textureSample(image, image_sampler, input.uv).rgb, 1.0);
    }
    output.depth = input.metric_depth;
    output.segmentation = model.segmentation_id;
    return output;
}
