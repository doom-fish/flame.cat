import { useRef } from "react";
import {
  FlameCatProvider,
  FlameCatViewer,
  useFlameGraph,
  useStatus,
  useProfile,
  useViewType,
  useColorMode,
  useLanes,
  useViewport,
  useSearch,
  useTheme,
  useSelectedSpan,
  useHoveredSpan,
  useNavigation,
  useExport,
  useHotkeys,
  type ViewType,
} from "@flame-cat/react";

const WASM_URL = "/wasm/flame-cat-ui.js";

export function App() {
  return (
    <FlameCatProvider
      wasmUrl={WASM_URL}
      onError={(err) => console.error("flame-cat init failed:", err)}
    >
      <div style={{ display: "flex", flexDirection: "column", height: "100vh", fontFamily: "system-ui, -apple-system, sans-serif" }}>
        <Toolbar />
        <div style={{ display: "flex", flex: 1, minHeight: 0 }}>
          <Sidebar />
          <div style={{ flex: 1, position: "relative" }}>
            <FlameCatViewer style={{ width: "100%", height: "100%" }} />
            <HoverTooltip />
          </div>
        </div>
        <DetailPanel />
        <StatusBar />
      </div>
    </FlameCatProvider>
  );
}

// ── Toolbar ────────────────────────────────────────────────────────────

const VIEW_TYPES: { value: ViewType; label: string; icon: string }[] = [
  { value: "time_order", label: "Time", icon: "⏱" },
  { value: "left_heavy", label: "Left Heavy", icon: "◀" },
  { value: "icicle", label: "Icicle", icon: "▼" },
  { value: "sandwich", label: "Sandwich", icon: "🥪" },
  { value: "ranked", label: "Ranked", icon: "📊" },
];

function Toolbar() {
  const { loadProfile, ready } = useFlameGraph();
  const { query, setQuery } = useSearch();
  const { mode, toggle } = useTheme();
  const { colorMode, toggle: toggleColor } = useColorMode();
  const { viewType, setViewType } = useViewType();
  const { canGoBack, canGoForward, back, forward } = useNavigation();
  const { resetZoom } = useViewport();
  const { exportJSON, exportSVG } = useExport();
  const searchRef = useRef<HTMLInputElement>(null);

  useHotkeys({}, searchRef);

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      loadProfile(await file.arrayBuffer());
    } catch (err) {
      console.error("Failed to load file:", err);
    }
  };

  const handleExportJSON = () => {
    const json = exportJSON();
    if (!json) return;
    downloadBlob(json, "profile.json", "application/json");
  };

  const handleExportSVG = () => {
    const svg = exportSVG();
    if (!svg) return;
    downloadBlob(svg, "flamegraph.svg", "image/svg+xml");
  };

  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 8,
      padding: "6px 12px",
      borderBottom: "1px solid var(--border)",
      background: mode === "dark" ? "#1a1a2e" : "#f8f9fa",
      color: mode === "dark" ? "#e0e0e0" : "#1a1a2e",
      fontSize: 13,
    }}>
      <span style={{ fontWeight: 700, fontSize: 15 }}>🔥 flame.cat</span>

      <label style={{
        padding: "4px 10px", borderRadius: 4, cursor: "pointer",
        background: mode === "dark" ? "#2a2a4a" : "#e9ecef",
      }}>
        📂 Open
        <input type="file" accept=".json,.cpuprofile,.speedscope" onChange={handleFile} style={{ display: "none" }} />
      </label>

      <Sep />

      {/* View type tabs */}
      <div style={{ display: "flex", gap: 2 }}>
        {VIEW_TYPES.map(({ value, label, icon }) => (
          <button
            key={value}
            onClick={() => setViewType(value)}
            disabled={!ready}
            style={{
              padding: "3px 8px", border: "none", borderRadius: 3, cursor: "pointer", fontSize: 12,
              background: viewType === value ? (mode === "dark" ? "#4a4a7a" : "#dee2e6") : "transparent",
              color: "inherit", opacity: ready ? 1 : 0.4,
            }}
          >
            {icon} {label}
          </button>
        ))}
      </div>

      <Sep />

      {/* Navigation */}
      <button onClick={back} disabled={!canGoBack} title="Back" style={navBtn(mode)}>←</button>
      <button onClick={forward} disabled={!canGoForward} title="Forward" style={navBtn(mode)}>→</button>
      <button onClick={resetZoom} title="Reset zoom (0/Home)" style={navBtn(mode)}>⊞</button>

      <div style={{ flex: 1 }} />

      {/* Search */}
      <input
        ref={searchRef}
        placeholder="🔍 Search… (/, Enter/Shift+Enter)"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        style={{
          padding: "3px 8px", borderRadius: 4, border: "1px solid",
          borderColor: mode === "dark" ? "#444" : "#ccc",
          background: mode === "dark" ? "#2a2a4a" : "#fff",
          color: "inherit", width: 180, fontSize: 12,
        }}
      />

      <button onClick={handleExportJSON} title="Export JSON" style={navBtn(mode)}>💾 JSON</button>
      <button onClick={handleExportSVG} title="Export SVG" style={navBtn(mode)}>🖼 SVG</button>
      <button onClick={toggleColor} title="Toggle color mode" style={navBtn(mode)}>
        {colorMode === "by_name" ? "🎨 Color" : "🔢 Value"}
      </button>
      <button onClick={toggle} title="Toggle theme (t)" style={navBtn(mode)}>
        {mode === "dark" ? "☀️ Light" : "🌙 Dark"}
      </button>
    </div>
  );
}

