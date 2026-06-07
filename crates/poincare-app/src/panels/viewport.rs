use eframe::egui;
use viewport_lib::{
    ButtonState, Modifiers, MouseButton, OrbitCameraController, ScrollUnits, ViewPreset,
    ViewportContext, ViewportEvent,
};

use crate::App;
use crate::CameraCommand;
use crate::picking::{ProbeHit, pick_probe, world_to_screen};

impl App {
    pub(crate) fn viewport(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        frame: &mut eframe::Frame,
    ) {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let doc_idx = self.active_document_idx;
        self.last_viewport_size = [rect.width().max(1.0) as u32, rect.height().max(1.0) as u32];
        let scrolled =
            response.hovered() && ui.ctx().input(|i| i.raw_scroll_delta != egui::Vec2::ZERO);
        if response.dragged() || response.drag_started() || scrolled {
            self.cancel_camera_animation();
        }

        if self.documents[self.active_document_idx].probe_mode {
            push_egui_events_filtered(
                &mut self.orbit_controller,
                ui,
                &response,
                rect,
                true,
                self.invert_scroll,
            );
        } else {
            push_egui_events(
                &mut self.orbit_controller,
                ui,
                &response,
                rect,
                self.invert_scroll,
            );
        }
        self.orbit_controller
            .apply_to_camera(&mut self.documents[self.active_document_idx].camera);

        // Axis indicator click (runs in both modes).
        let mut consumed_click = false;
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let vp_rect = [rect.left(), rect.top(), rect.width(), rect.height()];
                if let Some(hit) = viewport_lib::axes_indicator::hit_test(
                    [pos.x, pos.y],
                    vp_rect,
                    self.documents[self.active_document_idx].camera.orientation,
                ) {
                    // Z-up convention:
                    //   axis 0 (X): Right (+X) / Left (-X)
                    //   axis 1 (Y): Front (+Y) / Back (-Y)
                    //   axis 2 (Z): Top  (+Z, looking down at XY) / Bottom (-Z)
                    let i = hit.axis_index;
                    let preset = if self.last_axes_snap == Some((i, true)) {
                        self.last_axes_snap = Some((i, false));
                        match i {
                            0 => ViewPreset::Left,
                            1 => ViewPreset::Back,
                            _ => ViewPreset::Bottom,
                        }
                    } else {
                        self.last_axes_snap = Some((i, true));
                        match i {
                            0 => ViewPreset::Right,
                            1 => ViewPreset::Front,
                            _ => ViewPreset::Top,
                        }
                    };
                    self.run_camera_command(CameraCommand::ViewPreset(preset));
                    consumed_click = true;
                }
            }
        }

        if rect.height() > 0.0 {
            self.documents[self.active_document_idx]
                .camera
                .set_aspect_ratio(rect.width(), rect.height());
        }

        if self.documents[self.active_document_idx].probe_mode {
            self.probe_tick(ui, &response, rect);
            if response.clicked()
                && ui.input(|i| i.modifiers.command)
                && !probe_btn_hit_test(response.interact_pointer_pos(), rect)
            {
                if let Some(hit) = self.documents[self.active_document_idx].probe_hit.clone() {
                    self.documents[self.active_document_idx]
                        .pinned_probes
                        .push(hit);
                }
            }
        } else {
            self.documents[self.active_document_idx].probe_hit = None;
            let picked_plot = (!consumed_click
                && (response.clicked() || response.double_clicked()))
            .then(|| {
                response
                    .interact_pointer_pos()
                    .and_then(|pos| self.pick_plot_at(frame, pos, rect))
            })
            .flatten();
            if !consumed_click && response.clicked() {
                if let Some(plot_idx) = picked_plot {
                    self.set_selected_plot(self.active_document_idx, Some(plot_idx));
                    self.documents[self.active_document_idx].viewport_selection_hidden_for = None;
                    self.pending_focus_tab = Some(crate::dock::DockTab::PlotProperties);
                } else {
                    self.documents[self.active_document_idx].viewport_selection_hidden_for =
                        self.documents[self.active_document_idx].selected_plot;
                }
            }
            if response.double_clicked() && !consumed_click && picked_plot.is_some() {
                self.documents[self.active_document_idx].selected_plot = picked_plot;
                self.run_camera_command(CameraCommand::FrameSelected);
            }
        }

        let selected_plot_id = self.documents[doc_idx]
            .selected_plot
            .filter(|&idx| self.documents[doc_idx].viewport_selection_hidden_for != Some(idx))
            .map(|idx| (idx + 1) as u64);
        let mut frame_data = self.documents[self.active_document_idx]
            .scene
            .build_frame_with_selection(
                &self.documents[self.active_document_idx].camera,
                selected_plot_id,
                None,
            );
        frame_data.camera.viewport_size = [rect.width(), rect.height()];
        frame_data.camera.pixels_per_point = ctx.pixels_per_point();
        frame_data.viewport.show_grid = self.documents[self.active_document_idx]
            .axis_config
            .show_grid;
        frame_data.viewport.background_colour =
            Some(self.documents[self.active_document_idx].viewport_background);
        frame_data.effects.ground_plane = viewport_lib::GroundPlane {
            mode: self.documents[self.active_document_idx].ground_plane_mode,
            height: self.documents[self.active_document_idx].ground_plane_height,
            colour: self.documents[self.active_document_idx].ground_plane_color,
            tile_colour2: [
                self.documents[self.active_document_idx].ground_plane_color[0] * 0.82,
                self.documents[self.active_document_idx].ground_plane_color[1] * 0.82,
                self.documents[self.active_document_idx].ground_plane_color[2] * 0.82,
                self.documents[self.active_document_idx].ground_plane_color[3],
            ],
            tile_size: self.documents[self.active_document_idx].ground_plane_tile_size,
            shadow_colour: [0.0, 0.0, 0.0, 1.0],
            shadow_opacity: 0.35,
        };
        self.inject_frame_attachments(doc_idx, &mut frame_data);

        ui.painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                rect,
                super::viewport_callback::ViewportCallback { frame: frame_data },
            ));

        // Draw probe coordinate overlay.
        if let Some(hit) = &self.documents[self.active_document_idx].probe_hit {
            let view_proj = self.documents[self.active_document_idx]
                .camera
                .proj_matrix()
                * self.documents[self.active_document_idx]
                    .camera
                    .view_matrix();
            let viewport_size = glam::Vec2::new(rect.width(), rect.height());

            if let Some(hit_screen_local) = world_to_screen(hit.world_pos, view_proj, viewport_size)
            {
                let hit_screen = egui::Pos2::new(
                    rect.left() + hit_screen_local.x,
                    rect.top() + hit_screen_local.y,
                );
                let painter = ui.painter();

                if hit.snapped {
                    let gold = egui::Color32::from_rgb(255, 210, 40);
                    painter.circle_stroke(
                        hit_screen,
                        9.0,
                        egui::Stroke::new(2.0, egui::Color32::from_black_alpha(140)),
                    );
                    painter.circle_stroke(hit_screen, 8.0, egui::Stroke::new(2.0, gold));
                    painter.circle_filled(hit_screen, 2.5, gold);
                } else if hit.near_snap {
                    let amber = egui::Color32::from_rgb(255, 160, 30);
                    let r = 9.0_f32;
                    let segments = 8usize;
                    for k in 0..segments {
                        let start = (k as f32 / segments as f32) * std::f32::consts::TAU;
                        let end = start + 0.55_f32;
                        let steps = 4usize;
                        for s in 0..steps {
                            let a0 = start + (end - start) * s as f32 / steps as f32;
                            let a1 = start + (end - start) * (s + 1) as f32 / steps as f32;
                            let p0 = hit_screen + egui::Vec2::new(a0.cos() * r, a0.sin() * r);
                            let p1 = hit_screen + egui::Vec2::new(a1.cos() * r, a1.sin() * r);
                            painter.line_segment([p0, p1], egui::Stroke::new(2.0, amber));
                        }
                    }
                    painter.circle_filled(hit_screen, 2.0, amber);
                } else {
                    painter.circle_stroke(
                        hit_screen,
                        7.0,
                        egui::Stroke::new(2.0, egui::Color32::from_black_alpha(140)),
                    );
                    painter.circle_stroke(
                        hit_screen,
                        6.0,
                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                    );
                    painter.circle_filled(hit_screen, 2.0, egui::Color32::WHITE);
                }

                let n = hit.normal;
                let ref_vec = if n.dot(glam::Vec3::Y).abs() < 0.9 {
                    glam::Vec3::Y
                } else {
                    glam::Vec3::X
                };
                let t1 = (ref_vec - ref_vec.dot(n) * n).normalize_or_zero();
                let t2 = n.cross(t1).normalize_or_zero();

                let circle_r = if hit.snapped { 8.0_f32 } else { 6.0_f32 };
                let line_half_px = 28.0_f32;
                let gap_px = circle_r + 3.0;
                let delta = (self.scene_extent() * 0.02).max(0.01);

                let frame_spec: [(glam::Vec3, egui::Color32); 3] = [
                    (n, egui::Color32::from_rgba_unmultiplied(80, 140, 255, 200)),
                    (t1, egui::Color32::from_rgba_unmultiplied(220, 70, 70, 200)),
                    (t2, egui::Color32::from_rgba_unmultiplied(70, 200, 90, 200)),
                ];
                for (axis, color) in &frame_spec {
                    if axis.length_squared() < 1e-4 {
                        continue;
                    }
                    let s_plus =
                        world_to_screen(hit.world_pos + *axis * delta, view_proj, viewport_size);
                    let s_minus =
                        world_to_screen(hit.world_pos - *axis * delta, view_proj, viewport_size);
                    if let (Some(sp), Some(sm)) = (s_plus, s_minus) {
                        let dir = (sp - sm).normalize_or_zero();
                        if dir.length_squared() < 1e-4 {
                            continue;
                        }
                        let d = egui::Vec2::new(dir.x, dir.y);
                        let shadow = egui::Stroke::new(2.5, egui::Color32::from_black_alpha(100));
                        let fg = egui::Stroke::new(1.5, *color);
                        painter.line_segment(
                            [hit_screen - d * line_half_px, hit_screen - d * gap_px],
                            shadow,
                        );
                        painter.line_segment(
                            [hit_screen + d * gap_px, hit_screen + d * line_half_px],
                            shadow,
                        );
                        painter.line_segment(
                            [hit_screen - d * line_half_px, hit_screen - d * gap_px],
                            fg,
                        );
                        painter.line_segment(
                            [hit_screen + d * gap_px, hit_screen + d * line_half_px],
                            fg,
                        );
                    }
                }

                let p = hit.world_pos;
                let label = if hit.snapped {
                    format!("⊕ ({:.3}, {:.3}, {:.3})", p.x, p.y, p.z)
                } else if hit.near_snap {
                    format!("◎ ({:.3}, {:.3}, {:.3})", p.x, p.y, p.z)
                } else {
                    format!("({:.3}, {:.3}, {:.3})", p.x, p.y, p.z)
                };
                let label_fg = if hit.snapped {
                    egui::Color32::from_rgb(255, 210, 40)
                } else if hit.near_snap {
                    egui::Color32::from_rgb(255, 180, 60)
                } else {
                    egui::Color32::WHITE
                };
                let label_pos = hit_screen + egui::Vec2::new(12.0, -14.0);
                painter.text(
                    label_pos + egui::Vec2::new(1.0, 1.0),
                    egui::Align2::LEFT_BOTTOM,
                    &label,
                    egui::FontId::monospace(13.0),
                    egui::Color32::from_black_alpha(160),
                );
                painter.text(
                    label_pos,
                    egui::Align2::LEFT_BOTTOM,
                    &label,
                    egui::FontId::monospace(13.0),
                    label_fg,
                );
            }
        }
        if self.documents[self.active_document_idx].probe_mode {
            let view_proj = self.documents[self.active_document_idx]
                .camera
                .proj_matrix()
                * self.documents[self.active_document_idx]
                    .camera
                    .view_matrix();
            let viewport_size = glam::Vec2::new(rect.width(), rect.height());
            let painter = ui.painter();
            let pinned = &self.documents[self.active_document_idx].pinned_probes;
            for (idx, hit) in pinned.iter().enumerate() {
                if let Some(screen_local) = world_to_screen(hit.world_pos, view_proj, viewport_size)
                {
                    let screen =
                        egui::Pos2::new(rect.left() + screen_local.x, rect.top() + screen_local.y);
                    let color = egui::Color32::from_rgb(255, 210, 40);
                    painter.circle_filled(screen, 4.0, color);
                    painter.circle_stroke(
                        screen,
                        7.0,
                        egui::Stroke::new(1.5, egui::Color32::from_black_alpha(120)),
                    );
                    painter.text(
                        screen + egui::vec2(10.0, -10.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("#{}", idx + 1),
                        egui::FontId::monospace(12.0),
                        color,
                    );
                }
            }
            for pair in pinned.windows(2) {
                let a = pair[0].world_pos;
                let b = pair[1].world_pos;
                let Some(sa_local) = world_to_screen(a, view_proj, viewport_size) else {
                    continue;
                };
                let Some(sb_local) = world_to_screen(b, view_proj, viewport_size) else {
                    continue;
                };
                let sa = egui::Pos2::new(rect.left() + sa_local.x, rect.top() + sa_local.y);
                let sb = egui::Pos2::new(rect.left() + sb_local.x, rect.top() + sb_local.y);
                painter.line_segment(
                    [sa, sb],
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 210, 40)),
                );
                let mid = sa.lerp(sb, 0.5);
                let distance = a.distance(b);
                painter.text(
                    mid + egui::vec2(8.0, -8.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{distance:.3}"),
                    egui::FontId::monospace(12.0),
                    egui::Color32::from_rgb(255, 225, 120),
                );
            }
        }

        // Probe toggle button, top-right corner of the viewport.
        let btn_size = egui::vec2(54.0, 22.0);
        let btn_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - btn_size.x - 8.0, rect.top() + 8.0),
            btn_size,
        );
        let probe_btn = ui.put(
            btn_rect,
            egui::Button::new("Probe")
                .selected(self.documents[self.active_document_idx].probe_mode),
        );
        if probe_btn.clicked() {
            release_orbit_mouse_buttons(&mut self.orbit_controller);
            self.documents[self.active_document_idx].probe_mode =
                !self.documents[self.active_document_idx].probe_mode;
            if !self.documents[self.active_document_idx].probe_mode {
                self.documents[self.active_document_idx].probe_hit = None;
                self.documents[self.active_document_idx].last_probe_hit = None;
                self.documents[self.active_document_idx].probe_snap_locked = false;
                self.documents[self.active_document_idx].probe_snap_point = None;
            }
        }

        let has_selected_plot = self.documents[doc_idx].selected_plot.is_some();
        let frame_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - btn_size.x - 8.0, rect.top() + 36.0),
            btn_size,
        );
        let frame_btn = ui.add_enabled_ui(has_selected_plot, |ui| {
            ui.put(frame_rect, egui::Button::new("Frame"))
        });
        if frame_btn.inner.clicked() {
            self.run_camera_command(CameraCommand::FrameSelected);
        }

        if self.documents[doc_idx].probe_mode {
            let pin_rect = egui::Rect::from_min_size(
                egui::pos2(rect.right() - btn_size.x - 8.0, rect.top() + 64.0),
                btn_size,
            );
            let can_pin = self.documents[doc_idx].last_probe_hit.is_some();
            let pin_btn =
                ui.add_enabled_ui(can_pin, |ui| ui.put(pin_rect, egui::Button::new("Pin")));
            if pin_btn.inner.clicked() {
                if let Some(hit) = self.documents[doc_idx].last_probe_hit.clone() {
                    self.documents[doc_idx].pinned_probes.push(hit);
                }
            }

            let clear_rect = egui::Rect::from_min_size(
                egui::pos2(rect.right() - btn_size.x - 8.0, rect.top() + 92.0),
                btn_size,
            );
            let clear_btn = ui
                .add_enabled_ui(!self.documents[doc_idx].pinned_probes.is_empty(), |ui| {
                    ui.put(clear_rect, egui::Button::new("Clear"))
                });
            if clear_btn.inner.clicked() {
                self.documents[doc_idx].pinned_probes.clear();
            }
        }

        if self.documents[self.active_document_idx].probe_mode {
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        } else if response.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    pub(crate) fn probe_tick(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
    ) {
        const SNAP_RELEASE_PX: f32 = 6.0;
        const SNAP_ATTRACT_PX: f32 = 10.0;

        let cursor = match response.hover_pos() {
            Some(p) => p,
            None => {
                self.documents[self.active_document_idx].probe_hit = None;
                return;
            }
        };

        let local = glam::Vec2::new(cursor.x - rect.left(), cursor.y - rect.top());
        let viewport_size = glam::Vec2::new(rect.width(), rect.height());
        let view_proj = self.documents[self.active_document_idx]
            .camera
            .proj_matrix()
            * self.documents[self.active_document_idx]
                .camera
                .view_matrix();
        let view_proj_inv = view_proj.inverse();

        let (ray_orig, ray_dir) =
            viewport_lib::picking::screen_to_ray(local, viewport_size, view_proj_inv);

        let snap_world_radius = 0.05 * self.scene_extent();
        let pick_data = self.documents[self.active_document_idx].scene.probe_data();
        let (live_hit, surface_snap) = pick_probe(
            ray_orig,
            ray_dir,
            local,
            &pick_data,
            view_proj,
            viewport_size,
            snap_world_radius,
        );
        let live_pos = live_hit.map(|(p, _)| p);
        let live_normal = live_hit.map(|(_, n)| n);

        let mut nearest_candidate: Option<glam::Vec3> = None;
        let mut nearest_dist = SNAP_ATTRACT_PX;

        if let Some(snap_pt) = surface_snap {
            if let Some(sp) = world_to_screen(snap_pt, view_proj, viewport_size) {
                let d = sp.distance(local);
                if d < nearest_dist {
                    nearest_dist = d;
                    nearest_candidate = Some(snap_pt);
                }
            }
        }
        for &ipt in &self.documents[self.active_document_idx].intersection_cache {
            if let Some(sp) = world_to_screen(ipt, view_proj, viewport_size) {
                let d = sp.distance(local);
                if d < nearest_dist {
                    nearest_dist = d;
                    nearest_candidate = Some(ipt);
                }
            }
        }

        if self.documents[self.active_document_idx].probe_snap_locked {
            if let Some(locked_pos) = self.documents[self.active_document_idx].probe_snap_point {
                let screen_dist = world_to_screen(locked_pos, view_proj, viewport_size)
                    .map(|s| s.distance(local))
                    .unwrap_or(f32::MAX);
                if screen_dist < SNAP_RELEASE_PX {
                    if let Some(existing) = &self.documents[self.active_document_idx].probe_hit {
                        let normal = existing.normal;
                        self.documents[self.active_document_idx].probe_hit = Some(ProbeHit {
                            world_pos: locked_pos,
                            normal,
                            near_snap: false,
                            snapped: true,
                        });
                    }
                    let _ = ui;
                    return;
                }
                self.documents[self.active_document_idx].probe_snap_locked = false;
                self.documents[self.active_document_idx].probe_snap_point = None;
            }
        }

        if response.clicked() {
            if let Some(candidate) = nearest_candidate {
                self.documents[self.active_document_idx].probe_snap_locked = true;
                self.documents[self.active_document_idx].probe_snap_point = Some(candidate);
                let normal = live_normal.unwrap_or(glam::Vec3::Z);
                self.documents[self.active_document_idx].probe_hit = Some(ProbeHit {
                    world_pos: candidate,
                    normal,
                    near_snap: false,
                    snapped: true,
                });
                let _ = ui;
                return;
            }
        }

        let world_pos = live_pos.unwrap_or_else(|| nearest_candidate.unwrap_or(glam::Vec3::ZERO));
        let normal = live_normal.unwrap_or(glam::Vec3::Z);
        self.documents[self.active_document_idx].probe_hit =
            if live_pos.is_some() || nearest_candidate.is_some() {
                Some(ProbeHit {
                    world_pos,
                    normal,
                    near_snap: nearest_candidate.is_some(),
                    snapped: false,
                })
            } else {
                None
            };
        if let Some(hit) = self.documents[self.active_document_idx].probe_hit.clone() {
            self.documents[self.active_document_idx].last_probe_hit = Some(hit);
        }
        let _ = ui;
    }

    pub(crate) fn scene_extent(&self) -> f32 {
        self.documents[self.active_document_idx].scene_extent()
    }

    fn pick_plot_at(
        &self,
        frame: &mut eframe::Frame,
        cursor: egui::Pos2,
        rect: egui::Rect,
    ) -> Option<usize> {
        let local = glam::Vec2::new(cursor.x - rect.left(), cursor.y - rect.top());
        let viewport_size = glam::Vec2::new(rect.width(), rect.height());
        let view_proj = self.documents[self.active_document_idx]
            .camera
            .proj_matrix()
            * self.documents[self.active_document_idx]
                .camera
                .view_matrix();
        let render_state = frame.wgpu_render_state()?;
        let renderer_guard = render_state.renderer.read();
        let renderer = renderer_guard
            .callback_resources
            .get::<viewport_lib::ViewportRenderer>()?;
        let hit = renderer.pick(
            local,
            viewport_size,
            view_proj,
            viewport_lib::PickMask::OBJECT,
        )?;
        pick_id_to_plot_index(hit.id)
    }
}

