use eframe::egui;
use flame_cat_core::model::Session;
use flame_cat_core::parsers;
use flame_cat_protocol::{RenderCommand, Viewport, VisualProfile};

use crate::renderer;
use crate::theme::ThemeMode;

/// Format a duration in µs to human-readable string.
fn format_duration(us: f64) -> String {
    if us < 1000.0 {
        format!("{:.1}µs", us)
    } else if us < 1_000_000.0 {
        format!("{:.2}ms", us / 1000.0)
    } else {
        format!("{:.2}s", us / 1_000_000.0)
    }
}

/// Main application state.
pub struct FlameApp {
    session: Option<Session>,
    /// Per-lane state.
    lanes: Vec<LaneState>,
    /// Fractional view window [0..1].
    view_start: f64,
    view_end: f64,
    /// Theme mode.
    theme_mode: ThemeMode,
    /// Active visualization mode.
    view_type: crate::ViewType,
    /// Cached render commands per lane (invalidated on zoom/scroll/resize).
    lane_commands: Vec<Vec<RenderCommand>>,
    /// Global vertical scroll offset in pixels.
    scroll_y: f32,
    /// Selected span for detail panel.
    selected_span: Option<SelectedSpan>,
    /// Search query for filtering spans.
    search_query: String,
    /// Error message to display.
    error: Option<String>,
    /// Pending profile data from async load.
    pending_data: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,
    /// Loading state.
    loading: bool,
    /// Cached minimap density (invalidated on profile load only).
    minimap_density: Option<Vec<u32>>,
    /// Show keyboard help overlay.
    show_help: bool,
    /// Animation targets for smooth viewport transitions.
    anim_target: Option<(f64, f64)>,
    /// Context menu state: span info + screen position.
    context_menu: Option<ContextMenu>,
    /// Currently hovered span (for JS event hooks).
    hovered_span: Option<SelectedSpan>,
    /// Zoom history for back/forward navigation.
    zoom_history: Vec<(f64, f64)>,
    /// Current position in zoom_history (index of last applied entry).
    zoom_history_pos: usize,
}

#[derive(Clone)]
struct ContextMenu {
    span_name: String,
    /// Viewport-fractional bounds for zoom-to-span.
    zoom_start: f64,
    zoom_end: f64,
    pos: egui::Pos2,
}

#[derive(Clone)]
struct SelectedSpan {
    name: String,
    frame_id: u64,
    lane_index: usize,
    /// Time bounds for zoom-to-span (in session µs).
    start_us: f64,
    end_us: f64,
}

enum LaneKind {
    /// Flame chart for a thread (uses render_time_order).
    Thread(u32),
    /// Counter track (memory, CPU, etc.).
    Counter(usize),
    /// Async spans track.
    AsyncSpans,
    /// Markers track.
    Markers,
    /// CPU samples track.
    CpuSamples,
    /// Frame timing track.
    FrameTrack,
    /// Object lifecycle track (GC objects, etc.).
    ObjectTrack,
    /// Minimap overview.
    Minimap,
}

struct LaneState {
    kind: LaneKind,
    name: String,
    height: f32,
    visible: bool,
    span_count: usize,
}

