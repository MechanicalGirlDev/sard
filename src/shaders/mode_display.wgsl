@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> pixels: array<vec4<f32>>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

struct Output {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> Output {
    let index = u32(position.y) * frame.settings.x + u32(position.x);
    let pixel = pixels[index];
    var output: Output;
    output.color = vec4<f32>(tone_map(pixel.xyz), 1.0);
    output.depth = pixel.w;
    return output;
}
