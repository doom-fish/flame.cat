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
      font-size: 11px;
      padding: 6px 10px;
      border-radius: 4px;
      max-width: 420px;
      z-index: 100;
      display: none;
      line-height: 1.5;
      box-shadow: 0 2px 8px rgba(0,0,0,0.2);
    `;

    this.nameEl = document.createElement("div");
    this.nameEl.style.cssText = `
      font-weight: 600;
      font-size: 12px;
      margin-bottom: 2px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    `;

    this.detailEl = document.createElement("div");
    this.detailEl.style.cssText = `
      opacity: 0.85;
      white-space: pre-line;
    `;

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