// ── Sidebar ────────────────────────────────────────────────────────────

function Sidebar() {
  const { lanes, toggleVisibility, showAll, hideAll } = useLanes();
  const { mode } = useTheme();
  const profile = useProfile();

  if (!profile) return null;

  return (
    <div style={{
      width: 200, borderRight: "1px solid",
      borderColor: mode === "dark" ? "#333" : "#ddd",
      background: mode === "dark" ? "#16162a" : "#f1f3f5",
      color: mode === "dark" ? "#ccc" : "#333",
      display: "flex", flexDirection: "column",
      fontSize: 12,
    }}>
      <div style={{ padding: "8px 10px", borderBottom: "1px solid", borderColor: "inherit" }}>
        <div style={{ fontWeight: 600, marginBottom: 4 }}>
          {profile.name ?? "Profile"}
        </div>
        <div style={{ opacity: 0.7, fontSize: 11 }}>
          {profile.span_count} spans · {profile.thread_count} threads
        </div>
      </div>
      <div style={{ padding: "4px 10px", display: "flex", gap: 4, borderBottom: "1px solid", borderColor: "inherit" }}>
        <button onClick={showAll} style={tinyBtn(mode)}>Show all</button>
        <button onClick={hideAll} style={tinyBtn(mode)}>Hide all</button>
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: "4px 0" }}>
        {lanes.map((lane, i) => (
          <label
            key={i}
            style={{
              display: "flex", alignItems: "center", gap: 6,
              padding: "3px 10px", cursor: "pointer",
              opacity: lane.visible ? 1 : 0.4,
            }}
          >
            <input
              type="checkbox"
              checked={lane.visible}
              onChange={() => toggleVisibility(i)}
              style={{ accentColor: "#6c5ce7" }}
            />
            <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {lane.name}
            </span>
            <span style={{ opacity: 0.5, fontSize: 10 }}>
              {lane.span_count}
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}

// ── Hover Tooltip ──────────────────────────────────────────────────────

function HoverTooltip() {
  const hovered = useHoveredSpan();
  const profile = useProfile();
  const { mode } = useTheme();

  if (!hovered) return null;

  const dur = hovered.end_us - hovered.start_us;
  const pct = profile && profile.duration_us > 0
    ? ((dur / profile.duration_us) * 100).toFixed(2)
    : null;

  return (
    <div style={{
      position: "absolute", top: 44, right: 8,
      padding: "8px 12px", borderRadius: 6, fontSize: 12,
      background: mode === "dark" ? "rgba(30,30,60,0.95)" : "rgba(255,255,255,0.98)",
      color: mode === "dark" ? "#e0e0e0" : "#333",
      boxShadow: "0 4px 12px rgba(0,0,0,0.2)",
      pointerEvents: "none",
      maxWidth: 320, zIndex: 10,
    }}>
      <div style={{ fontWeight: 700 }}>{hovered.name}</div>
      <div style={{ opacity: 0.7, fontSize: 11, marginTop: 2 }}>
        {formatDuration(dur)}
        {pct != null && <span> · {pct}% of trace</span>}
      </div>
      <div style={{ opacity: 0.5, fontSize: 10, marginTop: 2 }}>
        Lane {hovered.lane_index} · Click to select
      </div>
    </div>
  );
}

// ── Detail Panel ───────────────────────────────────────────────────────

function DetailPanel() {
  const { selected, clear } = useSelectedSpan();
  const { mode } = useTheme();

  if (!selected) return null;

  return (
    <div style={{
      padding: "8px 12px",
      borderTop: "1px solid",
      borderColor: mode === "dark" ? "#333" : "#ddd",
      background: mode === "dark" ? "#1a1a2e" : "#f8f9fa",
      color: mode === "dark" ? "#e0e0e0" : "#333",
      display: "flex", alignItems: "center", gap: 12,
      fontSize: 13,
    }}>
      <strong>{selected.name}</strong>
      <span style={{ opacity: 0.6 }}>Lane {selected.lane_index}</span>
      <div style={{ flex: 1 }} />
      <button
        onClick={clear}
        style={{
          border: "none", background: "none", cursor: "pointer",
          color: "inherit", fontSize: 16, opacity: 0.6,
        }}
      >
        ✕
      </button>
    </div>
  );
}

// ── Status Bar ─────────────────────────────────────────────────────────

function StatusBar() {
  const { status, error } = useStatus();
  const profile = useProfile();
  const { start, end } = useViewport();
  const { mode } = useTheme();

  return (
    <div style={{
      padding: "3px 12px",
      borderTop: "1px solid",
      borderColor: mode === "dark" ? "#333" : "#ddd",
      background: mode === "dark" ? "#12122a" : "#e9ecef",
      color: mode === "dark" ? "#888" : "#666",
      fontSize: 11,
      display: "flex", gap: 16,
    }}>
      {status === "error" && <span style={{ color: "#ef4444" }}>Error: {error}</span>}
      {status === "loading" && <span>Loading WASM…</span>}
      {status === "ready" && !profile && <span>Drop a profile or click Open</span>}
      {profile && (
        <>
          <span>{formatDuration(profile.duration_us)}</span>
          <span>Zoom: {(100 / (end - start)).toFixed(0)}%</span>
          <span>{profile.span_count} spans</span>
        </>
      )}
    </div>
  );
}

// ── Helpers ────────────────────────────────────────────────────────────

function Sep() {
  return <div style={{ width: 1, height: 18, background: "#555", opacity: 0.3 }} />;
}

function navBtn(mode: string): React.CSSProperties {
  return {
    border: "none", background: "none", cursor: "pointer",
    color: mode === "dark" ? "#ccc" : "#333",
    fontSize: 14, padding: "2px 6px", borderRadius: 3,
  };
}

function tinyBtn(mode: string): React.CSSProperties {
  return {
    border: "none", borderRadius: 3, cursor: "pointer", fontSize: 10,
    padding: "2px 6px",
    background: mode === "dark" ? "#2a2a4a" : "#dee2e6",
    color: mode === "dark" ? "#ccc" : "#333",
  };
}

function formatDuration(us: number): string {
  if (us < 1000) return `${us.toFixed(1)}µs`;
  if (us < 1_000_000) return `${(us / 1000).toFixed(2)}ms`;
  return `${(us / 1_000_000).toFixed(2)}s`;
}

function downloadBlob(content: string, filename: string, mimeType: string) {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
