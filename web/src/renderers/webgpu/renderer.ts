import type { RenderCommand, ThemeToken } from "../../protocol";
import type { Theme, Color } from "../../themes";
import { resolveColor } from "../../themes";
import shaderSource from "./rect.wgsl?raw";

/** Per-instance data for one rectangle: x, y, w, h, r, g, b, a */
const INSTANCE_FLOATS = 8;
const INSTANCE_BYTES = INSTANCE_FLOATS * 4;

/**
 * Hybrid WebGPU + Canvas2D renderer.
 * WebGPU handles all rectangle rendering (instanced draws).
 * An overlay Canvas2D handles text, lines, and clipping.
 */
export class WebGPURenderer {
  private device!: GPUDevice;
  private gpuContext!: GPUCanvasContext;
  private pipeline!: GPURenderPipeline;
  private uniformBuffer!: GPUBuffer;
  private uniformBindGroup!: GPUBindGroup;
  private quadVertexBuffer!: GPUBuffer;
  private instanceBuffer!: GPUBuffer;
  private instanceCapacity = 0;
  private canvas: HTMLCanvasElement;
  private overlayCanvas!: HTMLCanvasElement;
  private ctx!: CanvasRenderingContext2D;
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

    // Create overlay canvas for text/lines
    this.overlayCanvas = document.createElement("canvas");
    this.overlayCanvas.style.position = "absolute";
    this.overlayCanvas.style.top = "0";
    this.overlayCanvas.style.left = "0";
    this.overlayCanvas.style.width = "100%";
    this.overlayCanvas.style.height = "100%";
    this.overlayCanvas.style.pointerEvents = "none";
    this.canvas.parentElement?.appendChild(this.overlayCanvas);
    const ctx = this.overlayCanvas.getContext("2d");
    if (!ctx) throw new Error("Failed to get overlay 2d context");
    this.ctx = ctx;

