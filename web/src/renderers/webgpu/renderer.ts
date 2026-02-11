import type { RenderCommand, ThemeToken } from "../../protocol";
import type { Theme } from "../../themes";
import { resolveColor } from "../../themes";
import shaderSource from "./rect.wgsl?raw";

/** Per-instance data for one rectangle: x, y, w, h, r, g, b, a */
const INSTANCE_FLOATS = 8;
const INSTANCE_BYTES = INSTANCE_FLOATS * 4;

export class WebGPURenderer {
  private device!: GPUDevice;
  private context!: GPUCanvasContext;
  private pipeline!: GPURenderPipeline;
  private uniformBuffer!: GPUBuffer;
  private uniformBindGroup!: GPUBindGroup;
  private quadVertexBuffer!: GPUBuffer;
  private instanceBuffer!: GPUBuffer;
  private instanceCapacity = 0;
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
    this.context = context;

    const format = navigator.gpu.getPreferredCanvasFormat();
    this.context.configure({ device: this.device, format, alphaMode: "premultiplied" });

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
            // Quad vertex buffer (per-vertex)
            arrayStride: 8,
            stepMode: "vertex",
            attributes: [{ shaderLocation: 0, offset: 0, format: "float32x2" }],
          },
          {
            // Instance buffer (per-instance)
            arrayStride: INSTANCE_BYTES,
            stepMode: "instance",
            attributes: [
              { shaderLocation: 1, offset: 0, format: "float32x2" }, // pos
              { shaderLocation: 2, offset: 8, format: "float32x2" }, // size
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
    this.canvas.width = Math.round(width * dpr);
    this.canvas.height = Math.round(height * dpr);

    // Update uniforms
    const uniforms = new Float32Array([width, height, scrollX, scrollY, 1, 1, dpr, 0]);
    this.device.queue.writeBuffer(this.uniformBuffer, 0, uniforms);

    // Collect DrawRect instances
    const rects = this.collectRects(commands);
    if (rects.length === 0) return;

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
          view: this.context.getCurrentTexture().createView(),
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

  private collectRects(
    commands: RenderCommand[],
  ): { x: number; y: number; w: number; h: number; r: number; g: number; b: number; a: number }[] {
    const result: {
      x: number;
      y: number;
      w: number;
      h: number;
      r: number;
      g: number;
      b: number;
      a: number;
    }[] = [];

    for (const cmd of commands) {
      if (typeof cmd === "string") continue;
      if ("DrawRect" in cmd) {
        const { rect, color } = cmd.DrawRect;
        const c = this.resolveToken(color);
        result.push({ x: rect.x, y: rect.y, w: rect.w, h: rect.h, ...c });
      }
    }
    return result;
  }

  private resolveToken(token: ThemeToken): { r: number; g: number; b: number; a: number } {
    const c = resolveColor(this.theme, token);
    return { r: c.r, g: c.g, b: c.b, a: c.a };
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
