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
    /// Cached render commands per lane (invalidated on zoom/scroll/resize).
    lane_commands: Vec<Vec<RenderCommand>>,
    /// Global vertical scroll offset in pixels.
    scroll_y: f32,
    /// Currently hovered frame_id.
    #[allow(dead_code)]
    hovered_frame: Option<u64>,
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
}

#[derive(Clone)]
struct SelectedSpan {
    name: String,
    frame_id: u64,
    lane_index: usize,
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
    #[allow(dead_code)]
    scroll_y: f32,
    visible: bool,
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
                if hash == "#demo" {
                    let pd = pending_data.clone();
                    let ctx = cc.egui_ctx.clone();
                    web_sys::console::log_1(&"flame.cat: loading demo profile...".into());
                    wasm_bindgen_futures::spawn_local(async move {
                        match Self::fetch_bytes("/assets/demo.json").await {
                            Ok(resp) => {
                                web_sys::console::log_1(
                                    &format!("flame.cat: fetched {} bytes", resp.len()).into(),
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
            lane_commands: Vec::new(),
            scroll_y: 0.0,
            hovered_frame: None,
            selected_span: None,
            search_query: String::new(),
            error: None,
            pending_data,
            loading: false,
        }
    }

    fn load_profile(&mut self, data: &[u8]) {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!("flame.cat: parsing {} bytes...", data.len()).into(),
        );
        match parsers::parse_auto_visual(data) {
            Ok(profile) => {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(
                    &format!(
                        "flame.cat: loaded {} threads",
                        profile.threads.len()
                    )
                    .into(),
                );
                self.setup_lanes(&profile);

                // Compute auto-zoom bounds before consuming profile
                let zoom_bounds = compute_auto_zoom(&profile);

                let session = Session::from_profile(profile, "Profile");
                let session_start = session.start_time();
                let session_end = session.end_time();
                let duration = session_end - session_start;

                if duration > 0.0 {
                    if let Some((lo, hi)) = zoom_bounds {
                        let pad = (hi - lo) * 0.15;
                        self.view_start =
                            ((lo - pad - session_start) / duration).clamp(0.0, 1.0);
                        self.view_end =
                            ((hi + pad - session_start) / duration).clamp(0.0, 1.0);
                    }
                } else {
                    self.view_start = 0.0;
                    self.view_end = 1.0;
                }

                self.session = Some(session);
                self.scroll_y = 0.0;
                self.error = None;
                self.selected_span = None;
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
                ((*max_depth + 1) as f32 * 18.0 + 4.0).min(250.0)
            };
            self.lanes.push(LaneState {
                kind: LaneKind::Thread(thread.id),
                name: format!("{} ({span_count} spans)", thread.name),
                height: content_height,
                scroll_y: 0.0,
                visible: true,
            });
        }

        // Specialty tracks (between dense and sparse threads)
        if !profile.async_spans.is_empty() {
            self.lanes.push(LaneState {
                kind: LaneKind::AsyncSpans,
                name: format!("Async ({} spans)", profile.async_spans.len()),
                height: 60.0,
                scroll_y: 0.0,
                visible: true,
            });
        }

        for (i, counter) in profile.counters.iter().enumerate() {
            self.lanes.push(LaneState {
                kind: LaneKind::Counter(i),
                name: format!("📊 {}", counter.name),
                height: 80.0,
                scroll_y: 0.0,
                visible: true,
            });
        }

        if !profile.markers.is_empty() {
            self.lanes.push(LaneState {
                kind: LaneKind::Markers,
                name: format!("Markers ({})", profile.markers.len()),
                height: 30.0,
                scroll_y: 0.0,
                visible: true,
            });
        }

        if profile.cpu_samples.is_some() {
            self.lanes.push(LaneState {
                kind: LaneKind::CpuSamples,
                name: "CPU Samples".to_string(),
                height: 80.0,
                scroll_y: 0.0,
                visible: true,
            });
        }

        if !profile.frames.is_empty() {
            self.lanes.push(LaneState {
                kind: LaneKind::FrameTrack,
                name: format!("Frames ({})", profile.frames.len()),
                height: 40.0,
                scroll_y: 0.0,
                visible: true,
            });
        }

        if !profile.object_events.is_empty() {
            self.lanes.push(LaneState {
                kind: LaneKind::ObjectTrack,
                name: format!("Objects ({})", profile.object_events.len()),
                height: 60.0,
                scroll_y: 0.0,
                visible: true,
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
                scroll_y: 0.0,
                visible: *span_count >= 3,
            });
        }

        // Minimap (always last)
        self.lanes.push(LaneState {
            kind: LaneKind::Minimap,
            name: "Overview".to_string(),
            height: 40.0,
            scroll_y: 0.0,
            visible: true,
        });
    }

    fn invalidate_commands(&mut self) {
        self.lane_commands.clear();
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
                y: lane.scroll_y as f64,
                width: canvas_width as f64,
                height: lane.height as f64,
                dpr: 1.0,
            };
            let cmds = match &lane.kind {
                LaneKind::Thread(tid) => {
                    flame_cat_core::views::time_order::render_time_order(
                        &entry.profile,
                        &viewport,
                        abs_start,
                        abs_end,
                        Some(*tid),
                    )
                }
                LaneKind::Counter(idx) => {
                    if let Some(counter) = entry.profile.counters.get(*idx) {
                        flame_cat_core::views::counter::render_counter_track(
                            counter, &viewport, abs_start, abs_end,
                        )
                    } else {
                        Vec::new()
                    }
                }
                LaneKind::AsyncSpans => {
                    flame_cat_core::views::async_track::render_async_track(
                        &entry.profile.async_spans,
                        &viewport,
                        abs_start,
                        abs_end,
                    )
                }
                LaneKind::Markers => {
                    flame_cat_core::views::markers::render_markers(
                        &entry.profile.markers,
                        &viewport,
                        abs_start,
                        abs_end,
                    )
                }
                LaneKind::CpuSamples => {
                    if let Some(ref samples) = entry.profile.cpu_samples {
                        flame_cat_core::views::cpu_samples::render_cpu_samples(
                            samples, &viewport, abs_start, abs_end,
                        )
                    } else {
                        Vec::new()
                    }
                }
                LaneKind::FrameTrack => {
                    flame_cat_core::views::frame_track::render_frame_track(
                        &entry.profile.frames,
                        &viewport,
                        abs_start,
                        abs_end,
                    )
                }
                LaneKind::ObjectTrack => {
                    flame_cat_core::views::object_track::render_object_track(
                        &entry.profile.object_events,
                        &viewport,
                        abs_start,
                        abs_end,
                    )
                }
                LaneKind::Minimap => {
                    flame_cat_core::views::minimap::render_minimap(
                        &entry.profile,
                        &viewport,
                        self.view_start,
                        self.view_end,
                    )
                }
            };
            self.lane_commands.push(cmds);
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;

        let window = web_sys::window().ok_or("no window")?;
        let resp_value = JsFuture::from(window.fetch_with_str(url))
            .await
            .map_err(|e| format!("{e:?}"))?;
        let resp: web_sys::Response = resp_value
            .dyn_into()
            .map_err(|_| "not a Response")?;
        if !resp.ok() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let buf = JsFuture::from(
            resp.array_buffer().map_err(|e| format!("{e:?}"))?
        )
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
        let tick_color = crate::theme::resolve(
            flame_cat_protocol::ThemeToken::LaneBorder,
            self.theme_mode,
        );

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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let zoom_pct = 100.0 / (self.view_end - self.view_start);
                    ui.label(format!("{zoom_pct:.0}%"));
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

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(session) = &self.session {
                    let duration_us = session.duration();
                    let view_span = self.view_end - self.view_start;
                    let vis_duration_us =
                        view_span * (session.end_time() - session.start_time());
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
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.button("✕").clicked() {
                                    self.selected_span = None;
                                }
                            },
                        );
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
                                // Truncate long names
                                let name = if lane.name.len() > 24 {
                                    format!("{}…", &lane.name[..23])
                                } else {
                                    lane.name.clone()
                                };
                                ui.label(
                                    egui::RichText::new(name).size(11.0),
                                );
                            });
                        }
                        if changed {
                            self.invalidate_commands();
                        }
                    });
                });
        }

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

            let available = ui.available_rect_before_wrap();
            self.ensure_commands(available.width());

            // Handle zoom/pan input
            let response = ui.allocate_rect(available, egui::Sense::click_and_drag());

            if response.dragged() {
                let delta = response.drag_delta();
                let view_span = self.view_end - self.view_start;
                let dx_frac = -(delta.x as f64) / (available.width() as f64) * view_span;
                self.view_start = (self.view_start + dx_frac).max(0.0);
                self.view_end = (self.view_end + dx_frac).min(1.0);
                self.scroll_y -= delta.y;
                self.scroll_y = self.scroll_y.max(0.0);
                self.invalidate_commands();
            }

            // Scroll wheel = zoom (like Chrome DevTools / Perfetto)
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            if scroll.y.abs() > 0.1 {
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
            }

            // Horizontal scroll (trackpad two-finger) = horizontal pan
            if scroll.x.abs() > 0.1 {
                let view_span = self.view_end - self.view_start;
                let dx_frac =
                    -(scroll.x as f64) / (available.width() as f64) * view_span;
                self.view_start = (self.view_start + dx_frac).max(0.0);
                self.view_end = (self.view_end + dx_frac).min(1.0);
                self.invalidate_commands();
            }

            // Also handle pinch zoom gesture
            let zoom_delta = ui.input(|i| i.zoom_delta());
            if (zoom_delta - 1.0).abs() > 0.001 {
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
                    self.view_start = (center - new_span / 2.0).max(0.0);
                    self.view_end = (center + new_span / 2.0).min(1.0);
                    self.invalidate_commands();
                }
                if i.key_pressed(egui::Key::Minus) {
                    let center = (self.view_start + self.view_end) / 2.0;
                    let new_span = (view_span * 2.0).clamp(1e-12, 1.0);
                    self.view_start = (center - new_span / 2.0).max(0.0);
                    self.view_end = (center + new_span / 2.0).min(1.0);
                    self.invalidate_commands();
                }
                if i.key_pressed(egui::Key::Num0) {
                    self.view_start = 0.0;
                    self.view_end = 1.0;
                    self.scroll_y = 0.0;
                    self.invalidate_commands();
                }
                if i.key_pressed(egui::Key::Escape) {
                    self.selected_span = None;
                }
            });

            // Render lanes
            let mut painter = ui.painter_at(available);
            let bg = crate::theme::resolve(
                flame_cat_protocol::ThemeToken::Background,
                self.theme_mode,
            );
            painter.rect_filled(available, egui::CornerRadius::ZERO, bg);

            let mut y_offset = available.top() - self.scroll_y;

            for (i, lane) in self.lanes.iter().enumerate() {
                if !lane.visible {
                    continue;
                }

                let lane_top = y_offset;
                let total_height = lane.height;

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

                    // Hover tooltip + click to select
                    if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        if content_rect.contains(hover_pos) {
                            let clicked = response.clicked();
                            for hit in &result.hit_regions {
                                if hit.rect.contains(hover_pos) {
                                    if let Some(name) = find_span_label(cmds, hit.frame_id) {
                                        #[allow(deprecated)]
                                        egui::show_tooltip_at_pointer(
                                            ui.ctx(),
                                            ui.layer_id(),
                                            egui::Id::new("span_tooltip"),
                                            |ui| {
                                                ui.label(&name);
                                            },
                                        );
                                        if clicked {
                                            self.selected_span = Some(SelectedSpan {
                                                name,
                                                frame_id: hit.frame_id,
                                                lane_index: i,
                                            });
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }

                painter.set_clip_rect(prev_clip);

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
        });

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
        let pending: Option<Vec<u8>> = ctx.memory_mut(|mem| {
            mem.data.get_temp::<Vec<u8>>(egui::Id::new("pending_file"))
        });
        if let Some(data) = pending {
            ctx.memory_mut(|mem| {
                mem.data.remove::<Vec<u8>>(egui::Id::new("pending_file"));
            });
            self.load_profile(&data);
        }
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
        let cmin = thread.spans.iter().map(|s| s.start).fold(f64::INFINITY, f64::min);
        let cmax = thread.spans.iter().map(|s| s.end).fold(f64::NEG_INFINITY, f64::max);
        return if cmin.is_finite() && cmax.is_finite() {
            Some((cmin, cmax))
        } else {
            None
        };
    }

    // Sort start times, then sliding window for smallest range covering 50% of spans
    let mut starts: Vec<f64> = thread.spans.iter().map(|s| s.start).collect();
    starts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let half = starts.len() / 2;
    let mut best_range = f64::MAX;
    let mut best_lo = starts[0];
    let mut best_hi = *starts.last().unwrap();
    for i in 0..starts.len() - half {
        let range = starts[i + half] - starts[i];
        if range < best_range {
            best_range = range;
            best_lo = starts[i];
            best_hi = starts[i + half];
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
