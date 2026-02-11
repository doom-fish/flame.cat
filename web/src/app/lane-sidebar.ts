import type { LaneConfig } from "./lane-manager";

export interface ProfileInfo {
  index: number;
  label: string;
  offset_us: number;
}

export interface LaneSidebarCallbacks {
  onToggle: (laneId: string, visible: boolean) => void;
  onReorder: (fromIndex: number, toIndex: number) => void;
  onOffsetChange?: (profileIndex: number, offsetUs: number) => void;
}

/**
 * A slide-out sidebar panel for selecting which lanes (threads/domains)
 * to show and reordering them via drag-and-drop.
 */
export class LaneSidebar {
  private el: HTMLElement;
  private profilesEl: HTMLElement;
  private listEl: HTMLElement;
  private callbacks: LaneSidebarCallbacks;
  private visible = false;
  private dragFromIndex: number | null = null;

  constructor(parent: HTMLElement, callbacks: LaneSidebarCallbacks) {
    this.callbacks = callbacks;

    this.el = document.createElement("div");
    this.el.style.cssText = `
      position: absolute;
      top: 0;
      left: 0;
      width: 260px;
      height: 100%;
      z-index: 100;
      display: none;
      flex-direction: column;
      box-sizing: border-box;
      overflow: hidden;
      box-shadow: 2px 0 8px rgba(0,0,0,0.3);
    `;

    const header = document.createElement("div");
    header.style.cssText = `
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 8px 12px;
      font-weight: 600;
      font-size: 13px;
      border-bottom: 1px solid;
      flex-shrink: 0;
    `;
    header.textContent = "Lanes";

    const closeBtn = document.createElement("button");
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = `
      background: none;
      border: none;
      cursor: pointer;
      font-size: 16px;
      padding: 2px 6px;
      line-height: 1;
    `;
    closeBtn.addEventListener("click", () => this.hide());
    header.appendChild(closeBtn);

    this.profilesEl = document.createElement("div");
    this.profilesEl.style.cssText = `
      flex-shrink: 0;
      padding: 0;
      display: none;
    `;

    this.listEl = document.createElement("div");
    this.listEl.style.cssText = `
      flex: 1;
      overflow-y: auto;
      padding: 4px 0;
    `;

    this.el.appendChild(header);
    this.el.appendChild(this.profilesEl);
    this.el.appendChild(this.listEl);
    parent.appendChild(this.el);
  }

  /** Update the profiles alignment section. Only shown when multiple profiles are loaded. */
  updateProfiles(profiles: ProfileInfo[]): void {
    this.profilesEl.innerHTML = "";
    if (profiles.length <= 1) {
      this.profilesEl.style.display = "none";
      return;
    }
    this.profilesEl.style.display = "block";

    const sectionHeader = document.createElement("div");
    sectionHeader.style.cssText = `
      padding: 6px 12px 4px;
      font-size: 11px;
      font-weight: 600;
      opacity: 0.6;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    `;
    sectionHeader.textContent = "Profile Alignment";
    this.profilesEl.appendChild(sectionHeader);

    for (const profile of profiles) {
      const row = document.createElement("div");
      row.style.cssText = `
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 4px 12px;
        font-size: 11px;
      `;

      const label = document.createElement("span");
      label.textContent = profile.label;
      label.style.cssText = `
        flex: 1;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      `;

      const offsetInput = document.createElement("input");
      offsetInput.type = "number";
      offsetInput.value = String(Math.round(profile.offset_us));
      offsetInput.title = "Offset in µs";
      offsetInput.style.cssText = `
        width: 80px;
        font-size: 11px;
        padding: 2px 4px;
        text-align: right;
        border: 1px solid;
        border-radius: 3px;
        background: transparent;
        color: inherit;
      `;
      offsetInput.addEventListener("change", () => {
        const val = parseFloat(offsetInput.value);
        if (!isNaN(val) && this.callbacks.onOffsetChange) {
          this.callbacks.onOffsetChange(profile.index, val);
        }
      });

      const unit = document.createElement("span");
      unit.textContent = "µs";
      unit.style.opacity = "0.5";

      row.appendChild(label);
      row.appendChild(offsetInput);
      row.appendChild(unit);
      this.profilesEl.appendChild(row);
    }

    const divider = document.createElement("div");
    divider.style.cssText = `
      margin: 4px 12px;
      border-bottom: 1px solid;
      opacity: 0.2;
    `;
    this.profilesEl.appendChild(divider);
  }