fn pick_id_to_plot_index(pick_id: u64) -> Option<usize> {
    pick_id.checked_sub(1).map(|idx| idx as usize)
}

fn probe_btn_hit_test(pointer_pos: Option<egui::Pos2>, rect: egui::Rect) -> bool {
    let Some(pointer_pos) = pointer_pos else {
        return false;
    };
    let btn_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 54.0 - 8.0, rect.top() + 8.0),
        egui::vec2(54.0, 22.0),
    );
    btn_rect.contains(pointer_pos)
}

/// Like `push_egui_events` but optionally skips left-button events (for probe mode).
pub(crate) fn push_egui_events_filtered(
    controller: &mut OrbitCameraController,
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
    skip_left: bool,
    invert_scroll: bool,
) {
    use viewport_lib::KeyCode;

    let hovered = response.hovered();
    controller.begin_frame(ViewportContext {
        hovered,
        focused: hovered,
        viewport_size: [rect.width(), rect.height()],
    });

    ui.input(|i| {
        let pointer_over_viewport = response.hovered();
        let owns_pointer =
            pointer_over_viewport || response.dragged() || response.is_pointer_button_down_on();

        let mods = Modifiers {
            alt: i.modifiers.alt,
            shift: i.modifiers.shift,
            ctrl: i.modifiers.command,
        };
        controller.push_event(ViewportEvent::ModifiersChanged(mods));

        if owns_pointer {
            if let Some(pos) = i.pointer.interact_pos() {
                let local = glam::Vec2::new(pos.x - rect.left(), pos.y - rect.top());
                controller.push_event(ViewportEvent::PointerMoved { position: local });
            }
        }

        for event in &i.events {
            match event {
                egui::Event::PointerButton {
                    pos: _,
                    button,
                    pressed,
                    ..
                } => {
                    if !pointer_over_viewport && !owns_pointer {
                        continue;
                    }
                    if skip_left && *button == egui::PointerButton::Primary {
                        continue;
                    }
                    let vp_button = match button {
                        egui::PointerButton::Primary => MouseButton::Left,
                        egui::PointerButton::Secondary => MouseButton::Right,
                        egui::PointerButton::Middle => MouseButton::Middle,
                        _ => continue,
                    };
                    let state = if *pressed {
                        ButtonState::Pressed
                    } else {
                        ButtonState::Released
                    };
                    controller.push_event(ViewportEvent::MouseButton {
                        button: vp_button,
                        state,
                    });
                }
                egui::Event::MouseWheel { delta, .. } => {
                    if !pointer_over_viewport {
                        continue;
                    }
                    let scroll_sign = if invert_scroll { -1.0 } else { 1.0 };
                    controller.push_event(ViewportEvent::Wheel {
                        delta: glam::Vec2::new(delta.x * scroll_sign, delta.y * scroll_sign),
                        units: ScrollUnits::Pixels,
                    });
                }
                egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    ..
                } => {
                    if !owns_pointer {
                        continue;
                    }
                    let kc = match key {
                        egui::Key::F => Some(KeyCode::F),
                        egui::Key::I => Some(KeyCode::I),
                        egui::Key::O => Some(KeyCode::O),
                        egui::Key::T => Some(KeyCode::T),
                        egui::Key::Tab => Some(KeyCode::Tab),
                        egui::Key::Escape => Some(KeyCode::Escape),
                        _ => None,
                    };
                    if let Some(key_code) = kc {
                        let state = if *pressed {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        };
                        controller.push_event(ViewportEvent::Key {
                            key: key_code,
                            state,
                            repeat: *repeat,
                        });
                    }
                }
                _ => {}
            }
        }
    });
}