impl FlameApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Use dark theme by default
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let pending_data: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));

        // On WASM, check URL hash for auto-load (e.g. #demo)
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window();
            if let Some(w) = window {
                let hash = w.location().hash().unwrap_or_default();
                if hash == "#demo" || hash == "#react-demo" || hash == "#react-devtools" {
                    let pd = pending_data.clone();
                    let ctx = cc.egui_ctx.clone();
                    let asset = match hash.as_str() {
                        "#react-demo" => "/assets/react-demo.json",
                        "#react-devtools" => "/assets/react-devtools-demo.json",
                        _ => "/assets/demo.json",
                    };
                    web_sys::console::log_1(&format!("flame.cat: loading {asset}...").into());
                    wasm_bindgen_futures::spawn_local(async move {
                        let result = if asset == "/assets/demo.json" {
                            Self::get_preloaded_demo().await.or_else(|| {
                                web_sys::console::log_1(
                                    &"flame.cat: preload miss, fetching...".into(),
                                );
                                None
                            })
                        } else {
                            None
                        };
                        let result = match result {
                            Some(data) => Ok(data),
                            None => Self::fetch_bytes(asset).await,
                        };
                        match result {
                            Ok(resp) => {
                                web_sys::console::log_1(
                                    &format!("flame.cat: loaded {} bytes", resp.len()).into(),
                                );
                                if let Ok(mut lock) = pd.lock() {
                                    *lock = Some(resp);
                                }
                                ctx.request_repaint();
                            }
                            Err(e) => {
                                web_sys::console::error_1(
                                    &format!("flame.cat: fetch error: {e}").into(),
                                );
                            }
                        }
                    });
                }
            }
        }

        Self {
            session: None,
            lanes: Vec::new(),
            view_start: 0.0,
            view_end: 1.0,
            theme_mode: ThemeMode::Dark,
            view_type: crate::ViewType::TimeOrder,
            lane_commands: Vec::new(),
            scroll_y: 0.0,
            selected_span: None,
            search_query: String::new(),
            error: None,
            pending_data,
            loading: false,
            minimap_density: None,
            show_help: false,
            anim_target: None,
            context_menu: None,
            hovered_span: None,
            zoom_history: vec![(0.0, 1.0)],
            zoom_history_pos: 0,
        }
    }

    /// Get a clone of the pending_data handle for JS interop.
    pub fn pending_data_handle(&self) -> std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>> {
        self.pending_data.clone()
    }

    fn load_profile(&mut self, data: &[u8]) {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&format!("flame.cat: parsing {} bytes...", data.len()).into());
        match parsers::parse_auto_visual(data) {
            Ok(mut profile) => {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(
                    &format!("flame.cat: loaded {} threads", profile.threads.len()).into(),
                );

                // Crop profile time bounds to actual span data range
                let mut data_start = f64::INFINITY;
                let mut data_end = f64::NEG_INFINITY;
                for span in profile.all_spans() {
                    data_start = data_start.min(span.start);
                    data_end = data_end.max(span.end);
                }
                if data_start.is_finite() && data_end.is_finite() && data_start < data_end {
                    profile.meta.start_time = data_start;
                    profile.meta.end_time = data_end;
                }

                self.setup_lanes(&profile);

                // Cache serialized profile for export
                crate::set_profile_json(serde_json::to_string(&profile).ok());

                // Compute auto-zoom bounds before consuming profile
                let zoom_bounds = compute_auto_zoom(&profile);

                let session = Session::from_profile(profile, "Profile");
                let session_start = session.start_time();
                let session_end = session.end_time();
                let duration = session_end - session_start;

                if duration > 0.0 {
                    if let Some((lo, hi)) = zoom_bounds {
                        let pad = (hi - lo) * 0.15;
                        self.view_start = ((lo - pad - session_start) / duration).clamp(0.0, 1.0);
                        self.view_end = ((hi + pad - session_start) / duration).clamp(0.0, 1.0);
                    }
                } else {
                    self.view_start = 0.0;
                    self.view_end = 1.0;
                }

                self.session = Some(session);
                self.scroll_y = 0.0;
                self.error = None;
                self.selected_span = None;
                self.minimap_density = None;
                self.invalidate_commands();
            }
            Err(e) => {
                self.error = Some(format!("Failed to parse profile: {e}"));
            }
        }
    }

    fn setup_lanes(&mut self, profile: &VisualProfile) {
        self.lanes.clear();

        // Collect threads sorted by span count (densest first)
        let mut thread_info: Vec<_> = profile
            .threads
            .iter()
            .map(|t| {
                let span_count = t.spans.len();
                let max_depth = t.spans.iter().map(|s| s.depth).max().unwrap_or(0);
                (t, span_count, max_depth)
            })
            .collect();
        thread_info.sort_by(|a, b| b.1.cmp(&a.1));

        // Split threads: dense (≥100 spans) go first, sparse go after specialty tracks
        let dense_threshold = 100;
        let (dense_threads, sparse_threads): (Vec<_>, Vec<_>) = thread_info
            .into_iter()
            .partition(|(_, count, _)| *count >= dense_threshold);

        // Dense threads first
        for (thread, span_count, max_depth) in &dense_threads {
            let content_height = if *max_depth == 0 {
                20.0_f32
            } else {
                ((*max_depth + 1) as f32 * 18.0 + 4.0).min(180.0)
            };
            self.lanes.push(LaneState {
                kind: LaneKind::Thread(thread.id),
                name: format!("{} ({span_count} spans)", thread.name),
                height: content_height,
                visible: true,
                span_count: *span_count,
            });
        }

        // Specialty tracks (between dense and sparse threads)
        if !profile.async_spans.is_empty() {
            let count = profile.async_spans.len();
            self.lanes.push(LaneState {
                kind: LaneKind::AsyncSpans,
                name: format!("Async ({count} spans)"),
                height: 60.0,
                visible: true,
                span_count: count,
            });
        }

        for (i, counter) in profile.counters.iter().enumerate() {
            self.lanes.push(LaneState {
                kind: LaneKind::Counter(i),
                name: format!("📊 {}", counter.name),
                height: 80.0,
                visible: true,
                span_count: counter.samples.len(),
            });
        }

        if !profile.markers.is_empty() {
            let count = profile.markers.len();
            self.lanes.push(LaneState {
                kind: LaneKind::Markers,
                name: format!("Markers ({count})"),
                height: 30.0,
                visible: true,
                span_count: count,
            });
        }

        if profile.cpu_samples.is_some() {
            self.lanes.push(LaneState {
                kind: LaneKind::CpuSamples,
                name: "CPU Samples".to_string(),
                height: 80.0,
                visible: true,
                span_count: profile.cpu_samples.as_ref().map_or(0, |s| s.timestamps.len()),
            });
        }

        if !profile.frames.is_empty() {
            let count = profile.frames.len();
            self.lanes.push(LaneState {
                kind: LaneKind::FrameTrack,
                name: format!("Frames ({count})"),
                height: 40.0,
                visible: true,
                span_count: count,
            });
        }

        if !profile.object_events.is_empty() {
            let count = profile.object_events.len();
            self.lanes.push(LaneState {
                kind: LaneKind::ObjectTrack,
                name: format!("Objects ({count})"),
                height: 60.0,
                visible: true,
                span_count: count,
            });
        }

        // Sparse threads after specialty tracks
        for (thread, span_count, max_depth) in &sparse_threads {
            let content_height = if *max_depth == 0 {
                16.0_f32
            } else {
                ((*max_depth + 1) as f32 * 18.0 + 4.0).min(120.0)
            };
            self.lanes.push(LaneState {
                kind: LaneKind::Thread(thread.id),
                name: format!("{} ({span_count} spans)", thread.name),
                height: content_height,
                visible: *span_count >= 3,
                span_count: *span_count,
            });
        }
    }

    fn invalidate_commands(&mut self) {
        self.lane_commands.clear();
    }

    /// Push a zoom entry to history (truncate any forward history).
    fn push_zoom(&mut self) {
        let entry = (self.view_start, self.view_end);
        // Skip duplicate entries
        if self.zoom_history.last() == Some(&entry) {
            return;
        }
        // Truncate forward history
        self.zoom_history.truncate(self.zoom_history_pos + 1);
        self.zoom_history.push(entry);
        self.zoom_history_pos = self.zoom_history.len() - 1;
        // Cap history at 100 entries
        if self.zoom_history.len() > 100 {
            self.zoom_history.remove(0);
            self.zoom_history_pos = self.zoom_history.len() - 1;
        }
    }

    fn ensure_commands(&mut self, canvas_width: f32) {
        let Some(session) = &self.session else {
            return;
        };
        let Some(entry) = session.profiles().first() else {
            return;
        };

        if self.lane_commands.len() == self.lanes.len() {
            return;
        }

        let session_start = session.start_time();
        let session_end = session.end_time();
        let duration = session_end - session_start;
        if duration <= 0.0 {
            return;
        }

        let abs_start = session_start + self.view_start * duration;
        let abs_end = session_start + self.view_end * duration;

        self.lane_commands.clear();
        for lane in &self.lanes {
            if !lane.visible {
                self.lane_commands.push(Vec::new());
                continue;
            }
            let viewport = Viewport {
                x: 0.0,
                y: 0.0,
                width: canvas_width as f64,
                height: lane.height as f64,
                dpr: 1.0,
            };
            let cmds = match &lane.kind {
                LaneKind::Thread(tid) => match self.view_type {
                    crate::ViewType::TimeOrder => {
                        flame_cat_core::views::time_order::render_time_order(
                            &entry.profile,
                            &viewport,
                            abs_start,
                            abs_end,
                            Some(*tid),
                        )
                    }
                    crate::ViewType::LeftHeavy => {
                        flame_cat_core::views::left_heavy::render_left_heavy(
                            &entry.profile,
                            &viewport,
                            Some(*tid),
                        )
                    }
                    crate::ViewType::Sandwich => {
                        if let Some(ref sel) = self.selected_span {
                            flame_cat_core::views::sandwich::render_sandwich(
                                &entry.profile,
                                sel.frame_id,
                                &viewport,
                            )
                        } else {
                            // No span selected — show time order as fallback
                            flame_cat_core::views::time_order::render_time_order(
                                &entry.profile,
                                &viewport,
                                abs_start,
                                abs_end,
                                Some(*tid),
                            )
                        }
                    }
                    crate::ViewType::Ranked => {
                        flame_cat_core::views::ranked::render_ranked(
                            &entry.profile,
                            &viewport,
                            flame_cat_core::views::ranked::RankedSort::SelfTime,
                            false,
                        )
                    }
                },
                LaneKind::Counter(idx) => {
                    if let Some(counter) = entry.profile.counters.get(*idx) {
                        flame_cat_core::views::counter::render_counter_track(
                            counter, &viewport, abs_start, abs_end,
                        )
                    } else {
                        Vec::new()
                    }
                }
                LaneKind::AsyncSpans => flame_cat_core::views::async_track::render_async_track(
                    &entry.profile.async_spans,
                    &viewport,
                    abs_start,
                    abs_end,
                ),
                LaneKind::Markers => flame_cat_core::views::markers::render_markers(
                    &entry.profile.markers,
                    &viewport,
                    abs_start,
                    abs_end,
                ),
                LaneKind::CpuSamples => {
                    if let Some(ref samples) = entry.profile.cpu_samples {
                        flame_cat_core::views::cpu_samples::render_cpu_samples(
                            samples, &viewport, abs_start, abs_end,
                        )
                    } else {
                        Vec::new()
                    }
                }
                LaneKind::FrameTrack => flame_cat_core::views::frame_track::render_frame_track(
                    &entry.profile.frames,
                    &viewport,
                    abs_start,
                    abs_end,
                ),
                LaneKind::ObjectTrack => flame_cat_core::views::object_track::render_object_track(
                    &entry.profile.object_events,
                    &viewport,
                    abs_start,
                    abs_end,
                ),
                LaneKind::Minimap => {
                    // Minimap is rendered separately as a fixed strip
                    Vec::new()
                }
            };
            self.lane_commands.push(cmds);
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn get_preloaded_demo() -> Option<Vec<u8>> {
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;

        let window = web_sys::window()?;
        let promise = js_sys::Reflect::get(&window, &"__demoData".into()).ok()?;
        if promise.is_undefined() || promise.is_null() {
            return None;
        }
        let promise: js_sys::Promise = promise.dyn_into().ok()?;
        let buf = JsFuture::from(promise).await.ok()?;
        if buf.is_null() || buf.is_undefined() {
            return None;
        }
        let array_buf: js_sys::ArrayBuffer = buf.dyn_into().ok()?;
        let uint8 = js_sys::Uint8Array::new(&array_buf);
        Some(uint8.to_vec())
    }

    #[cfg(target_arch = "wasm32")]
    async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;

        let window = web_sys::window().ok_or("no window")?;
        let resp_value = JsFuture::from(window.fetch_with_str(url))
            .await
            .map_err(|e| format!("{e:?}"))?;
        let resp: web_sys::Response = resp_value.dyn_into().map_err(|_| "not a Response")?;
        if !resp.ok() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let buf = JsFuture::from(resp.array_buffer().map_err(|e| format!("{e:?}"))?)
            .await
            .map_err(|e| format!("{e:?}"))?;
        let uint8 = js_sys::Uint8Array::new(&buf);
        Ok(uint8.to_vec())
    }

    /// Draw the time axis ruler showing tick marks and time labels.
    fn draw_time_axis(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(session) = &self.session else {
            return;
        };
        let session_start = session.start_time();
        let session_end = session.end_time();
        let duration = session_end - session_start;
        if duration <= 0.0 {
            return;
        }

        let painter = ui.painter_at(rect);

        // Background
        let bg = crate::theme::resolve(
            flame_cat_protocol::ThemeToken::LaneHeaderBackground,
            self.theme_mode,
        );
        painter.rect_filled(rect, egui::CornerRadius::ZERO, bg);

        // Visible time window in µs
        let vis_start_us = session_start + self.view_start * duration;
        let vis_end_us = session_start + self.view_end * duration;
        let vis_duration = vis_end_us - vis_start_us;
        if vis_duration <= 0.0 {
            return;
        }

        // Compute nice tick interval
        let target_tick_count = (rect.width() / 100.0).max(2.0) as usize;
        let tick_interval = nice_tick_interval(vis_duration, target_tick_count);

        let text_color = crate::theme::resolve(
            flame_cat_protocol::ThemeToken::TextSecondary,
            self.theme_mode,
        );
        let tick_color =
            crate::theme::resolve(flame_cat_protocol::ThemeToken::LaneBorder, self.theme_mode);

        // First tick aligned to interval (relative to session start)
        let rel_start = vis_start_us - session_start;
        let first_tick = (rel_start / tick_interval).ceil() * tick_interval;

        let mut tick = first_tick;
        while tick <= vis_end_us - session_start {
            let frac = (tick - rel_start) / vis_duration;
            let x = rect.left() + frac as f32 * rect.width();

            // Tick mark
            painter.line_segment(
                [
                    egui::pos2(x, rect.bottom() - 6.0),
                    egui::pos2(x, rect.bottom()),
                ],
                egui::Stroke::new(1.0, tick_color),
            );

            // Time label
            let label = format_tick_label(tick, tick_interval);
            painter.text(
                egui::pos2(x, rect.center().y),
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(10.0),
                text_color,
            );

            tick += tick_interval;
        }

        // Bottom border
        painter.line_segment(
            [
                egui::pos2(rect.left(), rect.bottom()),
                egui::pos2(rect.right(), rect.bottom()),
            ],
            egui::Stroke::new(1.0, tick_color),
        );
    }

    /// Draw an interactive minimap with density heatmap and draggable viewport.
    fn draw_minimap(&mut self, ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response) {
        let Some(session) = &self.session else {
            return;
        };
        let Some(entry) = session.profiles().first() else {
            return;
        };

        let profile = &entry.profile;
        let duration = profile.duration();
        if duration <= 0.0 {
            return;
        }

        // Draw directly on the panel painter (same layer, no clipping issues)
        let painter = ui.painter();

        // Dark background (themed)
        let bg = crate::theme::resolve(
            flame_cat_protocol::ThemeToken::MinimapBackground,
            self.theme_mode,
        );
        painter.rect_filled(rect, egui::CornerRadius::ZERO, bg);

        // Top border
        let border_color =
            crate::theme::resolve(flame_cat_protocol::ThemeToken::LaneBorder, self.theme_mode);
        painter.line_segment(
            [rect.left_top(), egui::pos2(rect.right(), rect.top())],
            egui::Stroke::new(1.0, border_color),
        );

        // Build or reuse cached density per column
        let cols = (rect.width() as usize).max(1);
        let density = match &self.minimap_density {
            Some(cached) if cached.len() == cols => cached,
            _ => {
                let start = profile.meta.start_time;
                let col_dur = duration / cols as f64;
                let mut d = vec![0u32; cols];
                for span in profile.all_spans() {
                    let rel_start = (span.start - start) / col_dur;
                    let rel_end = (span.end - start) / col_dur;
                    if rel_end < 0.0 || rel_start >= cols as f64 {
                        continue;
                    }
                    let c0 = (rel_start.max(0.0) as usize).min(cols);
                    let c1 = (rel_end.ceil() as usize).min(cols);
                    for c in c0..c1 {
                        d[c] += 1;
                    }
                }
                self.minimap_density = Some(d);
                self.minimap_density.as_ref().unwrap()
            }
        };
        let max_d = *density.iter().max().unwrap_or(&1).max(&1);

        // Draw density bars (batch adjacent columns into rects for performance)
        let bar_color = crate::theme::resolve(
            flame_cat_protocol::ThemeToken::MinimapDensity,
            self.theme_mode,
        );
        let mut c = 0;
        while c < cols {
            if density[c] == 0 {
                c += 1;
                continue;
            }
            // Find run of non-zero columns
            let run_start = c;
            let mut run_max = density[c];
            while c < cols && density[c] > 0 {
                run_max = run_max.max(density[c]);
                c += 1;
            }
            // Use sqrt scaling so low-density regions are still visible
            let frac = (run_max as f32).sqrt() / (max_d as f32).sqrt();
            let h = (frac * rect.height()).max(2.0);
            let x = rect.left() + run_start as f32;
            let w = (c - run_start) as f32;
            let bar_rect =
                egui::Rect::from_min_size(egui::pos2(x, rect.bottom() - h), egui::vec2(w, h));
            painter.rect_filled(
                bar_rect,
                egui::CornerRadius::ZERO,
                bar_color.gamma_multiply(0.4 + 0.6 * frac),
            );
        }

        // Viewport highlight
        let vp_left = rect.left() + (self.view_start as f32) * rect.width();
        let vp_right = rect.left() + (self.view_end as f32) * rect.width();
        let viewport_rect = egui::Rect::from_min_max(
            egui::pos2(vp_left, rect.top()),
            egui::pos2(vp_right, rect.bottom()),
        );
        let vp_color = crate::theme::resolve(
            flame_cat_protocol::ThemeToken::MinimapViewport,
            self.theme_mode,
        );
        painter.rect_filled(viewport_rect, egui::CornerRadius::ZERO, vp_color);

        // Viewport border lines
        let handle_color = crate::theme::resolve(
            flame_cat_protocol::ThemeToken::MinimapHandle,
            self.theme_mode,
        );
        painter.line_segment(
            [
                egui::pos2(vp_left, rect.top()),
                egui::pos2(vp_left, rect.bottom()),
            ],
            egui::Stroke::new(2.0, handle_color),
        );
        painter.line_segment(
            [
                egui::pos2(vp_right, rect.top()),
                egui::pos2(vp_right, rect.bottom()),
            ],
            egui::Stroke::new(2.0, handle_color),
        );
        // Top/bottom edges of viewport
        painter.line_segment(
            [
                egui::pos2(vp_left, rect.top()),
                egui::pos2(vp_right, rect.top()),
            ],
            egui::Stroke::new(1.0, handle_color.gamma_multiply(0.5)),
        );
        painter.line_segment(
            [
                egui::pos2(vp_left, rect.bottom()),
                egui::pos2(vp_right, rect.bottom()),
            ],
            egui::Stroke::new(1.0, handle_color.gamma_multiply(0.5)),
        );

        // Bottom border of minimap strip
        painter.line_segment(
            [
                egui::pos2(rect.left(), rect.bottom()),
                egui::pos2(rect.right(), rect.bottom()),
            ],
            egui::Stroke::new(1.0, border_color),
        );

        // Interactive: drag to pan/resize viewport
        let handle_w = 6.0_f32;

        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                self.anim_target = None;
                let frac = ((pos.x - rect.left()) / rect.width()) as f64;
                let frac = frac.clamp(0.0, 1.0);
                let delta_x = resp.drag_delta().x;

                let drag_start = pos.x - delta_x;
                let on_left_handle = (drag_start - vp_left).abs() < handle_w * 2.0;
                let on_right_handle = (drag_start - vp_right).abs() < handle_w * 2.0;

                if on_left_handle {
                    self.view_start = frac.min(self.view_end - 0.001);
                } else if on_right_handle {
                    self.view_end = frac.max(self.view_start + 0.001);
                } else {
                    let dx_frac = (delta_x as f64) / rect.width() as f64;
                    let span = self.view_end - self.view_start;
                    self.view_start = (self.view_start + dx_frac).clamp(0.0, 1.0 - span);
                    self.view_end = self.view_start + span;
                }
                self.invalidate_commands();
            }
        }

        // Click outside viewport: center viewport on click
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let frac = ((pos.x - rect.left()) / rect.width()) as f64;
                let frac = frac.clamp(0.0, 1.0);
                if frac < self.view_start || frac > self.view_end {
                    let span = self.view_end - self.view_start;
                    self.view_start = (frac - span / 2.0).clamp(0.0, 1.0 - span);
                    self.view_end = self.view_start + span;
                    self.invalidate_commands();
                }
            }
        }

        // Cursor hint
        if resp.hovered() {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                let on_left = (pos.x - vp_left).abs() < handle_w * 2.0;
                let on_right = (pos.x - vp_right).abs() < handle_w * 2.0;
                if on_left || on_right {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                } else if pos.x > vp_left && pos.x < vp_right {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
            }
        }
    }

    /// Start an animated transition to the given viewport.
    fn animate_to(&mut self, start: f64, end: f64) {
        self.anim_target = Some((start.max(0.0), end.min(1.0)));
    }

    /// Advance viewport animation by one frame. Returns true if still animating.
    fn tick_animation(&mut self, ctx: &egui::Context) -> bool {
        let Some((target_start, target_end)) = self.anim_target else {
            return false;
        };
        // Exponential ease-out: approach target by 20% each frame (~60fps → ~150ms to settle)
        let t = 0.2;
        let new_start = self.view_start + (target_start - self.view_start) * t;
        let new_end = self.view_end + (target_end - self.view_end) * t;
        // Snap when close enough (sub-pixel precision at any zoom)
        let epsilon = (target_end - target_start) * 1e-4;
        if (new_start - target_start).abs() < epsilon && (new_end - target_end).abs() < epsilon {
            self.view_start = target_start;
            self.view_end = target_end;
            self.anim_target = None;
            self.push_zoom();
            self.invalidate_commands();
            return false;
        }
        self.view_start = new_start;
        self.view_end = new_end;
        self.invalidate_commands();
        ctx.request_repaint();
        true
    }

    fn render_toolbar(&mut self, ctx: &egui::Context) {
        // Top toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🔥 flame.cat");
                ui.separator();

                if ui.button("📂 Open").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Profile", &["json", "cpuprofile", "speedscope"])
                            .pick_file()
                        {
                            match std::fs::read(&path) {
                                Ok(data) => self.load_profile(&data),
                                Err(e) => {
                                    self.error = Some(format!("Failed to read file: {e}"));
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let pd = self.pending_data.clone();
                        let ctx_clone = ctx.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(data) = pick_file_wasm().await {
                                if let Ok(mut lock) = pd.lock() {
                                    *lock = Some(data);
                                }
                                ctx_clone.request_repaint();
                            }
                        });
                    }
                }

                ui.separator();

                let theme_label = match self.theme_mode {
                    ThemeMode::Dark => "🌙 Dark",
                    ThemeMode::Light => "☀ Light",
                };
                if ui.button(theme_label).clicked() {
                    self.theme_mode = match self.theme_mode {
                        ThemeMode::Dark => {
                            ctx.set_visuals(egui::Visuals::light());
                            ThemeMode::Light
                        }
                        ThemeMode::Light => {
                            ctx.set_visuals(egui::Visuals::dark());
                            ThemeMode::Dark
                        }
                    };
                    self.invalidate_commands();
                }

                ui.separator();

                // View type tabs
                if self.session.is_some() {
                    let views = [
                        (crate::ViewType::TimeOrder, "⏱ Time"),
                        (crate::ViewType::LeftHeavy, "◀ Left Heavy"),
                        (crate::ViewType::Sandwich, "🥪 Sandwich"),
                        (crate::ViewType::Ranked, "📊 Ranked"),
                    ];
                    for (vt, label) in views {
                        if ui
                            .selectable_label(self.view_type == vt, label)
                            .clicked()
                        {
                            self.view_type = vt;
                            self.invalidate_commands();
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let zoom_pct = 100.0 / (self.view_end - self.view_start);
                    ui.label(format!("{zoom_pct:.0}%"));
                    ui.separator();

                    if ui
                        .button("❓")
                        .on_hover_text("Keyboard shortcuts (?)")
                        .clicked()
                    {
                        self.show_help = !self.show_help;
                    }
                    ui.separator();

                    // Search box
                    let search_response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("🔍 Search spans…")
                            .desired_width(150.0),
                    );
                    if search_response.changed() {
                        self.invalidate_commands();
                    }
                });
            });
        });
    }

    fn render_status_bar(&self, ctx: &egui::Context) {
        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(session) = &self.session {
                    let duration_us = session.duration();
                    let view_span = self.view_end - self.view_start;
                    let vis_duration_us = view_span * (session.end_time() - session.start_time());
                    ui.label(format!(
                        "Duration: {} | Viewing: {} | Zoom: {:.0}% | Lanes: {}",
                        format_duration(duration_us),
                        format_duration(vis_duration_us),
                        100.0 / view_span,
                        self.lanes.iter().filter(|l| l.visible).count(),
                    ));
                } else {
                    ui.label("No profile loaded — click Open or drag & drop a file");
                }
            });
        });
    }

    fn render_detail_panel(&mut self, ctx: &egui::Context) {
        // Detail panel: show selected span info
        if let Some(selected) = &self.selected_span {
            let selected_clone = selected.clone();
            egui::TopBottomPanel::bottom("detail_panel")
                .min_height(60.0)
                .max_height(150.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("📋 Detail");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕").clicked() {
                                self.selected_span = None;
                            }
                        });
                    });
                    ui.separator();

                    // Find the span in the session to show timing info
                    if let Some(session) = &self.session {
                        if let Some(entry) = session.profiles().first() {
                            let lane = &self.lanes[selected_clone.lane_index];
                            if let LaneKind::Thread(tid) = &lane.kind {
                                if let Some(thread) =
                                    entry.profile.threads.iter().find(|t| t.id == *tid)
                                {
                                    if let Some(span) = thread
                                        .spans
                                        .iter()
                                        .find(|s| s.id == selected_clone.frame_id)
                                    {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(&selected_clone.name)
                                                    .strong()
                                                    .size(13.0),
                                            );
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label(format!(
                                                "Duration: {} | Self: {} | Depth: {} | Thread: {}",
                                                format_duration(span.duration()),
                                                format_duration(span.self_value),
                                                span.depth,
                                                lane.name,
                                            ));
                                        });
                                        if let Some(cat) = &span.category {
                                            ui.label(format!("Category: {}", cat.name));
                                        }
                                    }
                                }
                            } else {
                                // Non-thread lanes: just show the name
                                ui.label(
                                    egui::RichText::new(&selected_clone.name)
                                        .strong()
                                        .size(13.0),
                                );
                            }
                        }
                    }
                });
        }
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        // Sidebar: lane visibility toggles
        if self.session.is_some() {
            egui::SidePanel::left("lane_sidebar")
                .default_width(160.0)
                .min_width(100.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("Lanes");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut changed = false;
                        for lane in &mut self.lanes {
                            let mut vis = lane.visible;
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut vis, "").changed() {
                                    lane.visible = vis;
                                    changed = true;
                                }
                                // Truncate long names (safe for multi-byte chars)
                                let name = if lane.name.chars().count() > 24 {
                                    let end = lane
                                        .name
                                        .char_indices()
                                        .nth(23)
                                        .map_or(lane.name.len(), |(i, _)| i);
                                    format!("{}…", &lane.name[..end])
                                } else {
                                    lane.name.clone()
                                };
                                ui.label(egui::RichText::new(name).size(11.0));
                            });
                        }
                        if changed {
                            self.invalidate_commands();
                        }
                    });
                });
        }
    }

    fn render_central_panel(&mut self, ctx: &egui::Context) {
        // Clear hover state each frame
        self.hovered_span = None;

        // Central panel: flame chart
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.session.is_none() {
                // Welcome screen
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        ui.heading("🔥");
                        ui.heading("Drop a profile here or click Open");
                        ui.label("Supports: Chrome DevTools, Firefox, Speedscope, pprof, Tracy, React DevTools, and more");
                    });
                });
                return;
            }

            // Time axis ruler
            let time_axis_height = 24.0_f32;
            let (time_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), time_axis_height),
                egui::Sense::hover(),
            );
            self.draw_time_axis(ui, time_rect);

            // Minimap overview strip (interactive range slider)
            let minimap_height = 48.0_f32;
            let (minimap_rect, minimap_resp) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), minimap_height),
                egui::Sense::click_and_drag(),
            );
            self.draw_minimap(ui, minimap_rect, &minimap_resp);

            let available = ui.available_rect_before_wrap();

            // Handle zoom/pan input
            let response = ui.allocate_rect(available, egui::Sense::click_and_drag());

            if response.dragged() {
                self.anim_target = None;
                let delta = response.drag_delta();
                let view_span = self.view_end - self.view_start;
                let dx_frac = -(delta.x as f64) / (available.width() as f64) * view_span;
                let new_start = (self.view_start + dx_frac).clamp(0.0, 1.0 - view_span);
                self.view_start = new_start;
                self.view_end = new_start + view_span;
                self.scroll_y -= delta.y;
                self.scroll_y = self.scroll_y.max(0.0);
                self.invalidate_commands();
            }

            // Scroll wheel: Ctrl/Cmd+scroll = zoom, plain scroll = vertical pan
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            let ctrl_held = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);

            if ctrl_held && scroll.y.abs() > 0.1 {
                // Ctrl+scroll = zoom (like Chrome DevTools / Perfetto)
                self.anim_target = None;
                let zoom_factor = 2.0_f64.powf(-(scroll.y as f64) * 0.01);
                let mouse_frac = if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    ((pos.x - available.left()) as f64 / available.width() as f64)
                        .clamp(0.0, 1.0)
                } else {
                    0.5
                };

                let view_span = self.view_end - self.view_start;
                let cursor_time = self.view_start + mouse_frac * view_span;
                let new_span = (view_span * zoom_factor).clamp(1e-12, 1.0);

                self.view_start = (cursor_time - mouse_frac * new_span).max(0.0);
                self.view_end = (self.view_start + new_span).min(1.0);
                self.invalidate_commands();
            } else if !ctrl_held && scroll.y.abs() > 0.1 {
                // Plain scroll = vertical scroll through lanes
                self.scroll_y = (self.scroll_y - scroll.y).max(0.0);
            }

            // Horizontal scroll (trackpad two-finger) = horizontal pan
            if !ctrl_held && scroll.x.abs() > 0.1 {
                self.anim_target = None;
                let view_span = self.view_end - self.view_start;
                let dx_frac =
                    -(scroll.x as f64) / (available.width() as f64) * view_span;
                let new_start = (self.view_start + dx_frac).clamp(0.0, 1.0 - view_span);
                self.view_start = new_start;
                self.view_end = new_start + view_span;
                self.invalidate_commands();
            }

            // Also handle pinch zoom gesture
            let zoom_delta = ui.input(|i| i.zoom_delta());
            if (zoom_delta - 1.0).abs() > 0.001 {
                self.anim_target = None;
                let mouse_frac = if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    ((pos.x - available.left()) as f64 / available.width() as f64)
                        .clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let view_span = self.view_end - self.view_start;
                let cursor_time = self.view_start + mouse_frac * view_span;
                let new_span =
                    (view_span / zoom_delta as f64).clamp(1e-12, 1.0);
                self.view_start = (cursor_time - mouse_frac * new_span).max(0.0);
                self.view_end = (self.view_start + new_span).min(1.0);
                self.invalidate_commands();
            }

            // WASD keyboard navigation
            ui.input(|i| {
                let view_span = self.view_end - self.view_start;
                let pan_step = view_span * 0.1;
                if i.key_pressed(egui::Key::A) || i.key_pressed(egui::Key::ArrowLeft) {
                    self.view_start = (self.view_start - pan_step).max(0.0);
                    self.view_end = self.view_start + view_span;
                    self.invalidate_commands();
                }
                if i.key_pressed(egui::Key::D) || i.key_pressed(egui::Key::ArrowRight) {
                    self.view_end = (self.view_end + pan_step).min(1.0);
                    self.view_start = self.view_end - view_span;
                    self.invalidate_commands();
                }
                if i.key_pressed(egui::Key::W) || i.key_pressed(egui::Key::ArrowUp) {
                    self.scroll_y = (self.scroll_y - 50.0).max(0.0);
                }
                if i.key_pressed(egui::Key::S) || i.key_pressed(egui::Key::ArrowDown) {
                    self.scroll_y += 50.0;
                }
                // +/= key = zoom in, - key = zoom out, 0 = reset
                if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                    let center = (self.view_start + self.view_end) / 2.0;
                    let new_span = (view_span * 0.5).clamp(1e-12, 1.0);
                    self.animate_to(
                        (center - new_span / 2.0).max(0.0),
                        (center + new_span / 2.0).min(1.0),
                    );
                }
                if i.key_pressed(egui::Key::Minus) {
                    let center = (self.view_start + self.view_end) / 2.0;
                    let new_span = (view_span * 2.0).clamp(1e-12, 1.0);
                    self.animate_to(
                        (center - new_span / 2.0).max(0.0),
                        (center + new_span / 2.0).min(1.0),
                    );
                }
                if i.key_pressed(egui::Key::Num0) {
                    self.animate_to(0.0, 1.0);
                    self.scroll_y = 0.0;
                }
                if i.key_pressed(egui::Key::Escape) {
                    self.selected_span = None;
                }
            });

            // Generate render commands AFTER all input (so invalidations are resolved)
            self.ensure_commands(available.width());

            // Clamp scroll_y to valid range
            let total_lane_height: f32 = self.lanes.iter()
                .filter(|l| l.visible)
                .map(|l| l.height + 1.0) // +1 for lane separator
                .sum();
            let max_scroll = (total_lane_height - available.height()).max(0.0);
            self.scroll_y = self.scroll_y.clamp(0.0, max_scroll);

            // Render lanes
            let mut painter = ui.painter_at(available);
            let bg = crate::theme::resolve(
                flame_cat_protocol::ThemeToken::Background,
                self.theme_mode,
            );
            painter.rect_filled(available, egui::CornerRadius::ZERO, bg);

            // Vertical gridlines at time axis tick positions
            if let Some(session) = &self.session {
                let session_start = session.start_time();
                let session_duration = session.end_time() - session_start;
                if session_duration > 0.0 {
                    let vis_start = session_start + self.view_start * session_duration;
                    let vis_end = session_start + self.view_end * session_duration;
                    let vis_dur = vis_end - vis_start;
                    let interval = nice_tick_interval(vis_dur, 8);
                    let first_tick = (vis_start / interval).ceil() * interval;
                    let grid_color = crate::theme::resolve(
                        flame_cat_protocol::ThemeToken::Border,
                        self.theme_mode,
                    ).gamma_multiply(0.3);
                    let mut t = first_tick;
                    while t <= vis_end {
                        let frac = (t - vis_start) / vis_dur;
                        let x = available.left() + frac as f32 * available.width();
                        painter.line_segment(
                            [egui::pos2(x, available.top()), egui::pos2(x, available.bottom())],
                            egui::Stroke::new(1.0, grid_color),
                        );
                        t += interval;
                    }
                }
            }

            let mut y_offset = available.top() - self.scroll_y;
            let mut deferred_zoom: Option<(f64, f64)> = None;
            // Collect tid → y_center for flow arrow rendering
            let mut tid_to_y: std::collections::HashMap<u64, f32> = std::collections::HashMap::new();

            for (i, lane) in self.lanes.iter().enumerate() {
                if !lane.visible {
                    continue;
                }

                let lane_top = y_offset;
                let total_height = lane.height;

                // Record lane y-center for flow arrows
                if let LaneKind::Thread(tid) = &lane.kind {
                    tid_to_y.insert(*tid as u64, lane_top + total_height / 2.0);
                }

                // Skip if completely off-screen
                if lane_top > available.bottom() {
                    break;
                }
                if lane_top + total_height < available.top() {
                    y_offset += total_height + 1.0;
                    continue;
                }

                // Lane content (no header — sidebar identifies lanes)
                let content_rect = egui::Rect::from_min_size(
                    egui::pos2(available.left(), lane_top),
                    egui::vec2(available.width(), lane.height),
                );

                // Set clip for lane content
                let prev_clip = painter.clip_rect();
                painter.set_clip_rect(content_rect.intersect(available));

                let lane_bg = crate::theme::resolve(
                    flame_cat_protocol::ThemeToken::LaneBackground,
                    self.theme_mode,
                );
                painter.rect_filled(content_rect, egui::CornerRadius::ZERO, lane_bg);

                // Render commands
                if let Some(cmds) = self.lane_commands.get(i) {
                    let result = renderer::render_commands(
                        &mut painter,
                        cmds,
                        egui::pos2(available.left(), lane_top),
                        self.theme_mode,
                        &self.search_query,
                    );

                    // Hover tooltip + click to select + right-click context menu
                    if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        if content_rect.contains(hover_pos) {
                            let clicked = response.clicked();
                            let right_clicked = response.secondary_clicked();
                            for hit in &result.hit_regions {
                                if hit.rect.contains(hover_pos) {
                                    if let Some(name) = find_span_label(cmds, hit.frame_id) {
                                        // Update hovered span for JS hooks
                                        self.hovered_span = Some(SelectedSpan {
                                            name: name.to_string(),
                                            frame_id: hit.frame_id,
                                            lane_index: i,
                                            start_us: hit.rect.left() as f64,
                                            end_us: hit.rect.right() as f64,
                                        });

                                        egui::Area::new(egui::Id::new("span_tooltip"))
                                            .order(egui::Order::Tooltip)
                                            .current_pos(hover_pos + egui::vec2(12.0, 12.0))
                                            .show(ui.ctx(), |ui| {
                                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                                    ui.label(&name);
                                                });
                                            });
                                        if clicked {
                                            self.context_menu = None;
                                            self.selected_span = Some(SelectedSpan {
                                                name,
                                                frame_id: hit.frame_id,
                                                lane_index: i,
                                                start_us: hit.rect.left() as f64,
                                                end_us: hit.rect.right() as f64,
                                            });
                                        } else if right_clicked {
                                            let span_left = (hit.rect.left() - available.left()) as f64 / available.width() as f64;
                                            let span_right = (hit.rect.right() - available.left()) as f64 / available.width() as f64;
                                            let view_span = self.view_end - self.view_start;
                                            let abs_left = self.view_start + span_left * view_span;
                                            let abs_right = self.view_start + span_right * view_span;
                                            let pad = (abs_right - abs_left) * 0.15;
                                            self.context_menu = Some(ContextMenu {
                                                span_name: name,
                                                zoom_start: (abs_left - pad).max(0.0),
                                                zoom_end: (abs_right + pad).min(1.0),
                                                pos: hover_pos,
                                            });
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }

                    // Selected span highlight
                    if let Some(sel) = &self.selected_span {
                        if sel.lane_index == i {
                            for hit in &result.hit_regions {
                                if hit.frame_id == sel.frame_id {
                                    let sel_color = crate::theme::resolve(
                                        flame_cat_protocol::ThemeToken::SelectionHighlight,
                                        self.theme_mode,
                                    );
                                    painter.rect_stroke(
                                        hit.rect,
                                        egui::CornerRadius::ZERO,
                                        egui::Stroke::new(2.0, sel_color),
                                        egui::StrokeKind::Outside,
                                    );
                                    break;
                                }
                            }
                        }
                    }

                    // Double-click to zoom to span
                    if response.double_clicked() {
                        if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                            if content_rect.contains(hover_pos) {
                                for hit in &result.hit_regions {
                                    if hit.rect.contains(hover_pos) {
                                        let span_left = (hit.rect.left() - available.left()) as f64 / available.width() as f64;
                                        let span_right = (hit.rect.right() - available.left()) as f64 / available.width() as f64;
                                        let view_span = self.view_end - self.view_start;
                                        let abs_left = self.view_start + span_left * view_span;
                                        let abs_right = self.view_start + span_right * view_span;
                                        let pad = (abs_right - abs_left) * 0.15;
                                        deferred_zoom = Some((
                                            (abs_left - pad).max(0.0),
                                            (abs_right + pad).min(1.0),
                                        ));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                painter.set_clip_rect(prev_clip);

                // Inline lane label (subtle, top-left corner with background pill)
                let label_text = &lane.name;
                let label_font = egui::FontId::proportional(10.0);
                let label_text_color = crate::theme::resolve(
                    flame_cat_protocol::ThemeToken::InlineLabelText,
                    self.theme_mode,
                );
                let label_galley = painter.layout_no_wrap(
                    label_text.clone(),
                    label_font,
                    label_text_color,
                );
                let label_w = label_galley.size().x + 8.0;
                let label_h = label_galley.size().y + 4.0;
                let label_rect = egui::Rect::from_min_size(
                    egui::pos2(available.left() + 2.0, lane_top + 2.0),
                    egui::vec2(label_w, label_h),
                );
                if label_rect.intersects(available) {
                    let label_bg_color = crate::theme::resolve(
                        flame_cat_protocol::ThemeToken::InlineLabelBackground,
                        self.theme_mode,
                    );
                    painter.rect_filled(
                        label_rect,
                        egui::CornerRadius::same(3),
                        label_bg_color,
                    );
                    painter.galley(
                        egui::pos2(available.left() + 6.0, lane_top + 4.0),
                        label_galley,
                        egui::Color32::TRANSPARENT, // color already in galley
                    );
                }

                // Lane border
                let border_color = crate::theme::resolve(
                    flame_cat_protocol::ThemeToken::LaneBorder,
                    self.theme_mode,
                );
                painter.line_segment(
                    [
                        egui::pos2(available.left(), lane_top + total_height),
                        egui::pos2(available.right(), lane_top + total_height),
                    ],
                    egui::Stroke::new(1.0, border_color),
                );

                y_offset += total_height + 1.0;
            }

            // Draw flow arrows across lanes
            if let Some(session) = &self.session {
                if let Some(entry) = session.profiles().first() {
                    let profile = &entry.profile;
                    let session_start = session.start_time();
                    let session_duration = session.end_time() - session_start;
                    if session_duration > 0.0 && !profile.flow_arrows.is_empty() {
                        let arrow_color = crate::theme::resolve(
                            flame_cat_protocol::ThemeToken::FlowArrow,
                            self.theme_mode,
                        );
                        let head_color = crate::theme::resolve(
                            flame_cat_protocol::ThemeToken::FlowArrowHead,
                            self.theme_mode,
                        );
                        let view_span = self.view_end - self.view_start;

                        painter.set_clip_rect(available);

                        for arrow in &profile.flow_arrows {
                            let from_y = tid_to_y.get(&arrow.from_tid);
                            let to_y = tid_to_y.get(&arrow.to_tid);
                            let (Some(&from_y), Some(&to_y)) = (from_y, to_y) else {
                                continue;
                            };

                            // Convert timestamps to fractional viewport position
                            let from_frac =
                                ((arrow.from_ts - session_start) / session_duration - self.view_start) / view_span;
                            let to_frac =
                                ((arrow.to_ts - session_start) / session_duration - self.view_start) / view_span;

                            // Skip if both endpoints are off-screen
                            if (from_frac < -0.1 && to_frac < -0.1)
                                || (from_frac > 1.1 && to_frac > 1.1)
                            {
                                continue;
                            }

                            let from_x = available.left() + from_frac as f32 * available.width();
                            let to_x = available.left() + to_frac as f32 * available.width();

                            let p1 = egui::pos2(from_x, from_y);
                            let p4 = egui::pos2(to_x, to_y);

                            // Cubic Bézier with horizontal control points
                            let dx = (to_x - from_x).abs() * 0.4;
                            let p2 = egui::pos2(from_x + dx, from_y);
                            let p3 = egui::pos2(to_x - dx, to_y);

                            let bezier = egui::epaint::CubicBezierShape::from_points_stroke(
                                [p1, p2, p3, p4],
                                false,
                                egui::Color32::TRANSPARENT,
                                egui::Stroke::new(1.5, arrow_color),
                            );
                            painter.add(bezier);

                            // Small arrowhead triangle at destination
                            let arrow_size = 5.0_f32;
                            let dir = (p4 - p3).normalized();
                            let perp = egui::vec2(-dir.y, dir.x);
                            let tip = p4;
                            let left = tip - dir * arrow_size + perp * arrow_size * 0.5;
                            let right = tip - dir * arrow_size - perp * arrow_size * 0.5;
                            painter.add(egui::epaint::PathShape::convex_polygon(
                                vec![tip, left, right],
                                head_color,
                                egui::Stroke::NONE,
                            ));
                        }
                    }
                }
            }

            // Apply deferred double-click zoom (animated)
            if let Some((new_start, new_end)) = deferred_zoom {
                self.animate_to(new_start, new_end);
            }
        });
    }

    fn handle_file_drop(&mut self, ctx: &egui::Context) {
        // Handle file drop
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(file) = i.raw.dropped_files.first() {
                    if let Some(bytes) = &file.bytes {
                        // bytes is Arc<[u8]>
                        let data: Vec<u8> = bytes.to_vec();
                        // We need to defer this to avoid borrow issues
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(egui::Id::new("pending_file"), data);
                        });
                    }
                }
            }
        });

        // Process pending file drop
        let pending: Option<Vec<u8>> =
            ctx.memory_mut(|mem| mem.data.get_temp::<Vec<u8>>(egui::Id::new("pending_file")));
        if let Some(data) = pending {
            ctx.memory_mut(|mem| {
                mem.data.remove::<Vec<u8>>(egui::Id::new("pending_file"));
            });
            self.load_profile(&data);
        }
    }

    fn render_help_overlay(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_help = false;
            return;
        }

        egui::Area::new(egui::Id::new("help_overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(16.0)
                    .show(ui, |ui| {
                        ui.heading("⌨ Keyboard Shortcuts");
                        ui.separator();
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let shortcuts = [
                            ("A / ←", "Pan left"),
                            ("D / →", "Pan right"),
                            ("W / ↑", "Scroll up"),
                            ("S / ↓", "Scroll down"),
                            ("+", "Zoom in"),
                            ("-", "Zoom out"),
                            ("0", "Reset zoom"),
                            ("Ctrl+Scroll", "Zoom at cursor"),
                            ("Drag", "Pan + vertical scroll"),
                            ("Double-click", "Zoom to span"),
                            ("Click", "Select span"),
                            ("Right-click", "Context menu"),
                            ("Esc", "Deselect / close help"),
                            ("?", "Toggle this help"),
                        ];
                        for (key, desc) in shortcuts {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(key).strong().monospace());
                                ui.label(desc);
                            });
                        }
                        ui.separator();
                        if ui.button("Close").clicked() {
                            self.show_help = false;
                        }
                    });
            });
    }

    fn render_context_menu(&mut self, ctx: &egui::Context) {
        let Some(menu) = self.context_menu.clone() else {
            return;
        };

        let area_resp = egui::Area::new(egui::Id::new("span_context_menu"))
            .order(egui::Order::Foreground)
            .current_pos(menu.pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(160.0);
                    ui.label(egui::RichText::new(&menu.span_name).strong().size(12.0));
                    ui.separator();
                    if ui.button("📋 Copy Name").clicked() {
                        ui.ctx().copy_text(menu.span_name.clone());
                        self.context_menu = None;
                    }
                    if ui.button("🔍 Zoom to Span").clicked() {
                        self.animate_to(menu.zoom_start, menu.zoom_end);
                        self.context_menu = None;
                    }
                    if ui.button("🔎 Find Similar").clicked() {
                        self.search_query = menu.span_name.clone();
                        self.context_menu = None;
                    }
                });
            });

        // Dismiss if clicked outside the menu area or Escape
        let dismiss = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if dismiss {
            self.context_menu = None;
        } else if ctx.input(|i| i.pointer.any_pressed()) {
            let menu_rect = area_resp.response.rect;
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if !menu_rect.contains(pos) {
                    self.context_menu = None;
                }
            }
        }
    }
}

