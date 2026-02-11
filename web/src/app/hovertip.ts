/**
 * Hovertip — a tooltip that follows the cursor and displays frame info.
 * Positioned near the cursor, avoiding overflow beyond the container.
 */
export class Hovertip {
  private el: HTMLElement;
  private nameEl: HTMLElement;
  private detailEl: HTMLElement;

  constructor(container: HTMLElement) {
    this.el = document.createElement("div");
    this.el.style.cssText = `
      position: absolute;
      pointer-events: none;
      user-select: none;
      font-family: 'SF Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
      font-size: 12px;
      padding: 4px 8px;
      border-radius: 3px;
      max-width: 400px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      z-index: 100;
      display: none;
    `;

    this.nameEl = document.createElement("div");
    this.nameEl.style.fontWeight = "bold";

    this.detailEl = document.createElement("div");
    this.detailEl.style.opacity = "0.8";

    this.el.appendChild(this.nameEl);
    this.el.appendChild(this.detailEl);
    container.appendChild(this.el);
  }

  show(
    x: number,
    y: number,
    containerWidth: number,
    containerHeight: number,
    name: string,
    detail: string,
  ): void {
    this.nameEl.textContent = name;
    this.detailEl.textContent = detail;
    this.el.style.display = "block";

    const OFFSET = 12;

    // Position to the right of cursor by default
    let left = x + OFFSET;
    const rect = this.el.getBoundingClientRect();
    if (left + rect.width > containerWidth - 4) {
      left = containerWidth - rect.width - 4;
      if (left < 4) left = 4;
    }

    let top = y + OFFSET;
    if (top + rect.height > containerHeight - 4) {
      top = y - rect.height - 4;
      if (top < 4) top = 4;
    }

    this.el.style.left = `${left}px`;
    this.el.style.top = `${top}px`;
  }

  hide(): void {
    this.el.style.display = "none";
  }

  applyTheme(colors: { bg: string; text: string; border: string }): void {
    this.el.style.background = colors.bg;
    this.el.style.color = colors.text;
    this.el.style.border = `1px solid ${colors.border}`;
  }
}
