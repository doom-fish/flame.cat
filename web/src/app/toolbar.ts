import type { ViewType } from "./lane-manager";

export interface ToolbarConfig {
  activeView: ViewType;
  profileName: string | null;
  onViewChange: (view: ViewType) => void;
  onSearch: () => void;
}

const TOOLBAR_HEIGHT = 36;

const VIEW_TABS: { id: ViewType; label: string; emoji: string }[] = [
  { id: "time-order", label: "Time Order", emoji: "🕰" },
  { id: "left-heavy", label: "Left Heavy", emoji: "⬅️" },
  { id: "sandwich", label: "Sandwich", emoji: "🥪" },
  { id: "ranked", label: "Ranked", emoji: "📊" },
];

/**
 * Creates the toolbar DOM element with view tabs and controls.
 * Renders as a fixed bar above the canvas.
 */
export function createToolbar(config: ToolbarConfig): HTMLElement {
  const toolbar = document.createElement("div");
  toolbar.id = "toolbar";
  toolbar.style.cssText = `
    height: ${TOOLBAR_HEIGHT}px;
    display: flex;
    align-items: center;
    font-family: 'SF Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
    font-size: 13px;
    user-select: none;
    flex-shrink: 0;
  `;

  // Left: view tabs
  const tabsContainer = document.createElement("div");
  tabsContainer.style.cssText = `display: flex; gap: 2px; padding-left: 4px;`;

  for (const tab of VIEW_TABS) {
    const btn = document.createElement("button");
    btn.dataset["view"] = tab.id;
    btn.textContent = `${tab.emoji} ${tab.label}`;
    btn.style.cssText = `
      border: none;
      padding: 4px 10px;
      cursor: pointer;
      font-family: inherit;
      font-size: 12px;
      border-radius: 3px;
      transition: background 0.15s ease;
    `;
    btn.addEventListener("click", () => config.onViewChange(tab.id));
    tabsContainer.appendChild(btn);
  }

  // Center: profile name
  const center = document.createElement("div");
  center.id = "toolbar-center";
  center.style.cssText = `flex: 1; text-align: center; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;`;
  center.textContent = config.profileName ?? "flame.cat";

  // Right: controls
  const right = document.createElement("div");
  right.style.cssText = `display: flex; gap: 4px; padding-right: 8px;`;

  const searchBtn = document.createElement("button");
  searchBtn.textContent = "🔍 Search";
  searchBtn.style.cssText = `
    border: none;
    padding: 4px 10px;
    cursor: pointer;
    font-family: inherit;
    font-size: 12px;
    border-radius: 3px;
  `;
  searchBtn.addEventListener("click", config.onSearch);
  right.appendChild(searchBtn);

  toolbar.appendChild(tabsContainer);
  toolbar.appendChild(center);
  toolbar.appendChild(right);

  return toolbar;
}

/**
 * Updates toolbar styling based on the active theme.
 */
export function applyToolbarTheme(
  toolbar: HTMLElement,
  colors: {
    bg: string;
    text: string;
    tabActive: string;
    tabHover: string;
  },
  activeView: ViewType,
): void {
  toolbar.style.background = colors.bg;
  toolbar.style.color = colors.text;

  const buttons = toolbar.querySelectorAll("button");
  for (const btn of buttons) {
    const view = btn.dataset["view"] as ViewType | undefined;
    const isActive = view === activeView;
    btn.style.background = isActive ? colors.tabActive : "transparent";
    btn.style.color = isActive ? "#fff" : colors.text;
    if (!isActive) {
      btn.onmouseenter = () => {
        btn.style.background = colors.tabHover;
      };
      btn.onmouseleave = () => {
        btn.style.background = "transparent";
      };
    } else {
      btn.onmouseenter = null;
      btn.onmouseleave = null;
    }
  }
}

export { TOOLBAR_HEIGHT };
