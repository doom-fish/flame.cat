/**
 * Search bar — overlaid at the top of the canvas for filtering frames.
 * Activated with Ctrl+F, dismissed with Escape.
 */
export class SearchBar {
  private el: HTMLElement;
  private input: HTMLInputElement;
  private countEl: HTMLElement;
  private visible = false;
  private onQueryChange: (query: string) => void;
  private onClose: () => void;

  constructor(container: HTMLElement, onQueryChange: (query: string) => void, onClose: () => void) {
    this.onQueryChange = onQueryChange;
    this.onClose = onClose;

    this.el = document.createElement("div");
    this.el.id = "search-bar";
    this.el.style.cssText = `
      position: absolute;
      top: 40px;
      right: 8px;
      display: none;
      align-items: center;
      gap: 8px;
      padding: 4px 8px;
      border-radius: 4px;
      z-index: 50;
      font-family: 'SF Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
      font-size: 13px;
    `;

    this.input = document.createElement("input");
    this.input.type = "text";
    this.input.placeholder = "Search frames…";
    this.input.style.cssText = `
      border: none;
      outline: none;
      font-family: inherit;
      font-size: inherit;
      padding: 4px 8px;
      border-radius: 3px;
      width: 200px;
    `;

    this.input.addEventListener("input", () => {
      this.onQueryChange(this.input.value);
    });

    this.input.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        this.hide();
        this.onClose();
      }
    });

    this.countEl = document.createElement("span");
    this.countEl.style.cssText = `opacity: 0.6; white-space: nowrap;`;

    const closeBtn = document.createElement("button");
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = `
      border: none;
      background: transparent;
      cursor: pointer;
      font-size: 14px;
      padding: 2px 4px;
    `;
    closeBtn.addEventListener("click", () => {
      this.hide();
      this.onClose();
    });

    this.el.appendChild(this.input);
    this.el.appendChild(this.countEl);
    this.el.appendChild(closeBtn);
    container.appendChild(this.el);
  }

  show(): void {
    this.visible = true;
    this.el.style.display = "flex";
    this.input.focus();
    this.input.select();
  }

  hide(): void {
    this.visible = false;
    this.el.style.display = "none";
    this.input.value = "";
  }

  get isVisible(): boolean {
    return this.visible;
  }

  get query(): string {
    return this.input.value;
  }

  setMatchCount(count: number, total: number): void {
    this.countEl.textContent = count > 0 ? `${count}/${total}` : "No matches";
  }

  applyTheme(colors: { bg: string; text: string; border: string; inputBg: string }): void {
    this.el.style.background = colors.bg;
    this.el.style.color = colors.text;
    this.el.style.border = `1px solid ${colors.border}`;
    this.input.style.background = colors.inputBg;
    this.input.style.color = colors.text;
  }
}
