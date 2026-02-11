import type { RenderCommand, ThemeToken } from "../../protocol";
import type { Theme } from "../../themes";
import { resolveColor } from "../../themes";
import rectShaderSource from "./rect.wgsl?raw";
import textShaderSource from "./text.wgsl?raw";
import { generateSDFAtlas, type GlyphAtlas, type GlyphMetrics } from "./sdf-atlas";

/** Per-instance rect: x, y, w, h, r, g, b, a */
const RECT_FLOATS = 8;
const RECT_BYTES = RECT_FLOATS * 4;

/** Per-instance glyph: x, y, w, h, u0, v0, u1, v1, r, g, b, a */
const GLYPH_FLOATS = 12;
const GLYPH_BYTES = GLYPH_FLOATS * 4;

/**
 * 100% WebGPU renderer — no Canvas2D.
 * Rects + lines via instanced rect pipeline.
 * Text via SDF glyph atlas pipeline.
 * Clipping via GPU scissor rects.
 */
export class WebGPURenderer {
  private device!: GPUDevice;
  private gpuContext!: GPUCanvasContext;
  private rectPipeline!: GPURenderPipeline;
  private textPipeline!: GPURenderPipeline;
  private uniformBuffer!: GPUBuffer;
  private rectBindGroup!: GPUBindGroup;
  private textBindGroup!: GPUBindGroup;
  private quadVertexBuffer!: GPUBuffer;
  private rectInstanceBuffer!: GPUBuffer;
  private rectInstanceCapacity = 0;
  private glyphInstanceBuffer!: GPUBuffer;
  private glyphInstanceCapacity = 0;
  private atlas!: GlyphAtlas;
  private canvas: HTMLCanvasElement;
  private theme: Theme;

  constructor(canvas: HTMLCanvasElement, theme: Theme) {
    this.canvas = canvas;
    this.theme = theme;
  }