  /** Update the sidebar with the current lane list. */
  update(lanes: LaneConfig[]): void {
    this.listEl.innerHTML = "";
    for (let i = 0; i < lanes.length; i++) {
      const lane = lanes[i];
      if (!lane) continue;
      const row = this.createRow(lane, i);
      this.listEl.appendChild(row);
    }
  }

  private createRow(lane: LaneConfig, index: number): HTMLElement {
    const row = document.createElement("div");
    row.draggable = true;
    row.dataset.index = String(index);
    row.style.cssText = `
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 6px 12px;
      cursor: grab;
      font-size: 12px;
      user-select: none;
      transition: background 0.1s;
    `;

    // Drag handle
    const handle = document.createElement("span");
    handle.textContent = "≡";
    handle.style.cssText = `
      cursor: grab;
      font-size: 16px;
      opacity: 0.5;
      flex-shrink: 0;
    `;

    // Visibility checkbox
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = lane.visible;
    checkbox.style.cssText = "margin: 0; flex-shrink: 0; cursor: pointer;";
    checkbox.addEventListener("change", () => {
      this.callbacks.onToggle(lane.id, checkbox.checked);
    });

    // Label
    const label = document.createElement("span");
    label.textContent = lane.threadName ?? lane.id;
    label.style.cssText = `
      flex: 1;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    `;
    if (!lane.visible) label.style.opacity = "0.5";

    row.appendChild(handle);
    row.appendChild(checkbox);
    row.appendChild(label);

    // Drag-and-drop events
    row.addEventListener("dragstart", (e) => {
      this.dragFromIndex = index;
      row.style.opacity = "0.4";
      e.dataTransfer?.setData("text/plain", String(index));
    });
    row.addEventListener("dragend", () => {
      row.style.opacity = "1";
      this.dragFromIndex = null;
      this.clearDropIndicators();
    });
    row.addEventListener("dragover", (e) => {
      e.preventDefault();
      this.clearDropIndicators();
      if (this.dragFromIndex !== null && this.dragFromIndex !== index) {
        row.style.borderTop = this.dragFromIndex > index ? "2px solid #4af" : "";
        row.style.borderBottom = this.dragFromIndex < index ? "2px solid #4af" : "";
      }
    });
    row.addEventListener("dragleave", () => {
      row.style.borderTop = "";
      row.style.borderBottom = "";
    });
    row.addEventListener("drop", (e) => {
      e.preventDefault();
      this.clearDropIndicators();
      if (this.dragFromIndex !== null && this.dragFromIndex !== index) {
        this.callbacks.onReorder(this.dragFromIndex, index);
        this.dragFromIndex = null;
      }
    });

    return row;
  }

  private clearDropIndicators(): void {
    for (const child of this.listEl.children) {
      (child as HTMLElement).style.borderTop = "";
      (child as HTMLElement).style.borderBottom = "";
    }
  }

  toggle(): void {
    if (this.visible) {
      this.hide();
    } else {
      this.show();
    }
  }

  show(): void {
    this.visible = true;
    this.el.style.display = "flex";
  }

  hide(): void {
    this.visible = false;
    this.el.style.display = "none";
  }

  get isVisible(): boolean {
    return this.visible;
  }

  applyTheme(colors: { bg: string; text: string; border: string }): void {
    this.el.style.backgroundColor = colors.bg;
    this.el.style.color = colors.text;
    this.el.style.borderColor = colors.border;
    const header = this.el.firstElementChild as HTMLElement;
    if (header) header.style.borderColor = colors.border;
    // Style close button
    const closeBtn = header?.querySelector("button");
    if (closeBtn) closeBtn.style.color = colors.text;
  }
}