impl eframe::App for FlameApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for async-loaded profile data
        let pending = {
            let mut lock = self.pending_data.lock().unwrap_or_else(|e| e.into_inner());
            lock.take()
        };
        if let Some(data) = pending {
            self.load_profile(&data);
            self.loading = false;
        }

        // Process commands from JS API
        for cmd in crate::drain_commands() {
            match cmd {
                crate::AppCommand::SetTheme(mode) => {
                    self.theme_mode = mode;
                    match mode {
                        crate::theme::ThemeMode::Dark => {
                            ctx.set_visuals(egui::Visuals::dark());
                        }
                        crate::theme::ThemeMode::Light => {
                            ctx.set_visuals(egui::Visuals::light());
                        }
                    }
                    self.invalidate_commands();
                }
                crate::AppCommand::SetSearch(query) => {
                    self.search_query = query;
                    self.invalidate_commands();
                }
                crate::AppCommand::ResetZoom => {
                    self.view_start = 0.0;
                    self.view_end = 1.0;
                    self.scroll_y = 0.0;
                    self.push_zoom();
                    self.invalidate_commands();
                }
                crate::AppCommand::SetViewport(start, end) => {
                    self.view_start = start.max(0.0);
                    self.view_end = end.min(1.0);
                    self.push_zoom();
                    self.invalidate_commands();
                }
                crate::AppCommand::SetLaneVisibility(index, visible) => {
                    if let Some(lane) = self.lanes.get_mut(index) {
                        lane.visible = visible;
                        self.invalidate_commands();
                    }
                }
                crate::AppCommand::SetLaneHeight(index, height) => {
                    if let Some(lane) = self.lanes.get_mut(index) {
                        lane.height = height.max(16.0).min(600.0);
                        self.invalidate_commands();
                    }
                }
                crate::AppCommand::ReorderLanes(from, to) => {
                    let len = self.lanes.len();
                    if from < len && to < len && from != to {
                        let lane = self.lanes.remove(from);
                        self.lanes.insert(to, lane);
                        // Also reorder cached commands
                        if self.lane_commands.len() == len {
                            let cmds = self.lane_commands.remove(from);
                            self.lane_commands.insert(to, cmds);
                        } else {
                            self.invalidate_commands();
                        }
                    }
                }
                crate::AppCommand::SelectSpan(frame_id) => {
                    if let Some(fid) = frame_id {
                        // Find the span name from render commands
                        let label = self
                            .lane_commands
                            .iter()
                            .flat_map(|cmds| cmds.iter())
                            .find_map(|cmd| {
                                if let RenderCommand::DrawRect {
                                    label: Some(label),
                                    frame_id: Some(id),
                                    ..
                                } = cmd
                                {
                                    if *id == fid {
                                        Some(label.clone())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            });
                        if let Some(name) = label {
                            self.selected_span = Some(SelectedSpan {
                                name: name.to_string(),
                                frame_id: fid,
                                lane_index: 0,
                                start_us: 0.0,
                                end_us: 0.0,
                            });
                        }
                    } else {
                        self.selected_span = None;
                    }
                }
                crate::AppCommand::SetViewType(vt) => {
                    self.view_type = vt;
                    self.invalidate_commands();
                }
                crate::AppCommand::NavigateBack => {
                    if self.zoom_history_pos > 0 {
                        self.zoom_history_pos -= 1;
                        let (s, e) = self.zoom_history[self.zoom_history_pos];
                        self.view_start = s;
                        self.view_end = e;
                        self.invalidate_commands();
                    }
                }
                crate::AppCommand::NavigateForward => {
                    if self.zoom_history_pos + 1 < self.zoom_history.len() {
                        self.zoom_history_pos += 1;
                        let (s, e) = self.zoom_history[self.zoom_history_pos];
                        self.view_start = s;
                        self.view_end = e;
                        self.invalidate_commands();
                    }
                }
            }
        }

        self.tick_animation(ctx);

        self.render_toolbar(ctx);
        self.render_status_bar(ctx);
        self.render_detail_panel(ctx);
        self.render_sidebar(ctx);
        self.render_central_panel(ctx);
        self.render_help_overlay(ctx);
        self.render_context_menu(ctx);
        self.handle_file_drop(ctx);

        // Global ? key to toggle help
        if ctx.input(|i| i.key_pressed(egui::Key::Questionmark)) {
            self.show_help = !self.show_help;
        }

        // Emit state snapshot for JS hooks
        self.emit_snapshot();
    }
}

