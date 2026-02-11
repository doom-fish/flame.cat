/**
 * Detail panel — shows timing info for a selected frame.
 * Displayed at the bottom of the view.
 */
export interface FrameDetail {
  name: string;
  selfTime: number;
  totalTime: number;
  depth: number;
  category: string | null;
}

const DETAIL_HEIGHT = 48;

export class DetailPanel {
  private el: HTMLElement;
  private contentEl: HTMLElement;
  private visible = false;

  constructor(container: HTMLElement) {
    this.el = document.createElement("div");
    this.el.id = "detail-panel";
    this.el.style.cssText = `
      height: ${DETAIL_HEIGHT}px;
      display: none;
      align-items: center;
      font-family: 'SF Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
      font-size: 12px;
      padding: 0 12px;
      flex-shrink: 0;
      gap: 24px;
      border-top: 1px solid transparent;
    `;

    this.contentEl = document.createElement("div");
    this.contentEl.style.cssText = `display: flex; gap: 24px; align-items: center; width: 100%;`;
    this.el.appendChild(this.contentEl);
    container.appendChild(this.el);
  }

  show(detail: FrameDetail, profileDuration: number): void {
    this.visible = true;
    this.el.style.display = "flex";

    const selfPct = ((detail.selfTime / profileDuration) * 100).toFixed(1);
    const totalPct = ((detail.totalTime / profileDuration) * 100).toFixed(1);

    this.contentEl.innerHTML = `
      <span style="font-weight: bold; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 40%;">${escapeHtml(detail.name)}</span>
      <span>Self: ${formatTime(detail.selfTime)} (${selfPct}%)</span>
      <span>Total: ${formatTime(detail.totalTime)} (${totalPct}%)</span>
      <span>Depth: ${detail.depth}</span>
      ${detail.category ? `<span style="opacity: 0.6">${escapeHtml(detail.category)}</span>` : ""}
    `;
  }

  hide(): void {
    this.visible = false;
    this.el.style.display = "none";
  }

  get isVisible(): boolean {
    return this.visible;
  }

  get height(): number {
    return this.visible ? DETAIL_HEIGHT : 0;
  }

  applyTheme(colors: { bg: string; text: string; border: string }): void {
    this.el.style.background = colors.bg;
    this.el.style.color = colors.text;
    this.el.style.borderTopColor = colors.border;
  }
}

function formatTime(us: number): string {
  if (us >= 1_000_000) return `${(us / 1_000_000).toFixed(2)}s`;
  if (us >= 1_000) return `${(us / 1_000).toFixed(1)}ms`;
  return `${us.toFixed(0)}µs`;
}

function escapeHtml(str: string): string {
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

export { DETAIL_HEIGHT };
