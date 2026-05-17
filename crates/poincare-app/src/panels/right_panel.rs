use eframe::egui;
use viewport_lib::{AttributeKind, Easing, Projection, ViewPreset};

use crate::App;
use crate::CameraCommand;
use crate::InspectorTab;
use crate::dock::DockTab;
use crate::document::{ExportFormat, ExportMode, default_export_dir, ensure_export_dir_exists, export_mode_for_format};
use crate::plot::analysis::{
    PointAnnotation, SliceAxis, default_slice_position, make_arrow_annotation,
    make_point_annotations,
};
use crate::plot::entry::PlotEntry;
use crate::plot::kind::{PlotKind, StyleCaps, evenly_spaced_isovalues};
use crate::ui::domain_editor::{edit_domain, edit_resolution};
use crate::ui::expr_params::show_expression_params;
use crate::ui::style_editor::{
    align_surface_colour_for_lic, edit_plot_style_basic, edit_plot_surface_settings,
};

impl App {
    pub(crate) fn bottom_inspector(&mut self, ui: &mut egui::Ui) {
        let doc_idx = self.active_document_idx;
        let selected_plot = self.documents[doc_idx].selected_plot;

        ui.horizontal(|ui| {
            if let Some(index) = selected_plot {
                if let Some(plot) = self.documents[doc_idx].plots.get(index) {
                    let color = self.representative_plot_color(plot);
                    let (dot_rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot_rect.center(), 5.0, color);
                    ui.label(egui::RichText::new(&plot.name).strong());
                }
            } else {
                ui.label(egui::RichText::new("No plot selected").weak());
            }

            ui.separator();
            ui.selectable_value(&mut self.inspector_tab, InspectorTab::Domain, "Domain");
            ui.selectable_value(&mut self.inspector_tab, InspectorTab::Style, "Style");
            ui.selectable_value(&mut self.inspector_tab, InspectorTab::Surface, "Surface");
            ui.selectable_value(&mut self.inspector_tab, InspectorTab::Analysis, "Analysis");
        });
        ui.separator();

        let Some(index) = selected_plot else {
            ui.label("Select a plot to edit its domain, style, and surface settings.");
            return;
        };

        {
            let plot_count = self.documents[doc_idx].plots.len();
            self.documents[doc_idx]
                .sweep_config
                .resize_with(plot_count, Default::default);
        }

        let inspector_tab = self.inspector_tab;
        let mut selected_dirty = false;

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let doc = &mut self.documents[doc_idx];
                let plot = &mut doc.plots[index];
                let sweep_map = &mut doc.sweep_config[index];

                match inspector_tab {
                    InspectorTab::Domain => {
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                selected_dirty |=
                                    edit_domain(ui, &mut plot.domain, plot.kind.domain_labels());
                                ui.add_space(8.0);
                                let resolution_label = if plot.kind.uses_seed_resolution() {
                                    "Seed Resolution"
                                } else {
                                    "Resolution"
                                };
                                ui.label(resolution_label);
                                selected_dirty |= edit_resolution(
                                    ui,
                                    &mut plot.resolution,
                                    plot.kind.uses_resolution(),
                                );
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                selected_dirty |= show_expression_params(
                                    ui,
                                    &mut plot.kind,
                                    &mut self.slider_dragging,
                                    &mut self.eq_editor,
                                    sweep_map,
                                );
                            });
                        });
                    }
                    InspectorTab::Style => {
                        selected_dirty |=
                            edit_plot_style_basic(ui, &mut plot.style, plot.kind.style_caps());

                        if let PlotKind::ContouredSurface {
                            contour_values,
                            contour_style,
                        } = &mut plot.kind
                        {
                            ui.add_space(10.0);
                            ui.separator();
                            ui.label("Contours");
                            let mut contour_count = contour_values.len() as u32;
                            if ui
                                .add(
                                    egui::Slider::new(&mut contour_count, 1..=20)
                                        .text("Line Count"),
                                )
                                .changed()
                            {
                                *contour_values = evenly_spaced_isovalues(contour_count as usize);
                                selected_dirty = true;
                            }
                            selected_dirty |= edit_plot_style_basic(
                                ui,
                                contour_style,
                                StyleCaps {
                                    mesh: false,
                                    line: true,
                                    point: false,
                                    glyph: false,
                                },
                            );
                        }
                    }
                    InspectorTab::Surface => {
                        selected_dirty |=
                            edit_plot_surface_settings(ui, &mut plot.style, plot.kind.style_caps());
                        selected_dirty |= align_surface_colour_for_lic(&mut plot.style);
                    }
                    InspectorTab::Analysis => {
                        let selected_index = index;
                        let _ = plot;
                        let _ = sweep_map;
                        self.analysis_inspector(ui, doc_idx, selected_index);
                    }
                }
            });

        if selected_dirty {
            self.mark_dirty();
            ui.add_space(6.0);
            ui.colored_label(egui::Color32::YELLOW, "Pending scene rebuild");
        }
    }

    pub(crate) fn camera_inspector(&mut self, ui: &mut egui::Ui) {
        let camera = &self.documents[self.active_document_idx].camera;
        ui.label(egui::RichText::new("Viewport Camera").strong());
        ui.label(
            egui::RichText::new(format!(
                "Center ({:.2}, {:.2}, {:.2})  Distance {:.2}",
                camera.center.x, camera.center.y, camera.center.z, camera.distance
            ))
            .small()
            .weak(),
        );
        ui.separator();

        ui.label("Views");
        egui::Grid::new("camera_view_presets")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                for (label, preset) in [
                    ("Front", ViewPreset::Front),
                    ("Back", ViewPreset::Back),
                    ("Left", ViewPreset::Left),
                    ("Right", ViewPreset::Right),
                    ("Top", ViewPreset::Top),
                    ("Bottom", ViewPreset::Bottom),
                    ("Iso", ViewPreset::Isometric),
                ] {
                    if ui.button(label).clicked() {
                        self.run_camera_command(CameraCommand::ViewPreset(preset));
                    }
                }
            });

        ui.add_space(10.0);
        ui.separator();
        ui.label("Frame");
        ui.horizontal(|ui| {
            if ui.button("Frame All").clicked() {
                self.run_camera_command(CameraCommand::FrameAll);
            }
            if ui.button("Frame Selected").clicked() {
                self.run_camera_command(CameraCommand::FrameSelected);
            }
            if ui.button("Reset View").clicked() {
                self.run_camera_command(CameraCommand::ResetView);
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.label("Projection");
        let current_projection = self.documents[self.active_document_idx].camera.projection;
        ui.horizontal(|ui| {
            if ui
                .radio(current_projection == Projection::Perspective, "Perspective")
                .clicked()
            {
                self.run_camera_command(CameraCommand::SetProjection(Projection::Perspective));
            }
            if ui
                .radio(
                    current_projection == Projection::Orthographic,
                    "Orthographic",
                )
                .clicked()
            {
                self.run_camera_command(CameraCommand::SetProjection(Projection::Orthographic));
            }
        });
        if current_projection == Projection::Perspective {
            let mut fov_deg = self.documents[self.active_document_idx]
                .camera
                .fov_y
                .to_degrees();
            if ui
                .add(egui::Slider::new(&mut fov_deg, 20.0_f32..=120.0_f32).text("FOV"))
                .changed()
            {
                self.cancel_camera_animation();
                self.documents[self.active_document_idx]
                    .camera
                    .set_fov_y(fov_deg.to_radians());
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label("Animation");
        ui.checkbox(&mut self.camera_animations_enabled, "Animate View Changes");
        ui.label(
            egui::RichText::new(
                "Applies to preset views, framing actions, reset view, and saved-view recall.",
            )
            .small()
            .weak(),
        );
        ui.add_enabled_ui(self.camera_animations_enabled, |ui| {
            ui.add(
                egui::Slider::new(&mut self.camera_animation_duration, 0.1_f32..=2.0_f32)
                    .text("Duration"),
            );
            egui::ComboBox::from_label("Easing")
                .selected_text(match self.camera_animation_easing {
                    Easing::Linear => "Linear",
                    Easing::EaseOutCubic => "Ease Out",
                    Easing::EaseInOutCubic => "Ease In Out",
                    _ => "Custom",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.camera_animation_easing,
                        Easing::Linear,
                        "Linear",
                    );
                    ui.selectable_value(
                        &mut self.camera_animation_easing,
                        Easing::EaseOutCubic,
                        "Ease Out",
                    );
                    ui.selectable_value(
                        &mut self.camera_animation_easing,
                        Easing::EaseInOutCubic,
                        "Ease In Out",
                    );
                });
        });

        ui.add_space(10.0);
        ui.separator();
        ui.label("Saved Views");
        ui.horizontal(|ui| {
            if ui.button("+").clicked() {
                self.record_undo_point();
                self.add_saved_view();
                self.mark_non_scene_dirty();
            }
            ui.label(
                egui::RichText::new("Add the current camera as a new saved view.")
                    .small()
                    .weak(),
            );
        });
        let mut remove_view = None;
        let mut views_changed = false;
        for slot in 0..self.documents[self.active_document_idx].saved_views.len() {
            ui.horizontal(|ui| {
                let view = &mut self.documents[self.active_document_idx].saved_views[slot];
                views_changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut view.name)
                        .desired_width(120.0)
                        .hint_text("View name"),
                    )
                    .changed();
                if ui.button("Recall").clicked() {
                    self.run_camera_command(CameraCommand::RecallSlot(slot));
                }
                if ui.button("Overwrite").clicked() {
                    self.run_camera_command(CameraCommand::SaveSlot(slot));
                }
                if ui.button("×").clicked() {
                    remove_view = Some(slot);
                }
            });
        }
        if views_changed {
            self.mark_non_scene_dirty();
        }
        if let Some(slot) = remove_view {
            self.record_undo_point();
            self.documents[self.active_document_idx].saved_views.remove(slot);
            self.documents[self.active_document_idx].camera_track_playing = false;
            self.mark_non_scene_dirty();
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label("Track");
        let segment_changed = ui
            .add(
            egui::Slider::new(
                &mut self.documents[self.active_document_idx].camera_track_segment_duration,
                0.25_f32..=10.0_f32,
            )
            .text("Seconds per view"),
        )
        .changed();
        if segment_changed {
            self.mark_non_scene_dirty();
        }
        let track = self.build_saved_view_track();
        let duration = track.duration();
        ui.horizontal(|ui| {
            let can_play = track.len() >= 2;
            let playing = self.documents[self.active_document_idx].camera_track_playing;
            let label = if playing { "Stop" } else { "Play" };
            if ui.add_enabled(can_play, egui::Button::new(label)).clicked() {
                if playing {
                    self.documents[self.active_document_idx].camera_track_playing = false;
                } else {
                    self.documents[self.active_document_idx].camera_track_t = 0.0;
                    self.documents[self.active_document_idx].camera_track_playing = true;
                }
            }
            if ui.button("Rewind").clicked() {
                self.documents[self.active_document_idx].camera_track_t = 0.0;
                self.documents[self.active_document_idx].camera_track_playing = false;
                self.apply_saved_view_track_sample(0.0);
            }
        });
        if duration > 0.0 {
            let mut t = self.documents[self.active_document_idx].camera_track_t as f32;
            if ui
                .add(egui::Slider::new(&mut t, 0.0..=(duration as f32)).text("Track Position"))
                .changed()
            {
                self.documents[self.active_document_idx].camera_track_t = t as f64;
                self.documents[self.active_document_idx].camera_track_playing = false;
                self.apply_saved_view_track_sample(t as f64);
            }
        }
        ui.label(
            egui::RichText::new(format!(
                "{} views, {:.1}s track duration",
                self.documents[self.active_document_idx].saved_views.len(),
                duration
            ))
            .small()
            .weak(),
        );
    }

    pub(crate) fn export_inspector(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let export_running = self.export_job.is_some();
        ui.label(egui::RichText::new("Export").strong());
        ui.label(
            egui::RichText::new("PNG exports the current camera. GIF and MP4 follow the saved-view track and require ffmpeg.")
                .small()
                .weak(),
        );
        ui.separator();
        ui.add_enabled_ui(!export_running, |ui| {
            let current_format = self.documents[self.active_document_idx].export_format;
            let mut mode = export_mode_for_format(current_format);
            let (mut dir, mut filename) =
                crate::split_export_path(&self.documents[self.active_document_idx].export_path, current_format);
            ui.horizontal(|ui| {
                ui.label("Mode");
                let image_clicked = ui
                    .selectable_value(&mut mode, ExportMode::Image, "Image")
                    .clicked();
                let video_clicked = ui
                    .selectable_value(&mut mode, ExportMode::Video, "Video")
                    .clicked();
                if image_clicked && self.documents[self.active_document_idx].export_format != ExportFormat::Png {
                    self.documents[self.active_document_idx].export_format = ExportFormat::Png;
                    dir = default_export_dir(ExportMode::Image);
                    let _ = ensure_export_dir_exists(&dir);
                    filename = "poincare-export.png".to_string();
                }
                if video_clicked && self.documents[self.active_document_idx].export_format == ExportFormat::Png {
                    self.documents[self.active_document_idx].export_format = ExportFormat::Mp4;
                    dir = default_export_dir(ExportMode::Video);
                    let _ = ensure_export_dir_exists(&dir);
                    filename = "poincare-export.mp4".to_string();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Directory");
                let mut dir_text = dir.to_string_lossy().into_owned();
                ui.add(
                    egui::TextEdit::singleline(&mut dir_text)
                        .desired_width(320.0),
                );
                if ui.button("Choose…").clicked() {
                    let start_dir = if dir.as_os_str().is_empty() {
                        default_export_dir(mode)
                    } else {
                        dir.clone()
                    };
                    if let Some(chosen) = rfd::FileDialog::new().set_directory(start_dir).pick_folder() {
                        dir = chosen;
                    }
                }
                if dir_text != dir.to_string_lossy() {
                    dir = std::path::PathBuf::from(dir_text);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Filename");
                ui.add(
                    egui::TextEdit::singleline(&mut filename)
                        .desired_width(220.0),
                );
            });
            let full_export_path =
                crate::export_path_from_parts(&dir, &filename, self.documents[self.active_document_idx].export_format);
            self.documents[self.active_document_idx].export_path =
                full_export_path.to_string_lossy().into_owned();
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.documents[self.active_document_idx].export_width)
                        .speed(1)
                        .range(256..=8192)
                        .prefix("W "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.documents[self.active_document_idx].export_height)
                        .speed(1)
                        .range(256..=8192)
                        .prefix("H "),
                );
            });
            if mode == ExportMode::Video {
                egui::ComboBox::from_label("Video Format")
                    .selected_text(match self.documents[self.active_document_idx].export_format {
                        ExportFormat::Gif => "GIF",
                        ExportFormat::Mp4 => "MP4",
                        ExportFormat::Png => "MP4",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.documents[self.active_document_idx].export_format,
                            ExportFormat::Gif,
                            "GIF",
                        );
                        ui.selectable_value(
                            &mut self.documents[self.active_document_idx].export_format,
                            ExportFormat::Mp4,
                            "MP4",
                        );
                    });
            }
            ui.label(
                egui::RichText::new(match self.documents[self.active_document_idx].export_format {
                    ExportFormat::Png => "Images default to ~/Pictures/Poincare and use .png files.",
                    ExportFormat::Gif => "Videos default to ~/Videos/Poincare and use .gif files.",
                    ExportFormat::Mp4 => "Videos default to ~/Videos/Poincare and use .mp4 files.",
                })
                .small()
                .weak(),
            );
            ui.add(
                egui::DragValue::new(&mut self.documents[self.active_document_idx].export_fps)
                    .speed(1)
                    .range(1..=120)
                    .prefix("FPS "),
            );
            if self.documents[self.active_document_idx].export_format != ExportFormat::Png {
                ui.add(
                    egui::Slider::new(
                        &mut self.documents[self.active_document_idx].camera_track_segment_duration,
                        0.25_f32..=10.0_f32,
                    )
                    .text("Track seconds per view"),
                );
            }
            ui.horizontal(|ui| {
                let action_label = if mode == ExportMode::Image {
                    "Export Image"
                } else {
                    "Export Video"
                };
                if ui.button(action_label).clicked() {
                    self.pending_focus_tab = Some(DockTab::ExportProperties);
                    if let Err(err) = ensure_export_dir_exists(&dir) {
                        self.documents[self.active_document_idx].export_status = err;
                        self.documents[self.active_document_idx].export_progress = None;
                        return;
                    }
                    self.rebuild_scene(frame);
                    if mode == ExportMode::Image {
                        self.export_png(frame);
                    } else {
                        self.export_animation(frame);
                    }
                }
            });
        });
        if export_running {
            ui.add_space(6.0);
            if let Some(progress) = self.documents[self.active_document_idx].export_progress {
                ui.add(
                    egui::ProgressBar::new(progress)
                        .desired_width(260.0)
                        .show_percentage(),
                );
            } else {
                ui.add(egui::Spinner::new());
                ui.label(egui::RichText::new("Pending...").small().weak());
            }
        }
        if !self.documents[self.active_document_idx]
            .export_status
            .is_empty()
        {
            ui.label(&self.documents[self.active_document_idx].export_status);
        }
    }

    fn analysis_inspector(&mut self, ui: &mut egui::Ui, doc_idx: usize, plot_idx: usize) {
        let selected = self.documents[doc_idx].plots[plot_idx].clone();
        ui.label("Derived Tools");
        match &selected.kind {
            PlotKind::ExprVolume { .. } | PlotKind::ExprIsosurface { .. } => {
                ui.horizontal(|ui| {
                    if ui.button("Add Z Slice").clicked()
                        && let Some(plot) =
                            self.make_scalar_slice_plot(&selected, SliceAxis::Z)
                    {
                        self.push_analysis_plot(doc_idx, plot);
                    }
                    if ui.button("Add Gradient Field").clicked()
                        && let Some(plot) = self.make_gradient_plot(&selected)
                    {
                        self.push_analysis_plot(doc_idx, plot);
                    }
                });
                ui.label(egui::RichText::new("Slices include contour cross-sections.").small().weak());
            }
            PlotKind::ExprVectorField { .. } => {
                ui.horizontal(|ui| {
                    if ui.button("Add Z Vector Slice").clicked()
                        && let Some(plot) =
                            self.make_vector_slice_plot(&selected, SliceAxis::Z)
                    {
                        self.push_analysis_plot(doc_idx, plot);
                    }
                    if ui.button("Add Divergence Volume").clicked()
                        && let Some(plot) = self.make_divergence_plot(&selected)
                    {
                        self.push_analysis_plot(doc_idx, plot);
                    }
                    if ui.button("Add Curl Field").clicked()
                        && let Some(plot) = self.make_curl_plot(&selected)
                    {
                        self.push_analysis_plot(doc_idx, plot);
                    }
                });
            }
            _ => {
                ui.label(egui::RichText::new(
                    "Select a scalar field or vector field plot to generate slices or derived fields.",
                ).small().weak());
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label("Annotations");

        if let Some(hit) = self.documents[doc_idx].last_probe_hit.clone() {
            ui.horizontal(|ui| {
                if ui.button("Annotate Probe Point").clicked() {
                    self.push_analysis_plot(
                        doc_idx,
                        PlotEntry {
                            name: "Probe Annotation".to_string(),
                            visible: true,
                            domain: self.documents[doc_idx].plots[plot_idx].domain.clone(),
                            resolution: self.documents[doc_idx].plots[plot_idx].resolution,
                            style: poincare_lib::PlotStyle {
                                colour_mode: poincare_lib::ColourMode::Solid([1.0, 0.95, 0.35, 1.0]),
                                point_size: 10.0,
                                ..poincare_lib::PlotStyle::default()
                            },
                            kind: PlotKind::PointAnnotations {
                                points: vec![PointAnnotation {
                                    position: hit.world_pos.to_array(),
                                    label: "Probe".to_string(),
                                }],
                                show_labels: true,
                            },
                        },
                    );
                }
                if ui.button("Annotate Probe Direction").clicked() {
                    self.push_analysis_plot(
                        doc_idx,
                        PlotEntry {
                            name: "Probe Direction".to_string(),
                            visible: true,
                            domain: self.documents[doc_idx].plots[plot_idx].domain.clone(),
                            resolution: self.documents[doc_idx].plots[plot_idx].resolution,
                            style: poincare_lib::PlotStyle {
                                colour_mode: poincare_lib::ColourMode::Solid([0.35, 0.85, 1.0, 1.0]),
                                glyph_scale: 1.0,
                                shading: poincare_lib::ShadingMode::Unlit,
                                ..poincare_lib::PlotStyle::default()
                            },
                            kind: PlotKind::ArrowAnnotations {
                                arrows: vec![make_arrow_annotation(
                                    hit.world_pos,
                                    hit.normal,
                                    if hit.snapped { "Snapped Direction" } else { "Probe Direction" },
                                )],
                                show_labels: true,
                            },
                        },
                    );
                }
            });
        } else {
            ui.label(egui::RichText::new("Use probe mode to create point, normal, or tangent annotations.").small().weak());
        }

        if !self.documents[doc_idx].pinned_probes.is_empty()
            && ui.button("Create Pinned Probe Samples").clicked()
        {
            let points = self.documents[doc_idx]
                .pinned_probes
                .iter()
                .map(|hit| hit.world_pos.to_array())
                .collect::<Vec<_>>();
            self.push_analysis_plot(
                doc_idx,
                PlotEntry {
                    name: "Pinned Probe Samples".to_string(),
                    visible: true,
                    domain: self.documents[doc_idx].plots[plot_idx].domain.clone(),
                    resolution: self.documents[doc_idx].plots[plot_idx].resolution,
                    style: poincare_lib::PlotStyle {
                        colour_mode: poincare_lib::ColourMode::Solid([1.0, 0.6, 0.2, 1.0]),
                        point_size: 8.0,
                        ..poincare_lib::PlotStyle::default()
                    },
                    kind: PlotKind::PointAnnotations {
                        points: make_point_annotations(&points, "Probe"),
                        show_labels: true,
                    },
                },
            );
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label("Intersections");
        if self.documents[doc_idx].intersection_cache.is_empty() {
            ui.label(egui::RichText::new("No cached curve intersections in the current scene.").small().weak());
        } else if ui.button("Create Intersection Markers").clicked() {
            let points = self.documents[doc_idx]
                .intersection_cache
                .iter()
                .map(|point| point.to_array())
                .collect::<Vec<_>>();
            self.push_analysis_plot(
                doc_idx,
                PlotEntry {
                    name: "Intersection Markers".to_string(),
                    visible: true,
                    domain: self.documents[doc_idx].plots[plot_idx].domain.clone(),
                    resolution: self.documents[doc_idx].plots[plot_idx].resolution,
                    style: poincare_lib::PlotStyle {
                        colour_mode: poincare_lib::ColourMode::Solid([0.9, 0.25, 0.25, 1.0]),
                        point_size: 9.0,
                        ..poincare_lib::PlotStyle::default()
                    },
                    kind: PlotKind::PointAnnotations {
                        points: make_point_annotations(&points, "Intersection"),
                        show_labels: true,
                    },
                },
            );
        }
    }

    fn push_analysis_plot(&mut self, doc_idx: usize, plot: PlotEntry) {
        self.documents[doc_idx].plots.push(plot);
        self.documents[doc_idx].selected_plot = Some(self.documents[doc_idx].plots.len() - 1);
        self.mark_dirty();
    }

    fn make_scalar_slice_plot(&self, source: &PlotEntry, axis: SliceAxis) -> Option<PlotEntry> {
        let (expression, parameters) = match &source.kind {
            PlotKind::ExprVolume { expression, parameters, .. }
            | PlotKind::ExprIsosurface { expression, parameters, .. } => {
                (expression.clone(), parameters.clone())
            }
            _ => return None,
        };
        Some(PlotEntry {
            name: format!("{} Slice {}", axis.label(), source.name),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: poincare_lib::PlotStyle {
                colour_mode: poincare_lib::ColourMode::ByAttribute {
                    name: "value".to_string(),
                    kind: AttributeKind::Vertex,
                },
                two_sided: true,
                ..source.style.clone()
            },
            kind: PlotKind::ScalarSlice {
                expression,
                parameters,
                axis,
                position: default_slice_position(&source.domain, axis),
                contour_values: evenly_spaced_isovalues(8),
                contour_style: poincare_lib::PlotStyle {
                    colour_mode: poincare_lib::ColourMode::Solid([1.0, 0.95, 0.35, 1.0]),
                    line_width: 2.0,
                    ..poincare_lib::PlotStyle::default()
                },
            },
        })
    }

    fn make_vector_slice_plot(&self, source: &PlotEntry, axis: SliceAxis) -> Option<PlotEntry> {
        let (expression, parameters) = match &source.kind {
            PlotKind::ExprVectorField { expression, parameters } => {
                (expression.clone(), parameters.clone())
            }
            _ => return None,
        };
        Some(PlotEntry {
            name: format!("{} Slice {}", axis.label(), source.name),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: source.style.clone(),
            kind: PlotKind::VectorSlice {
                expression,
                parameters,
                axis,
                position: default_slice_position(&source.domain, axis),
            },
        })
    }

    fn make_gradient_plot(&self, source: &PlotEntry) -> Option<PlotEntry> {
        let (expression, parameters) = match &source.kind {
            PlotKind::ExprVolume { expression, parameters, .. }
            | PlotKind::ExprIsosurface { expression, parameters, .. } => {
                (expression.clone(), parameters.clone())
            }
            _ => return None,
        };
        Some(PlotEntry {
            name: format!("Gradient {}", source.name),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: poincare_lib::PlotStyle {
                colour_mode: poincare_lib::ColourMode::ByAttribute {
                    name: "magnitude".to_string(),
                    kind: AttributeKind::Vertex,
                },
                glyph_scale: 0.8,
                shading: poincare_lib::ShadingMode::Unlit,
                ..poincare_lib::PlotStyle::default()
            },
            kind: PlotKind::GradientField {
                expression,
                parameters,
            },
        })
    }

    fn make_divergence_plot(&self, source: &PlotEntry) -> Option<PlotEntry> {
        let (expression, parameters) = match &source.kind {
            PlotKind::ExprVectorField { expression, parameters } => {
                (expression.clone(), parameters.clone())
            }
            _ => return None,
        };
        Some(PlotEntry {
            name: format!("Divergence {}", source.name),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: poincare_lib::PlotStyle {
                opacity: 0.3,
                transfer_function: Some(poincare_lib::TransferFunction {
                    opacity_scale: 0.4,
                    threshold: None,
                }),
                ..poincare_lib::PlotStyle::default()
            },
            kind: PlotKind::DivergenceField {
                expression,
                parameters,
                vol_resolution: [64, 64, 64],
            },
        })
    }

    fn make_curl_plot(&self, source: &PlotEntry) -> Option<PlotEntry> {
        let (expression, parameters) = match &source.kind {
            PlotKind::ExprVectorField { expression, parameters } => {
                (expression.clone(), parameters.clone())
            }
            _ => return None,
        };
        Some(PlotEntry {
            name: format!("Curl {}", source.name),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: poincare_lib::PlotStyle {
                colour_mode: poincare_lib::ColourMode::ByAttribute {
                    name: "magnitude".to_string(),
                    kind: AttributeKind::Vertex,
                },
                glyph_scale: 0.8,
                shading: poincare_lib::ShadingMode::Unlit,
                ..poincare_lib::PlotStyle::default()
            },
            kind: PlotKind::CurlField {
                expression,
                parameters,
            },
        })
    }
}