impl FlameApp {
    fn emit_snapshot(&self) {
        let profile = self.session.as_ref().map(|s| {
            let profiles = s.profiles();
            let thread_count: usize = profiles.iter().map(|p| p.profile.threads.len()).sum();
            let span_count: usize = profiles
                .iter()
                .flat_map(|p| &p.profile.threads)
                .map(|t| t.spans.len())
                .sum();
            crate::ProfileSnapshot {
                name: profiles.first().map(|p| p.label.clone()),
                format: profiles
                    .first()
                    .map(|p| format!("{:?}", p.profile.meta.source_format))
                    .unwrap_or_default(),
                duration_us: s.duration(),
                start_time: s.start_time(),
                end_time: s.end_time(),
                span_count,
                thread_count,
            }
        });
        let lanes = self
            .lanes
            .iter()
            .map(|l| crate::LaneSnapshot {
                name: l.name.clone(),
                kind: match &l.kind {
                    LaneKind::Thread(_) => "thread".to_string(),
                    LaneKind::Counter(_) => "counter".to_string(),
                    LaneKind::AsyncSpans => "async".to_string(),
                    LaneKind::Markers => "markers".to_string(),
                    LaneKind::CpuSamples => "cpu_samples".to_string(),
                    LaneKind::FrameTrack => "frame_track".to_string(),
                    LaneKind::ObjectTrack => "object_track".to_string(),
                    LaneKind::Minimap => "minimap".to_string(),
                },
                height: l.height,
                visible: l.visible,
                span_count: l.span_count,
            })
            .collect();
        let viewport = crate::ViewportSnapshot {
            start: self.view_start,
            end: self.view_end,
            scroll_y: self.scroll_y,
        };
        let selected = self.selected_span.as_ref().map(|s| crate::SelectedSpanSnapshot {
            name: s.name.clone(),
            frame_id: s.frame_id,
            lane_index: s.lane_index,
            start_us: s.start_us,
            end_us: s.end_us,
        });
        let hovered = self.hovered_span.as_ref().map(|s| crate::SelectedSpanSnapshot {
            name: s.name.clone(),
            frame_id: s.frame_id,
            lane_index: s.lane_index,
            start_us: s.start_us,
            end_us: s.end_us,
        });
        let theme = match self.theme_mode {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
        }
        .to_string();
        crate::write_snapshot(crate::StateSnapshot {
            profile,
            lanes,
            viewport,
            selected,
            hovered,
            search: self.search_query.clone(),
            theme,
            view_type: self.view_type,
            can_go_back: self.zoom_history_pos > 0,
            can_go_forward: self.zoom_history_pos + 1 < self.zoom_history.len(),
        });
    }
}

