import type { RenderCommand, ThemeToken } from "../../protocol";
import type { Theme, Color } from "../../themes";
import { resolveColor } from "../../themes";

/**
 * Canvas2D renderer — fallback for browsers without WebGPU.
 * Consumes the same RenderCommand[] as the WebGPU renderer.
 */
export class CanvasRenderer {
  private ctx: CanvasRenderingContext2D;
  private canvas: HTMLCanvasElement;
  private theme: Theme;
  private transformStack: { tx: number; ty: number; sx: number; sy: number }[] = [];

  constructor(canvas: HTMLCanvasElement, theme: Theme) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("Failed to get 2d context");
    this.ctx = ctx;
    this.theme = theme;
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

    const ctx = this.ctx;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.translate(-scrollX, -scrollY);

    // Clear
    const bg = this.resolve("Background");
    ctx.fillStyle = this.colorStr(bg);
    ctx.fillRect(scrollX, scrollY, width, height);

    this.transformStack = [];

    for (const cmd of commands) {
      if (typeof cmd === "string") {
        switch (cmd) {
          case "ClearClip":
            ctx.restore();
            break;
          case "PopTransform":
            ctx.restore();
            this.transformStack.pop();
            break;
          case "EndGroup":
            break;
        }
        continue;
      }

      if ("DrawRect" in cmd) {
        const { rect, color, border_color, label } = cmd.DrawRect;
        const x = Math.round(rect.x * dpr) / dpr;
        const y = Math.round(rect.y * dpr) / dpr;
        const w = Math.round(rect.w * dpr) / dpr;
        const h = Math.round(rect.h * dpr) / dpr;

        ctx.fillStyle = this.tokenStr(color);
        ctx.fillRect(x, y, w, h);

        if (border_color) {
          ctx.strokeStyle = this.tokenStr(border_color);
          ctx.lineWidth = 1 / dpr;
          ctx.strokeRect(x, y, w, h);
        }

        if (label && w > 20) {
          ctx.fillStyle = this.tokenStr("TextPrimary");
          ctx.font = "11px sans-serif";
          ctx.textBaseline = "middle";
          ctx.save();
          ctx.beginPath();
          ctx.rect(x + 2, y, w - 4, h);
          ctx.clip();
          ctx.fillText(label, x + 4, y + h / 2);
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
      } else if ("SetClip" in cmd) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(cmd.SetClip.rect.x, cmd.SetClip.rect.y, cmd.SetClip.rect.w, cmd.SetClip.rect.h);
        ctx.clip();
      } else if ("PushTransform" in cmd) {
        ctx.save();
        const { translate, scale } = cmd.PushTransform;
        ctx.translate(translate.x, translate.y);
        ctx.scale(scale.x, scale.y);
        this.transformStack.push({ tx: translate.x, ty: translate.y, sx: scale.x, sy: scale.y });
      } else if ("BeginGroup" in cmd) {
        // No-op for Canvas2D
      }
    }
  }

  private resolve(token: ThemeToken): Color {
    return resolveColor(this.theme, token);
  }

  private tokenStr(token: ThemeToken): string {
    const c = this.resolve(token);
    return this.colorStr(c);
  }

  private colorStr(c: Color): string {
    return `rgba(${Math.round(c.r * 255)},${Math.round(c.g * 255)},${Math.round(c.b * 255)},${c.a})`;
  }
}
