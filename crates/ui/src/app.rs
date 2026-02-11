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
    /// Error message to display.
    error: Option<String>,
    /// Pending profile data from async load.
    pending_data: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,
    /// Loading state.
    loading: bool,
}

struct LaneState {
    thread_id: Option<u32>,
    thread_name: String,
    height: f32,
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

                // Find content bounds to auto-zoom
                // Focus on the thread with the most spans (usually the main thread)
                let densest_thread = profile
                    .threads
                    .iter()
                    .max_by_key(|t| t.spans.len());

                let (content_min, content_max) = if let Some(thread) = densest_thread {
                    let mut cmin = f64::INFINITY;
                    let mut cmax = f64::NEG_INFINITY;
                    for span in &thread.spans {
                        cmin = cmin.min(span.start);
                        cmax = cmax.max(span.end);
                    }
                    (cmin, cmax)
                } else {
                    (f64::INFINITY, f64::NEG_INFINITY)
                };

                let session = Session::from_profile(profile, "Profile");
                let session_start = session.start_time();
                let session_end = session.end_time();
                let duration = session_end - session_start;

                if duration > 0.0
                    && content_min.is_finite()
                    && content_max.is_finite()
                    && content_max > content_min
                {
                    // Add 5% padding around content
                    let content_dur = content_max - content_min;
                    let pad = content_dur * 0.05;
                    self.view_start =
                        ((content_min - pad - session_start) / duration).clamp(0.0, 1.0);
                    self.view_end =
                        ((content_max + pad - session_start) / duration).clamp(0.0, 1.0);
                } else {
                    self.view_start = 0.0;
                    self.view_end = 1.0;
                }

                self.session = Some(session);
                self.scroll_y = 0.0;
                self.error = None;
                self.invalidate_commands();
            }
            Err(e) => {
                self.error = Some(format!("Failed to parse profile: {e}"));
            }
        }
    }

    fn setup_lanes(&mut self, profile: &VisualProfile) {
        self.lanes.clear();
        for thread in &profile.threads {
            let span_count = thread.spans.len();
            let max_depth = thread
                .spans
                .iter()
                .map(|s| s.depth)
                .max()
                .unwrap_or(0);
            let content_height = ((max_depth + 1) as f32 * 20.0 + 8.0).max(40.0).min(400.0);
            self.lanes.push(LaneState {
                thread_id: Some(thread.id),
                thread_name: format!("{} ({span_count} spans)", thread.name),
                height: content_height,
                scroll_y: 0.0,
                visible: span_count >= 3,
            });
        }
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
            let cmds = flame_cat_core::views::time_order::render_time_order(
                &entry.profile,
                &viewport,
                abs_start,
                abs_end,
                lane.thread_id,
            );
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
            });

            // Render lanes
            let mut painter = ui.painter_at(available);
            let bg = crate::theme::resolve(
                flame_cat_protocol::ThemeToken::Background,
                self.theme_mode,
            );
            painter.rect_filled(available, egui::CornerRadius::ZERO, bg);

            let mut y_offset = available.top() - self.scroll_y;
            let header_height = 24.0_f32;

            for (i, lane) in self.lanes.iter().enumerate() {
                if !lane.visible {
                    continue;
                }

                let lane_top = y_offset;
                let total_height = header_height + lane.height;

                // Skip if completely off-screen
                if lane_top > available.bottom() {
                    break;
                }
                if lane_top + total_height < available.top() {
                    y_offset += total_height + 2.0;
                    continue;
                }

                // Lane header
                let header_rect = egui::Rect::from_min_size(
                    egui::pos2(available.left(), lane_top),
                    egui::vec2(available.width(), header_height),
                );
                let header_bg = crate::theme::resolve(
                    flame_cat_protocol::ThemeToken::LaneHeaderBackground,
                    self.theme_mode,
                );
                let header_text_color = crate::theme::resolve(
                    flame_cat_protocol::ThemeToken::LaneHeaderText,
                    self.theme_mode,
                );
                painter.rect_filled(header_rect, egui::CornerRadius::ZERO, header_bg);
                painter.text(
                    egui::pos2(available.left() + 6.0, lane_top + header_height / 2.0),
                    egui::Align2::LEFT_CENTER,
                    &lane.thread_name,
                    egui::FontId::proportional(11.0),
                    header_text_color,
                );

                // Lane content
                let content_top = lane_top + header_height;
                let content_rect = egui::Rect::from_min_size(
                    egui::pos2(available.left(), content_top),
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
                        egui::pos2(available.left(), content_top),
                        self.theme_mode,
                    );

                    // Hover tooltip
                    if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        if content_rect.contains(hover_pos) {
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

                y_offset += total_height + 2.0;
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
