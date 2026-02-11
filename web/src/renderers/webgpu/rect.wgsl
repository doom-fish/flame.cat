// Quad vertex shader + fragment shader for instanced rectangle rendering.
// Each instance provides position, size, and color via instance buffer.

struct VertexInput {
  @location(0) quad_pos: vec2f,     // unit quad corner (0,0)→(1,1)
  @location(1) inst_pos: vec2f,     // rect x, y
  @location(2) inst_size: vec2f,    // rect w, h
  @location(3) inst_color: vec4f,   // resolved RGBA color
}

struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
}

struct Uniforms {
  viewport_size: vec2f,
  scroll_offset: vec2f,
  scale: vec2f,
  dpr: f32,
  _pad: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
  var out: VertexOutput;

  // Pixel position of this vertex
  let px = (v.inst_pos + v.quad_pos * v.inst_size - u.scroll_offset) * u.scale;

  // Pixel-snap to device pixels for crisp edges
  let snapped = floor(px * u.dpr + 0.5) / u.dpr;

  // Convert to clip space: [0, viewport] → [-1, 1]
  let ndc = snapped / u.viewport_size * 2.0 - 1.0;

  out.position = vec4f(ndc.x, -ndc.y, 0.0, 1.0); // flip Y for screen coords
  out.color = v.inst_color;
  return out;
}

@fragment
fn fs_main(v: VertexOutput) -> @location(0) vec4f {
  return v.color;
}