fn release_orbit_mouse_buttons(controller: &mut OrbitCameraController) {
    for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
        controller.push_event(ViewportEvent::MouseButton {
            button,
            state: ButtonState::Released,
        });
    }
}

/// Translate egui events into `ViewportEvent`s and push them to the controller.
pub(crate) fn push_egui_events(
    controller: &mut OrbitCameraController,
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
    invert_scroll: bool,
) {
    use viewport_lib::KeyCode;

    let hovered = response.hovered();
    controller.begin_frame(ViewportContext {
        hovered,
        focused: hovered,
        viewport_size: [rect.width(), rect.height()],
    });

    ui.input(|i| {
        let pointer_over_viewport = response.hovered();
        let owns_pointer =
            pointer_over_viewport || response.dragged() || response.is_pointer_button_down_on();

        let mods = Modifiers {
            alt: i.modifiers.alt,
            shift: i.modifiers.shift,
            ctrl: i.modifiers.command,
        };
        controller.push_event(ViewportEvent::ModifiersChanged(mods));

        if owns_pointer {
            if let Some(pos) = i.pointer.interact_pos() {
                let local = glam::Vec2::new(pos.x - rect.left(), pos.y - rect.top());
                controller.push_event(ViewportEvent::PointerMoved { position: local });
            }
        }

        for event in &i.events {
            match event {
                egui::Event::PointerButton {
                    pos: _,
                    button,
                    pressed,
                    ..
                } => {
                    if !pointer_over_viewport && !owns_pointer {
                        continue;
                    }
                    let vp_button = match button {
                        egui::PointerButton::Primary => MouseButton::Left,
                        egui::PointerButton::Secondary => MouseButton::Right,
                        egui::PointerButton::Middle => MouseButton::Middle,
                        _ => continue,
                    };
                    let state = if *pressed {
                        ButtonState::Pressed
                    } else {
                        ButtonState::Released
                    };
                    controller.push_event(ViewportEvent::MouseButton {
                        button: vp_button,
                        state,
                    });
                }
                egui::Event::MouseWheel { delta, .. } => {
                    if !pointer_over_viewport {
                        continue;
                    }
                    let scroll_sign = if invert_scroll { -1.0 } else { 1.0 };
                    controller.push_event(ViewportEvent::Wheel {
                        delta: glam::Vec2::new(delta.x * scroll_sign, delta.y * scroll_sign),
                        units: ScrollUnits::Pixels,
                    });
                }
                egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    ..
                } => {
                    if !owns_pointer {
                        continue;
                    }
                    let kc = match key {
                        egui::Key::A => Some(KeyCode::A),
                        egui::Key::D => Some(KeyCode::D),
                        egui::Key::E => Some(KeyCode::E),
                        egui::Key::F => Some(KeyCode::F),
                        egui::Key::G => Some(KeyCode::G),
                        egui::Key::I => Some(KeyCode::I),
                        egui::Key::O => Some(KeyCode::O),
                        egui::Key::R => Some(KeyCode::R),
                        egui::Key::S => Some(KeyCode::S),
                        egui::Key::T => Some(KeyCode::T),
                        egui::Key::W => Some(KeyCode::W),
                        egui::Key::X => Some(KeyCode::X),
                        egui::Key::Y => Some(KeyCode::Y),
                        egui::Key::Z => Some(KeyCode::Z),
                        egui::Key::Tab => Some(KeyCode::Tab),
                        egui::Key::Enter => Some(KeyCode::Enter),
                        egui::Key::Escape => Some(KeyCode::Escape),
                        _ => None,
                    };
                    if let Some(key_code) = kc {
                        let state = if *pressed {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        };
                        controller.push_event(ViewportEvent::Key {
                            key: key_code,
                            state,
                            repeat: *repeat,
                        });
                    }
                }
                _ => {}
            }
        }
    });
}