  async init(): Promise<void> {
    const adapter = await navigator.gpu?.requestAdapter();
    if (!adapter) throw new Error("WebGPU not supported");
    this.device = await adapter.requestDevice();

    const context = this.canvas.getContext("webgpu");
    if (!context) throw new Error("Failed to get webgpu context");
    this.gpuContext = context;

    const format = navigator.gpu.getPreferredCanvasFormat();
    this.gpuContext.configure({ device: this.device, format, alphaMode: "premultiplied" });

    // Generate SDF glyph atlas
    this.atlas = generateSDFAtlas(this.device);

    // Shared unit quad (two triangles)
    const quadVerts = new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]);
    this.quadVertexBuffer = this.device.createBuffer({
      size: quadVerts.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    this.device.queue.writeBuffer(this.quadVertexBuffer, 0, quadVerts);

    // Shared uniform buffer
    this.uniformBuffer = this.device.createBuffer({
      size: 32,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });

    const blendState: GPUBlendState = {
      color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha", operation: "add" },
      alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
    };

    // --- Rect pipeline ---
    const rectModule = this.device.createShaderModule({ code: rectShaderSource });
    const rectBGL = this.device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "uniform" } }],
    });
    this.rectBindGroup = this.device.createBindGroup({
      layout: rectBGL,
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
    });
    this.rectPipeline = this.device.createRenderPipeline({
      layout: this.device.createPipelineLayout({ bindGroupLayouts: [rectBGL] }),
      vertex: {
        module: rectModule,
        entryPoint: "vs_main",
        buffers: [
          { arrayStride: 8, stepMode: "vertex", attributes: [{ shaderLocation: 0, offset: 0, format: "float32x2" }] },
          {
            arrayStride: RECT_BYTES,
            stepMode: "instance",
            attributes: [
              { shaderLocation: 1, offset: 0, format: "float32x2" },
              { shaderLocation: 2, offset: 8, format: "float32x2" },
              { shaderLocation: 3, offset: 16, format: "float32x4" },
            ],
          },
        ],
      },
      fragment: { module: rectModule, entryPoint: "fs_main", targets: [{ format, blend: blendState }] },
      primitive: { topology: "triangle-list" },
    });

    // --- Text pipeline ---
    const textModule = this.device.createShaderModule({ code: textShaderSource });
    const sampler = this.device.createSampler({ magFilter: "linear", minFilter: "linear" });
    const textBGL = this.device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "uniform" } },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "float" } },
        { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: { type: "filtering" } },
      ],
    });
    this.textBindGroup = this.device.createBindGroup({
      layout: textBGL,
      entries: [
        { binding: 0, resource: { buffer: this.uniformBuffer } },
        { binding: 1, resource: this.atlas.texture.createView() },
        { binding: 2, resource: sampler },
      ],
    });
    this.textPipeline = this.device.createRenderPipeline({
      layout: this.device.createPipelineLayout({ bindGroupLayouts: [textBGL] }),
      vertex: {
        module: textModule,
        entryPoint: "vs_main",
        buffers: [
          { arrayStride: 8, stepMode: "vertex", attributes: [{ shaderLocation: 0, offset: 0, format: "float32x2" }] },
          {
            arrayStride: GLYPH_BYTES,
            stepMode: "instance",
            attributes: [
              { shaderLocation: 1, offset: 0, format: "float32x2" },   // pos
              { shaderLocation: 2, offset: 8, format: "float32x2" },   // size
              { shaderLocation: 3, offset: 16, format: "float32x2" },  // uv_min
              { shaderLocation: 4, offset: 24, format: "float32x2" },  // uv_max
              { shaderLocation: 5, offset: 32, format: "float32x4" },  // color
            ],
          },
        ],
      },
      fragment: { module: textModule, entryPoint: "fs_main", targets: [{ format, blend: blendState }] },
      primitive: { topology: "triangle-list" },
    });

    this.ensureRectBuffer(2048);
    this.ensureGlyphBuffer(4096);
  }

  setTheme(theme: Theme): void {
    this.theme = theme;
  }

  render(commands: RenderCommand[], scrollX: number, scrollY: number): void {
    const dpr = window.devicePixelRatio;
    const width = this.canvas.clientWidth;
    const height = this.canvas.clientHeight;
    const pxW = Math.round(width * dpr);
    const pxH = Math.round(height * dpr);
    this.canvas.width = pxW;
    this.canvas.height = pxH;

    const uniforms = new Float32Array([width, height, scrollX, scrollY, 1, 1, dpr, 0]);
    this.device.queue.writeBuffer(this.uniformBuffer, 0, uniforms);

    // --- Collect all geometry in a single pass ---
    const rects: number[] = [];
    const glyphs: number[] = [];
    const transformStack: { tx: number; ty: number; sx: number; sy: number }[] = [];
    let curTx = 0, curTy = 0, curSx = 1, curSy = 1;
    // Clip stack: pixel coordinates for scissor rects
    const clipStack: { x: number; y: number; w: number; h: number }[] = [];

    // Draw calls segmented by clip changes
    type DrawBatch = {
      rectStart: number; rectCount: number;
      glyphStart: number; glyphCount: number;
      scissor: { x: number; y: number; w: number; h: number } | null;
    };
    const batches: DrawBatch[] = [];
    let batchRectStart = 0;
    let batchGlyphStart = 0;
    let currentScissor: DrawBatch["scissor"] = null;

    const flushBatch = () => {
      const rc = (rects.length / RECT_FLOATS) - batchRectStart;
      const gc = (glyphs.length / GLYPH_FLOATS) - batchGlyphStart;
      if (rc > 0 || gc > 0) {
        batches.push({
          rectStart: batchRectStart, rectCount: rc,
          glyphStart: batchGlyphStart, glyphCount: gc,
          scissor: currentScissor,
        });
      }
      batchRectStart = rects.length / RECT_FLOATS;
      batchGlyphStart = glyphs.length / GLYPH_FLOATS;
    };

    const pushRect = (x: number, y: number, w: number, h: number, c: { r: number; g: number; b: number; a: number }) => {
      rects.push(x, y, w, h, c.r, c.g, c.b, c.a);
    };

    const layoutText = (
      text: string, x: number, y: number, fontSize: number,
      color: { r: number; g: number; b: number; a: number },
      align: string, maxWidth?: number,
    ) => {
      const scale = fontSize / this.atlas.atlasSize;
      // Measure total width
      let totalW = 0;
      for (const ch of text) {
        const m = this.atlas.metrics.get(ch);
        if (m) totalW += m.advance * scale;
      }

      // Truncate with ellipsis if needed
      let renderText = text;
      if (maxWidth !== undefined && totalW > maxWidth && maxWidth > 0) {
        const ellipsis = this.atlas.metrics.get("…");
        const ellipsisW = ellipsis ? ellipsis.advance * scale : fontSize * 0.5;
        const budget = maxWidth - ellipsisW;
        if (budget <= 0) return; // too narrow
        let w = 0;
        let cutIdx = 0;
        for (let i = 0; i < text.length; i++) {
          const m = this.atlas.metrics.get(text[i]!);
          const advance = m ? m.advance * scale : 0;
          if (w + advance > budget) break;
          w += advance;
          cutIdx = i + 1;
        }
        renderText = text.slice(0, cutIdx) + "…";
        totalW = w + ellipsisW;
      }

      // Apply alignment offset
      let ox = x;
      if (align === "Center") ox -= totalW / 2;
      else if (align === "Right") ox -= totalW;

      // Emit glyph instances
      const halfLineH = (this.atlas.lineHeight * scale) / 2;
      for (const ch of renderText) {
        const m = this.atlas.metrics.get(ch);
        if (!m) { ox += fontSize * 0.4; continue; }
        const gw = m.width * scale;
        const gh = m.height * scale;
        const gx = ox + m.bearingX * scale;
        const gy = y - halfLineH + m.bearingY * scale;
        glyphs.push(gx, gy, gw, gh, m.u0, m.v0, m.u1, m.v1, color.r, color.g, color.b, color.a);
        ox += m.advance * scale;
      }
    };

    for (const cmd of commands) {
      if (typeof cmd === "string") {
        if (cmd === "PopTransform") {
          const prev = transformStack.pop();
          if (prev) { curTx = prev.tx; curTy = prev.ty; curSx = prev.sx; curSy = prev.sy; }
          else { curTx = 0; curTy = 0; curSx = 1; curSy = 1; }
        } else if (cmd === "ClearClip") {
          flushBatch();
          clipStack.pop();
          currentScissor = clipStack.length > 0 ? clipStack[clipStack.length - 1]! : null;
        }
        continue;
      }

      if ("PushTransform" in cmd) {
        transformStack.push({ tx: curTx, ty: curTy, sx: curSx, sy: curSy });
        curTx += cmd.PushTransform.translate.x * curSx;
        curTy += cmd.PushTransform.translate.y * curSy;
        curSx *= cmd.PushTransform.scale.x;
        curSy *= cmd.PushTransform.scale.y;
      } else if ("SetClip" in cmd) {
        flushBatch();
        const cr = cmd.SetClip.rect;
        const cx = (cr.x * curSx + curTx - scrollX) * dpr;
        const cy = (cr.y * curSy + curTy - scrollY) * dpr;
        const cw = cr.w * curSx * dpr;
        const ch = cr.h * curSy * dpr;
        // Intersect with current scissor
        let sx = Math.max(0, cx), sy = Math.max(0, cy);
        let sw = Math.min(pxW, cx + cw) - sx;
        let sh = Math.min(pxH, cy + ch) - sy;
        if (currentScissor) {
          const nx = Math.max(sx, currentScissor.x);
          const ny = Math.max(sy, currentScissor.y);
          sw = Math.min(sx + sw, currentScissor.x + currentScissor.w) - nx;
          sh = Math.min(sy + sh, currentScissor.y + currentScissor.h) - ny;
          sx = nx; sy = ny;
        }
        const clip = { x: Math.round(sx), y: Math.round(sy), w: Math.max(0, Math.round(sw)), h: Math.max(0, Math.round(sh)) };
        clipStack.push(clip);
        currentScissor = clip;
      } else if ("DrawRect" in cmd) {
        const { rect, color, border_color, label } = cmd.DrawRect;
        const c = this.resolveToken(color);
        const x = rect.x * curSx + curTx;
        const y = rect.y * curSy + curTy;
        const w = rect.w * curSx;
        const h = rect.h * curSy;
        pushRect(x, y, w, h, c);
        if (border_color) {
          const bc = this.resolveToken(border_color);
          const lw = 1 / dpr;
          pushRect(x, y, w, lw, bc);
          pushRect(x, y + h - lw, w, lw, bc);
          pushRect(x, y, lw, h, bc);
          pushRect(x + w - lw, y, lw, h, bc);
        }
        if (label && w > 20) {
          const tc = this.resolveToken("TextPrimary");
          layoutText(label, x + 4, y + h / 2, 11, tc, "Left", w - 8);
        }
      } else if ("DrawText" in cmd) {
        const { position, text, color, font_size, align } = cmd.DrawText;
        const c = this.resolveToken(color);
        const px = position.x * curSx + curTx;
        const py = position.y * curSy + curTy;
        layoutText(text, px, py, font_size, c, align);
      } else if ("DrawLine" in cmd) {
        const { from, to, color, width: lineWidth } = cmd.DrawLine;
        const c = this.resolveToken(color);
        const fx = from.x * curSx + curTx;
        const fy = from.y * curSy + curTy;
        const tx = to.x * curSx + curTx;
        const ty = to.y * curSy + curTy;
        // Render line as thin rect
        const dx = tx - fx, dy = ty - fy;
        const len = Math.sqrt(dx * dx + dy * dy);
        if (len < 0.1) continue;
        if (Math.abs(dx) < 0.1) {
          // Vertical line
          pushRect(fx - lineWidth / 2, Math.min(fy, ty), lineWidth, Math.abs(dy), c);
        } else if (Math.abs(dy) < 0.1) {
          // Horizontal line
          pushRect(Math.min(fx, tx), fy - lineWidth / 2, Math.abs(dx), lineWidth, c);
        } else {
          // Diagonal — approximate as rect (rare in flame graphs)
          pushRect(Math.min(fx, tx), Math.min(fy, ty), Math.abs(dx), Math.max(lineWidth, Math.abs(dy)), c);
        }
      }
    }

    // Flush final batch
    flushBatch();

    // --- Upload and draw ---
    const totalRects = rects.length / RECT_FLOATS;
    const totalGlyphs = glyphs.length / GLYPH_FLOATS;

    if (totalRects > 0) {
      this.ensureRectBuffer(totalRects);
      this.device.queue.writeBuffer(this.rectInstanceBuffer, 0, new Float32Array(rects));
    }
    if (totalGlyphs > 0) {
      this.ensureGlyphBuffer(totalGlyphs);
      this.device.queue.writeBuffer(this.glyphInstanceBuffer, 0, new Float32Array(glyphs));
    }

    const encoder = this.device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: this.gpuContext.getCurrentTexture().createView(),
        clearValue: this.bgColor(),
        loadOp: "clear",
        storeOp: "store",
      }],
    });

    for (const batch of batches) {
      if (batch.scissor) {
        pass.setScissorRect(batch.scissor.x, batch.scissor.y, batch.scissor.w, batch.scissor.h);
      } else {
        pass.setScissorRect(0, 0, pxW, pxH);
      }

      // Draw rects
      if (batch.rectCount > 0) {
        pass.setPipeline(this.rectPipeline);
        pass.setBindGroup(0, this.rectBindGroup);
        pass.setVertexBuffer(0, this.quadVertexBuffer);
        pass.setVertexBuffer(1, this.rectInstanceBuffer);
        pass.draw(6, batch.rectCount, 0, batch.rectStart);
      }

      // Draw glyphs
      if (batch.glyphCount > 0) {
        pass.setPipeline(this.textPipeline);
        pass.setBindGroup(0, this.textBindGroup);
        pass.setVertexBuffer(0, this.quadVertexBuffer);
        pass.setVertexBuffer(1, this.glyphInstanceBuffer);
        pass.draw(6, batch.glyphCount, 0, batch.glyphStart);
      }
    }

    pass.end();
    this.device.queue.submit([encoder.finish()]);
  }

  private resolveToken(token: ThemeToken): { r: number; g: number; b: number; a: number } {
    const c = resolveColor(this.theme, token);
    return { r: c.r, g: c.g, b: c.b, a: c.a };
  }

  private bgColor(): GPUColor {
    const bg = resolveColor(this.theme, "Background");
    return { r: bg.r, g: bg.g, b: bg.b, a: bg.a };
  }

  private ensureRectBuffer(count: number): void {
    if (count <= this.rectInstanceCapacity) return;
    const cap = Math.max(count, this.rectInstanceCapacity * 2, 2048);
    this.rectInstanceBuffer?.destroy();
    this.rectInstanceBuffer = this.device.createBuffer({
      size: cap * RECT_BYTES,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    this.rectInstanceCapacity = cap;
  }

  private ensureGlyphBuffer(count: number): void {
    if (count <= this.glyphInstanceCapacity) return;
    const cap = Math.max(count, this.glyphInstanceCapacity * 2, 4096);
    this.glyphInstanceBuffer?.destroy();
    this.glyphInstanceBuffer = this.device.createBuffer({
      size: cap * GLYPH_BYTES,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    this.glyphInstanceCapacity = cap;
  }
}
