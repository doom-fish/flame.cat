import type { RenderCommand, ThemeToken } from "../../protocol";
import type { Theme, Color } from "../../themes";
import { resolveColor } from "../../themes";

const SVG_NS = "http://www.w3.org/2000/svg";

/**
 * SVG renderer — produces an SVG DOM element from render commands.
 * Useful for static exports and embedding.
 */
export class SvgRenderer {
  private theme: Theme;

  constructor(theme: Theme) {
    this.theme = theme;
  }

  setTheme(theme: Theme): void {
    this.theme = theme;
  }

  /** Render commands into an SVG element. */
  render(commands: RenderCommand[], width: number, height: number): SVGSVGElement {
    const svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("width", String(width));
    svg.setAttribute("height", String(height));
    svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
    svg.setAttribute("xmlns", SVG_NS);

    const bg = this.resolve("Background");
    const bgRect = document.createElementNS(SVG_NS, "rect");
    bgRect.setAttribute("width", "100%");
    bgRect.setAttribute("height", "100%");
    bgRect.setAttribute("fill", this.colorStr(bg));
    svg.appendChild(bgRect);

    let currentGroup: SVGGElement = document.createElementNS(SVG_NS, "g");
    svg.appendChild(currentGroup);

    for (const cmd of commands) {
      if (typeof cmd === "string") continue;

      if ("DrawRect" in cmd) {
        const { rect, color, border_color, label } = cmd.DrawRect;
        const g = document.createElementNS(SVG_NS, "g");

        const r = document.createElementNS(SVG_NS, "rect");
        r.setAttribute("x", String(rect.x));
        r.setAttribute("y", String(rect.y));
        r.setAttribute("width", String(rect.w));
        r.setAttribute("height", String(rect.h));
        r.setAttribute("fill", this.tokenStr(color));
        if (border_color) {
          r.setAttribute("stroke", this.tokenStr(border_color));
          r.setAttribute("stroke-width", "0.5");
        }
        g.appendChild(r);

        if (label && rect.w > 20) {
          const text = document.createElementNS(SVG_NS, "text");
          text.setAttribute("x", String(rect.x + 4));
          text.setAttribute("y", String(rect.y + rect.h / 2 + 4));
          text.setAttribute("fill", this.tokenStr("TextPrimary"));
          text.setAttribute("font-size", "11");
          text.setAttribute("font-family", "sans-serif");

          const clipId = `clip-${rect.x}-${rect.y}`;
          const clipPath = document.createElementNS(SVG_NS, "clipPath");
          clipPath.setAttribute("id", clipId);
          const clipRect = document.createElementNS(SVG_NS, "rect");
          clipRect.setAttribute("x", String(rect.x));
          clipRect.setAttribute("y", String(rect.y));
          clipRect.setAttribute("width", String(rect.w));
          clipRect.setAttribute("height", String(rect.h));
          clipPath.appendChild(clipRect);
          svg.appendChild(clipPath);
          text.setAttribute("clip-path", `url(#${clipId})`);

          text.textContent = label;
          g.appendChild(text);
        }

        currentGroup.appendChild(g);
      } else if ("DrawText" in cmd) {
        const { position, text: textStr, color, font_size, align } = cmd.DrawText;
        const text = document.createElementNS(SVG_NS, "text");
        text.setAttribute("x", String(position.x));
        text.setAttribute("y", String(position.y));
        text.setAttribute("fill", this.tokenStr(color));
        text.setAttribute("font-size", String(font_size));
        text.setAttribute("font-family", "sans-serif");
        if (align === "Center") text.setAttribute("text-anchor", "middle");
        else if (align === "Right") text.setAttribute("text-anchor", "end");
        text.textContent = textStr;
        currentGroup.appendChild(text);
      } else if ("DrawLine" in cmd) {
        const { from, to, color, width: lineWidth } = cmd.DrawLine;
        const line = document.createElementNS(SVG_NS, "line");
        line.setAttribute("x1", String(from.x));
        line.setAttribute("y1", String(from.y));
        line.setAttribute("x2", String(to.x));
        line.setAttribute("y2", String(to.y));
        line.setAttribute("stroke", this.tokenStr(color));
        line.setAttribute("stroke-width", String(lineWidth));
        currentGroup.appendChild(line);
      } else if ("BeginGroup" in cmd) {
        const g = document.createElementNS(SVG_NS, "g");
        if (cmd.BeginGroup.id) g.setAttribute("id", cmd.BeginGroup.id);
        currentGroup.appendChild(g);
        currentGroup = g;
      }
    }

    return svg;
  }

  /** Render commands to an SVG string for export. */
  renderToString(commands: RenderCommand[], width: number, height: number): string {
    const svg = this.render(commands, width, height);
    return new XMLSerializer().serializeToString(svg);
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
