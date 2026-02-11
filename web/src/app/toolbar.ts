import type { ViewType } from "./lane-manager";

export interface ToolbarConfig {
  activeView: ViewType;
  profileName: string | null;
  onViewChange: (view: ViewType) => void;
  onSearch: () => void;
  onOpenFile: () => void;
}

const TOOLBAR_HEIGHT = 36;

const VIEW_TABS: { id: ViewType; label: string; shortLabel: string; emoji: string }[] = [
  { id: "time-order", label: "Time Order", shortLabel: "Time", emoji: "🕰" },
  { id: "left-heavy", label: "Left Heavy", shortLabel: "Heavy", emoji: "⬅️" },
  { id: "sandwich", label: "Sandwich", shortLabel: "Sand.", emoji: "🥪" },
  { id: "ranked", label: "Ranked", shortLabel: "Rank", emoji: "📊" },
];

/**
 * Creates the toolbar DOM element with view tabs and controls.
 * Adapts layout for narrow screens.
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
    touch-action: manipulation;
  `;

  // Left: view tabs (horizontally scrollable on narrow screens)
  const tabsContainer = document.createElement("div");
  tabsContainer.style.cssText = `
    display: flex;
    gap: 2px;
    padding-left: 4px;
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
    scrollbar-width: none;
    flex-shrink: 0;
  `;

  for (const tab of VIEW_TABS) {
    const btn = document.createElement("button");
    btn.dataset["view"] = tab.id;
    btn.dataset["fullLabel"] = `${tab.emoji} ${tab.label}`;
    btn.dataset["shortLabel"] = `${tab.emoji} ${tab.shortLabel}`;
    btn.textContent = `${tab.emoji} ${tab.label}`;
    btn.style.cssText = `
      border: none;
      padding: 4px 10px;
      cursor: pointer;
      font-family: inherit;
      font-size: 12px;
      border-radius: 3px;
      transition: background 0.15s ease;
      white-space: nowrap;
      flex-shrink: 0;
      -webkit-tap-highlight-color: transparent;
    `;
    btn.addEventListener("click", () => config.onViewChange(tab.id));
    tabsContainer.appendChild(btn);
  }

  // Center: profile name
  const center = document.createElement("div");
  center.id = "toolbar-center";
  center.style.cssText = `
    flex: 1;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    padding: 0 4px;
  `;
  center.textContent = config.profileName ?? "flame.cat";

  // Right: controls
  const right = document.createElement("div");
  right.style.cssText = `display: flex; gap: 4px; padding-right: 8px; flex-shrink: 0;`;

  // Open file button (essential for mobile — no drag-and-drop)
  const openBtn = document.createElement("button");
  openBtn.textContent = "📂";
  openBtn.title = "Open file";
  openBtn.style.cssText = `
    border: none;
    padding: 4px 8px;
    cursor: pointer;
    font-family: inherit;
    font-size: 14px;
    border-radius: 3px;
    -webkit-tap-highlight-color: transparent;
  `;
  openBtn.addEventListener("click", config.onOpenFile);
  right.appendChild(openBtn);

  const searchBtn = document.createElement("button");
  searchBtn.textContent = "🔍";
  searchBtn.title = "Search frames";
  searchBtn.style.cssText = `
    border: none;
    padding: 4px 8px;
    cursor: pointer;
    font-family: inherit;
    font-size: 14px;
    border-radius: 3px;
    -webkit-tap-highlight-color: transparent;
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
 * Also adapts tab labels for the current viewport width.
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

  const useShort = window.innerWidth < 480;

  // Hide center text on very narrow screens
  const center = toolbar.querySelector("#toolbar-center") as HTMLElement | null;
  if (center) {
    center.style.display = useShort ? "none" : "";
  }

  const buttons = toolbar.querySelectorAll("button");
  for (const btn of buttons) {
    const view = btn.dataset["view"] as ViewType | undefined;
    if (!view) {
      // Non-tab buttons (open, search)
      btn.style.background = "transparent";
      btn.style.color = colors.text;
      continue;
    }
    const isActive = view === activeView;
    btn.textContent = useShort
      ? (btn.dataset["shortLabel"] ?? "")
      : (btn.dataset["fullLabel"] ?? "");
    btn.style.padding = useShort ? "4px 6px" : "4px 10px";
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