/// Find the label for a span by its frame_id in the render commands.
fn find_span_label(cmds: &[RenderCommand], frame_id: u64) -> Option<String> {
    for cmd in cmds {
        if let RenderCommand::DrawRect {
            label: Some(label),
            frame_id: Some(fid),
            ..
        } = cmd
        {
            if *fid == frame_id {
                return Some(label.to_string());
            }
        }
    }
    None
}

/// Compute a "nice" tick interval for the time axis.
/// Returns interval in µs.
fn nice_tick_interval(visible_duration_us: f64, target_ticks: usize) -> f64 {
    let raw = visible_duration_us / target_ticks as f64;
    // Find the nearest "nice" number: 1, 2, 5, 10, 20, 50, ...
    let mag = 10.0_f64.powf(raw.log10().floor());
    let residual = raw / mag;
    let nice = if residual <= 1.5 {
        1.0
    } else if residual <= 3.5 {
        2.0
    } else if residual <= 7.5 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

/// Format a tick label (time in µs relative to session start) with appropriate units.
fn format_tick_label(us: f64, interval: f64) -> String {
    if interval >= 1_000_000.0 {
        format!("{:.1}s", us / 1_000_000.0)
    } else if interval >= 1_000.0 {
        format!("{:.1}ms", us / 1_000.0)
    } else {
        format!("{:.0}µs", us)
    }
}

/// Find the densest time window in the busiest thread.
/// Returns `Some((lo, hi))` in µs, or `None` if no spans.
fn compute_auto_zoom(profile: &VisualProfile) -> Option<(f64, f64)> {
    let thread = profile.threads.iter().max_by_key(|t| t.spans.len())?;
    if thread.spans.is_empty() {
        return None;
    }

    if thread.spans.len() < 10 {
        let cmin = thread
            .spans
            .iter()
            .map(|s| s.start)
            .fold(f64::INFINITY, f64::min);
        let cmax = thread
            .spans
            .iter()
            .map(|s| s.end)
            .fold(f64::NEG_INFINITY, f64::max);
        return if cmin.is_finite() && cmax.is_finite() {
            Some((cmin, cmax))
        } else {
            None
        };
    }

    // Sort start times, then sliding window for smallest range covering 80% of spans
    let mut starts: Vec<f64> = thread.spans.iter().map(|s| s.start).collect();
    starts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let window_size = (starts.len() * 4) / 5; // 80% of spans
    let mut best_range = f64::MAX;
    let mut best_lo = starts[0];
    let mut best_hi = *starts.last().unwrap();
    for i in 0..starts.len() - window_size {
        let range = starts[i + window_size] - starts[i];
        if range < best_range {
            best_range = range;
            best_lo = starts[i];
            best_hi = starts[i + window_size];
        }
    }
    Some((best_lo, best_hi))
}

/// WASM file picker using the browser's File API.
#[cfg(target_arch = "wasm32")]
async fn pick_file_wasm() -> Result<Vec<u8>, String> {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let document = web_sys::window()
        .ok_or("no window")?
        .document()
        .ok_or("no document")?;

    let input: web_sys::HtmlInputElement = document
        .create_element("input")
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|_| "not an input")?;
    input.set_type("file");
    input.set_accept(".json,.cpuprofile,.speedscope,.pprof,.tracy");

    // Create a promise that resolves when a file is selected
    let (tx, rx) = futures_channel::oneshot::channel::<Vec<u8>>();
    let tx = std::rc::Rc::new(std::cell::RefCell::new(Some(tx)));

    let tx_clone = tx.clone();
    let input_clone = input.clone();
    let closure = Closure::wrap(Box::new(move || {
        let files = input_clone.files();
        if let Some(files) = files {
            if let Some(file) = files.get(0) {
                let reader = web_sys::FileReader::new().unwrap();
                let reader_clone = reader.clone();
                let tx_inner = tx_clone.clone();
                let onload = Closure::wrap(Box::new(move || {
                    if let Ok(result) = reader_clone.result() {
                        let buffer = js_sys::Uint8Array::new(&result);
                        let data = buffer.to_vec();
                        if let Some(sender) = tx_inner.borrow_mut().take() {
                            let _ = sender.send(data);
                        }
                    }
                }) as Box<dyn FnMut()>);
                reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                onload.forget();
                let _ = reader.read_as_array_buffer(&file);
            }
        }
    }) as Box<dyn FnMut()>);

    input
        .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())
        .map_err(|e| format!("{e:?}"))?;
    closure.forget();

    input.click();

    rx.await.map_err(|_| "file pick cancelled".to_string())
}