    // Unit quad (two triangles)
    const quadVerts = new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]);
    this.quadVertexBuffer = this.device.createBuffer({
      size: quadVerts.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    this.device.queue.writeBuffer(this.quadVertexBuffer, 0, quadVerts);

    // Uniform buffer: viewport_size(2) + scroll_offset(2) + scale(2) + dpr(1) + pad(1) = 8 floats
    this.uniformBuffer = this.device.createBuffer({
      size: 32,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });

    const shaderModule = this.device.createShaderModule({ code: shaderSource });

    const bindGroupLayout = this.device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "uniform" } }],
    });

    this.uniformBindGroup = this.device.createBindGroup({
      layout: bindGroupLayout,
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
    });

    const pipelineLayout = this.device.createPipelineLayout({
      bindGroupLayouts: [bindGroupLayout],
    });

    this.pipeline = this.device.createRenderPipeline({
      layout: pipelineLayout,
      vertex: {
        module: shaderModule,
        entryPoint: "vs_main",
        buffers: [
          {
            arrayStride: 8,
            stepMode: "vertex",
            attributes: [{ shaderLocation: 0, offset: 0, format: "float32x2" }],
          },
          {
            arrayStride: INSTANCE_BYTES,
            stepMode: "instance",
            attributes: [
              { shaderLocation: 1, offset: 0, format: "float32x2" },  // pos
              { shaderLocation: 2, offset: 8, format: "float32x2" },  // size
              { shaderLocation: 3, offset: 16, format: "float32x4" }, // color
            ],
          },
        ],
      },
      fragment: {
        module: shaderModule,
        entryPoint: "fs_main",
        targets: [
          {
            format,
            blend: {
              color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha", operation: "add" },
              alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
            },
          },
        ],
      },
      primitive: { topology: "triangle-list" },
    });

    this.ensureInstanceBuffer(1024);
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
    this.overlayCanvas.width = pxW;
    this.overlayCanvas.height = pxH;

    // --- Pass 1: Collect and render rects via WebGPU ---
    const uniforms = new Float32Array([width, height, scrollX, scrollY, 1, 1, dpr, 0]);
    this.device.queue.writeBuffer(this.uniformBuffer, 0, uniforms);

    // Flatten transforms and collect rects with applied transforms
    const rects: { x: number; y: number; w: number; h: number; r: number; g: number; b: number; a: number }[] = [];
    const transformStack: { tx: number; ty: number; sx: number; sy: number }[] = [];
    let curTx = 0, curTy = 0, curSx = 1, curSy = 1;

    for (const cmd of commands) {
      if (typeof cmd === "string") {
        if (cmd === "PopTransform") {
          const prev = transformStack.pop();
          if (prev) { curTx = prev.tx; curTy = prev.ty; curSx = prev.sx; curSy = prev.sy; }
          else { curTx = 0; curTy = 0; curSx = 1; curSy = 1; }
        }
        continue;
      }
      if ("PushTransform" in cmd) {
        transformStack.push({ tx: curTx, ty: curTy, sx: curSx, sy: curSy });
        curTx += cmd.PushTransform.translate.x * curSx;
        curTy += cmd.PushTransform.translate.y * curSy;
        curSx *= cmd.PushTransform.scale.x;
        curSy *= cmd.PushTransform.scale.y;
      } else if ("DrawRect" in cmd) {
        const { rect, color, border_color } = cmd.DrawRect;
        const c = this.resolveToken(color);
        const x = rect.x * curSx + curTx;
        const y = rect.y * curSy + curTy;
        const w = rect.w * curSx;
        const h = rect.h * curSy;
        rects.push({ x, y, w, h, ...c });
        if (border_color) {
          const bc = this.resolveToken(border_color);
          const lw = 1 / dpr;
          // Top
          rects.push({ x, y, w, h: lw, r: bc.r, g: bc.g, b: bc.b, a: bc.a });
          // Bottom
          rects.push({ x, y: y + h - lw, w, h: lw, r: bc.r, g: bc.g, b: bc.b, a: bc.a });
          // Left
          rects.push({ x, y, w: lw, h, r: bc.r, g: bc.g, b: bc.b, a: bc.a });
          // Right
          rects.push({ x: x + w - lw, y, w: lw, h, r: bc.r, g: bc.g, b: bc.b, a: bc.a });
        }
      }
    }

    if (rects.length > 0) {
      this.ensureInstanceBuffer(rects.length);
      const instanceData = new Float32Array(rects.length * INSTANCE_FLOATS);
      for (let i = 0; i < rects.length; i++) {
        const r = rects[i];
        if (!r) continue;
        const off = i * INSTANCE_FLOATS;
        instanceData[off] = r.x;
        instanceData[off + 1] = r.y;
        instanceData[off + 2] = r.w;
        instanceData[off + 3] = r.h;
        instanceData[off + 4] = r.r;
        instanceData[off + 5] = r.g;
        instanceData[off + 6] = r.b;
        instanceData[off + 7] = r.a;
      }
      this.device.queue.writeBuffer(this.instanceBuffer, 0, instanceData);

      const encoder = this.device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: this.gpuContext.getCurrentTexture().createView(),
            clearValue: this.bgColor(),
            loadOp: "clear",
            storeOp: "store",
          },
        ],
      });
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, this.uniformBindGroup);
      pass.setVertexBuffer(0, this.quadVertexBuffer);
      pass.setVertexBuffer(1, this.instanceBuffer);
      pass.draw(6, rects.length);
      pass.end();
      this.device.queue.submit([encoder.finish()]);
    }

    // --- Pass 2: Overlay text, lines, labels via Canvas2D ---
    const ctx = this.ctx;
    ctx.clearRect(0, 0, pxW, pxH);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.translate(-scrollX, -scrollY);

    const txStack: { tx: number; ty: number; sx: number; sy: number }[] = [];
    let otx = 0, oty = 0;

    for (const cmd of commands) {
      if (typeof cmd === "string") {
        if (cmd === "ClearClip") {
          ctx.restore();
          ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
          ctx.translate(-scrollX + otx, -scrollY + oty);
        } else if (cmd === "PopTransform") {
          const prev = txStack.pop();
          if (prev) { otx = prev.tx; oty = prev.ty; }
          else { otx = 0; oty = 0; }
          ctx.restore();
        }
        continue;
      }

      if ("PushTransform" in cmd) {
        txStack.push({ tx: otx, ty: oty, sx: 1, sy: 1 });
        ctx.save();
        ctx.translate(cmd.PushTransform.translate.x, cmd.PushTransform.translate.y);
        ctx.scale(cmd.PushTransform.scale.x, cmd.PushTransform.scale.y);
        otx += cmd.PushTransform.translate.x;
        oty += cmd.PushTransform.translate.y;
      } else if ("SetClip" in cmd) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(cmd.SetClip.rect.x, cmd.SetClip.rect.y, cmd.SetClip.rect.w, cmd.SetClip.rect.h);
        ctx.clip();
      } else if ("DrawRect" in cmd) {
        // Draw labels only (rects already done by WebGPU)
        const { rect, label } = cmd.DrawRect;
        if (label && rect.w > 20) {
          ctx.fillStyle = this.tokenStr("TextPrimary");
          ctx.font = "11px sans-serif";
          ctx.textBaseline = "middle";
          ctx.save();
          ctx.beginPath();
          ctx.rect(rect.x + 2, rect.y, rect.w - 4, rect.h);
          ctx.clip();
          ctx.fillText(label, rect.x + 4, rect.y + rect.h / 2);
          ctx.restore();
        }
      } else if ("DrawText" in cmd) {
        const { position, text, color, font_size, align } = cmd.DrawText;
        ctx.fillStyle = this.tokenStr(color);
        ctx.font = `${font_size}px sans-serif`;
        ctx.textAlign = align === "Center" ? "center" : align === "Right" ? "right" : "left";
        ctx.textBaseline = "middle";
        ctx.fillText(text, position.x, position.y);
      } else if ("DrawLine" in cmd) {
        const { from, to, color, width: lineWidth } = cmd.DrawLine;
        ctx.strokeStyle = this.tokenStr(color);
        ctx.lineWidth = lineWidth;
        ctx.beginPath();
        ctx.moveTo(from.x, from.y);
        ctx.lineTo(to.x, to.y);
        ctx.stroke();
      }
    }
  }

  private resolveToken(token: ThemeToken): { r: number; g: number; b: number; a: number } {
    const c = resolveColor(this.theme, token);
    return { r: c.r, g: c.g, b: c.b, a: c.a };
  }

  private tokenStr(token: ThemeToken): string {
    const c = resolveColor(this.theme, token);
    return `rgba(${Math.round(c.r * 255)},${Math.round(c.g * 255)},${Math.round(c.b * 255)},${c.a})`;
  }

  private bgColor(): GPUColor {
    const bg = resolveColor(this.theme, "Background");
    return { r: bg.r, g: bg.g, b: bg.b, a: bg.a };
  }

  private ensureInstanceBuffer(count: number): void {
    if (count <= this.instanceCapacity) return;
    const newCap = Math.max(count, this.instanceCapacity * 2, 1024);
    this.instanceBuffer?.destroy();
    this.instanceBuffer = this.device.createBuffer({
      size: newCap * INSTANCE_BYTES,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    this.instanceCapacity = newCap;
  }
}
