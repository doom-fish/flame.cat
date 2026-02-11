import type { RenderCommand, ThemeToken } from "../../protocol";
import type { Theme, Color } from "../../themes";
import { resolveColor } from "../../themes";

/**
 * WebGL renderer — middle ground between Canvas2D (compatibility) and
 * WebGPU (performance). Uses instanced rendering for rectangles.
 */
export class WebGLRenderer {
  private gl: WebGLRenderingContext;
  private canvas: HTMLCanvasElement;
  private theme: Theme;
  private program: WebGLProgram | null = null;

  constructor(canvas: HTMLCanvasElement, theme: Theme) {
    this.canvas = canvas;
    const gl = canvas.getContext("webgl");
    if (!gl) throw new Error("WebGL not supported");
    this.gl = gl;
    this.theme = theme;
  }

  init(): void {
    const gl = this.gl;

    const vsSource = `
      attribute vec2 a_position;
      attribute vec2 a_instancePos;
      attribute vec2 a_instanceSize;
      attribute vec4 a_instanceColor;
      uniform vec2 u_viewport;
      uniform vec2 u_scroll;
      varying vec4 v_color;
      void main() {
        vec2 px = (a_instancePos + a_position * a_instanceSize - u_scroll);
        vec2 ndc = px / u_viewport * 2.0 - 1.0;
        gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
        v_color = a_instanceColor;
      }
    `;

    const fsSource = `
      precision mediump float;
      varying vec4 v_color;
      void main() {
        gl_FragColor = v_color;
      }
    `;

    const vs = this.compileShader(gl.VERTEX_SHADER, vsSource);
    const fs = this.compileShader(gl.FRAGMENT_SHADER, fsSource);
    if (!vs || !fs) return;

    const program = gl.createProgram();
    if (!program) return;
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);

    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.error("Shader link error:", gl.getProgramInfoLog(program));
      return;
    }

    this.program = program;
  }

  setTheme(theme: Theme): void {
    this.theme = theme;
  }

  render(commands: RenderCommand[], scrollX: number, scrollY: number): void {
    const gl = this.gl;
    if (!this.program) return;

    const dpr = window.devicePixelRatio;
    const width = this.canvas.clientWidth;
    const height = this.canvas.clientHeight;
    this.canvas.width = Math.round(width * dpr);
    this.canvas.height = Math.round(height * dpr);

    gl.viewport(0, 0, this.canvas.width, this.canvas.height);

    const bg = this.resolve("Background");
    gl.clearColor(bg.r, bg.g, bg.b, bg.a);
    gl.clear(gl.COLOR_BUFFER_BIT);

    gl.useProgram(this.program);

    // Set uniforms
    const uViewport = gl.getUniformLocation(this.program, "u_viewport");
    const uScroll = gl.getUniformLocation(this.program, "u_scroll");
    gl.uniform2f(uViewport, width, height);
    gl.uniform2f(uScroll, scrollX, scrollY);

    // WebGL1 doesn't support instanced rendering natively (requires extension),
    // so we draw each rect as an individual quad for maximum compatibility.
    const rects = this.collectRects(commands);
    for (const r of rects) {
      this.drawQuad(r.x, r.y, r.w, r.h, r.r, r.g, r.b, r.a);
    }
  }

  private drawQuad(
    x: number,
    y: number,
    w: number,
    h: number,
    r: number,
    g: number,
    b: number,
    a: number,
  ): void {
    const gl = this.gl;
    if (!this.program) return;

    // prettier-ignore
    const vertices = new Float32Array([
      x, y,       x + w, y,       x, y + h,
      x, y + h,   x + w, y,       x + w, y + h,
    ]);

    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.DYNAMIC_DRAW);

    const posLoc = gl.getAttribLocation(this.program, "a_position");
    gl.enableVertexAttribArray(posLoc);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

    // Pass color as a constant attribute (WebGL1 workaround)
    const colorLoc = gl.getAttribLocation(this.program, "a_instanceColor");
    gl.disableVertexAttribArray(colorLoc);
    gl.vertexAttrib4f(colorLoc, r, g, b, a);

    // Instance pos/size as constants too
    const posAttr = gl.getAttribLocation(this.program, "a_instancePos");
    gl.disableVertexAttribArray(posAttr);
    gl.vertexAttrib2f(posAttr, 0, 0);

    const sizeAttr = gl.getAttribLocation(this.program, "a_instanceSize");
    gl.disableVertexAttribArray(sizeAttr);
    gl.vertexAttrib2f(sizeAttr, 1, 1);

    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.deleteBuffer(buf);
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
        const c = this.resolve(color);
        result.push({ x: rect.x, y: rect.y, w: rect.w, h: rect.h, r: c.r, g: c.g, b: c.b, a: c.a });
      }
    }
    return result;
  }

  private resolve(token: ThemeToken): Color {
    return resolveColor(this.theme, token);
  }

  private compileShader(type: number, source: string): WebGLShader | null {
    const gl = this.gl;
    const shader = gl.createShader(type);
    if (!shader) return null;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      console.error("Shader compile error:", gl.getShaderInfoLog(shader));
      gl.deleteShader(shader);
      return null;
    }
    return shader;
  }
}
