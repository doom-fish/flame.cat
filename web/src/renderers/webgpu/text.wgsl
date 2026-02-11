// SDF text rendering shader.
// Each instance is one glyph quad with position, size, UV rect, and color.

struct TextVertex {
  @location(0) quad_pos: vec2f,   // unit quad corner (0,0)→(1,1)
  @location(1) inst_pos: vec2f,   // glyph x, y in CSS pixels
  @location(2) inst_size: vec2f,  // glyph w, h in CSS pixels
  @location(3) inst_uv_min: vec2f,  // UV top-left in atlas
  @location(4) inst_uv_max: vec2f,  // UV bottom-right in atlas
  @location(5) inst_color: vec4f,   // text color RGBA
}

struct TextOutput {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
  @location(1) color: vec4f,
}

struct Uniforms {
  viewport_size: vec2f,
  scroll_offset: vec2f,
  scale: vec2f,
  dpr: f32,
  _pad: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var sdf_texture: texture_2d<f32>;
@group(0) @binding(2) var sdf_sampler: sampler;

@vertex
fn vs_main(v: TextVertex) -> TextOutput {
  var out: TextOutput;

  let px = (v.inst_pos + v.quad_pos * v.inst_size - u.scroll_offset) * u.scale;
  let snapped = floor(px * u.dpr + 0.5) / u.dpr;
  let ndc = snapped / u.viewport_size * 2.0 - 1.0;

  out.position = vec4f(ndc.x, -ndc.y, 0.0, 1.0);
  out.uv = mix(v.inst_uv_min, v.inst_uv_max, v.quad_pos);
  out.color = v.inst_color;
  return out;
}

@fragment
fn fs_main(v: TextOutput) -> @location(0) vec4f {
  let dist = textureSample(sdf_texture, sdf_sampler, v.uv).r;

  // Smoothstep for anti-aliased edges
  // Adjust smoothing based on glyph size for consistent quality
  let smoothing = 0.1;
  let alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, dist);

  return vec4f(v.color.rgb, v.color.a * alpha);
}
