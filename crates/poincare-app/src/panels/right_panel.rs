use eframe::egui;
use poincare_lib::{CurveInterpolation, CurveInterpolationKind, sample_curve_points};
use viewport_lib::{AttributeKind, Easing, Projection, ViewPreset};

use crate::App;
use crate::CameraCommand;
use crate::InspectorTab;
use crate::dock::DockTab;
use crate::document::{ExportFormat, ExportMode, default_export_dir, ensure_export_dir_exists, export_mode_for_format};
use crate::plot::analysis::{
    PointAnnotation, SliceAxis, default_slice_position, intersect_surface_meshes,
    make_arrow_annotation, make_point_annotations,
};
use crate::plot::entry::PlotEntry;
use crate::plot::kind::{PlotKind, StyleCaps, evenly_spaced_isovalues};
use crate::panels::left_panel::{PlotMarkerKind, paint_plot_marker};
use crate::plot::table::TableDataSet;
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
                    let (marker_rect, _) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    paint_plot_marker(
                        ui.painter(),
                        marker_rect,
                        color,
                        PlotMarkerKind::from_plot_kind(&plot.kind),
                    );
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&plot.name).strong());
                        ui.label(
                            egui::RichText::new(plot_properties_summary(plot))
                                .small()
                                .weak(),
                        );
                    });
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
        ui.label("Interpolation");
        if let Some(groups) = self.interpolation_source_groups(&selected) {
            let point_count: usize = groups.iter().map(Vec::len).sum();
            ui.label(
                egui::RichText::new(format!(
                    "{} point(s) across {} sequence(s) can be turned into curve geometry.",
                    point_count,
                    groups.len()
                ))
                .small()
                .weak(),
            );
                    if ui.button("Interpolate...").clicked() {
                        self.open_interpolate_modal(plot_idx, &selected);
                    }
        } else {
            ui.label(
                egui::RichText::new(
                    "Interpolation is available for point and ordered sample plots.",
                )
                .small()
                .weak(),
            );
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label("Point Extraction");
        if let Some(groups) = self.polyline_sample_groups(&selected) {
            let point_count: usize = groups.iter().map(Vec::len).sum();
            ui.label(
                egui::RichText::new(format!(
                    "{} sampled point(s) across {} polyline(s) can be extracted as a point plot.",
                    point_count,
                    groups.len()
                ))
                .small()
                .weak(),
            );
            if ui.button("Extract Points").clicked() {
                self.push_analysis_plot(
                    doc_idx,
                    self.make_extracted_points_plot(&selected, groups),
                );
            }
        } else {
            ui.label(
                egui::RichText::new(
                    "Point extraction is available for polyline and interpolated curve plots.",
                )
                .small()
                .weak(),
            );
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label("Intersections");
        ui.label("Curves");
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

        ui.add_space(6.0);
        ui.label("Surfaces");
        if !selected.kind.supports_surface_intersection() {
            ui.label(egui::RichText::new("Select a surface-like plot to compute surface intersections.").small().weak());
            return;
        }

        let candidates = self.surface_intersection_candidates(doc_idx, plot_idx);
        if candidates.is_empty() {
            ui.label(egui::RichText::new("No other compatible surface plots are available in this document.").small().weak());
            return;
        }

        if self.surface_intersection_target == Some(plot_idx) {
            self.surface_intersection_target = None;
        }
        if self
            .surface_intersection_target
            .is_none_or(|target| !candidates.iter().any(|(index, _)| *index == target))
        {
            self.surface_intersection_target = Some(candidates[0].0);
        }

        egui::ComboBox::from_label("Target Surface")
            .selected_text(
                self.surface_intersection_target
                    .and_then(|target| {
                        candidates
                            .iter()
                            .find(|(index, _)| *index == target)
                            .map(|(_, label)| label.clone())
                    })
                    .unwrap_or_else(|| "Select target".to_string()),
            )
            .show_ui(ui, |ui| {
                for (index, label) in &candidates {
                    ui.selectable_value(&mut self.surface_intersection_target, Some(*index), label);
                }
            });

        ui.horizontal(|ui| {
            ui.label("Tolerance");
            ui.add(
                egui::DragValue::new(&mut self.surface_intersection_tolerance)
                    .speed(0.001)
                    .range(0.0001..=1.0),
            );
            ui.label("Stitch");
            ui.add(
                egui::DragValue::new(&mut self.surface_intersection_stitch_distance)
                    .speed(0.001)
                    .range(0.0001..=2.0),
            );
        });
        ui.checkbox(
            &mut self.surface_intersection_make_points,
            "Create point markers for isolated contacts",
        );
        ui.add_enabled_ui(self.surface_intersection_make_points, |ui| {
            ui.checkbox(
                &mut self.surface_intersection_show_point_labels,
                "Show point labels",
            );
        });
        if self.documents[doc_idx].scene_dirty {
            ui.label(
                egui::RichText::new(
                    "Scene has pending changes; intersection uses the last rebuilt mesh state.",
                )
                .small()
                .weak(),
            );
        }
        if ui.button("Extract Surface Intersection").clicked()
            && let Some(target_idx) = self.surface_intersection_target
        {
            self.create_surface_intersection_plots(doc_idx, plot_idx, target_idx);
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

    fn surface_intersection_candidates(&self, doc_idx: usize, source_idx: usize) -> Vec<(usize, String)> {
        self.documents[doc_idx]
            .plots
            .iter()
            .enumerate()
            .filter(|(index, plot)| *index != source_idx && plot.kind.supports_surface_intersection())
            .map(|(index, plot)| (index, plot.name.clone()))
            .collect()
    }

    fn create_surface_intersection_plots(&mut self, doc_idx: usize, source_idx: usize, target_idx: usize) {
        let source_pick_id = (source_idx + 1) as u64;
        let target_pick_id = (target_idx + 1) as u64;
        let probe_data = self.documents[doc_idx].scene.probe_data();
        let source_surfaces: Vec<_> = probe_data
            .surfaces
            .iter()
            .filter(|surface| surface.pick_id == source_pick_id)
            .collect();
        let target_surfaces: Vec<_> = probe_data
            .surfaces
            .iter()
            .filter(|surface| surface.pick_id == target_pick_id)
            .collect();
        if source_surfaces.is_empty() || target_surfaces.is_empty() {
            self.documents[doc_idx].export_status =
                "Surface intersection failed: one or both plots have no cached surface mesh."
                    .to_string();
            return;
        }

        let mut all_curves = Vec::new();
        let mut all_points = Vec::new();
        for source in &source_surfaces {
            for target in &target_surfaces {
                let result = intersect_surface_meshes(
                    source.positions,
                    source.indices,
                    target.positions,
                    target.indices,
                    self.surface_intersection_tolerance,
                    self.surface_intersection_stitch_distance,
                );
                all_curves.extend(result.curves);
                all_points.extend(result.isolated_points);
            }
        }

        if all_curves.is_empty() && all_points.is_empty() {
            self.documents[doc_idx].export_status =
                "No surface-surface intersections were found with the current tolerance."
                    .to_string();
            return;
        }

        let source = self.documents[doc_idx].plots[source_idx].clone();
        let target = self.documents[doc_idx].plots[target_idx].clone();
        if !all_curves.is_empty() {
            self.push_analysis_plot(
                doc_idx,
                PlotEntry {
                    name: format!("{} intersect {}", source.name, target.name),
                    visible: true,
                    domain: source.domain.clone(),
                    resolution: source.resolution,
                    style: poincare_lib::PlotStyle {
                        colour_mode: poincare_lib::ColourMode::Solid([0.98, 0.85, 0.2, 1.0]),
                        line_width: 2.5,
                        ..poincare_lib::PlotStyle::default()
                    },
                    kind: PlotKind::DerivedPolylineGroups {
                        groups: all_curves
                            .iter()
                            .map(|curve| curve.iter().map(|point| point.to_array()).collect())
                            .collect(),
                    },
                },
            );
        }
        if self.surface_intersection_make_points && !all_points.is_empty() {
            let points = all_points.iter().map(|point| point.to_array()).collect::<Vec<_>>();
            self.push_analysis_plot(
                doc_idx,
                PlotEntry {
                    name: format!("{} intersect {} Points", source.name, target.name),
                    visible: true,
                    domain: source.domain.clone(),
                    resolution: source.resolution,
                    style: poincare_lib::PlotStyle {
                        colour_mode: poincare_lib::ColourMode::Solid([1.0, 0.35, 0.35, 1.0]),
                        point_size: 9.0,
                        ..poincare_lib::PlotStyle::default()
                    },
                    kind: PlotKind::PointAnnotations {
                        points: make_point_annotations(&points, "Surface Contact"),
                        show_labels: self.surface_intersection_show_point_labels,
                    },
                },
            );
        }
        self.documents[doc_idx].export_status.clear();
    }

    pub(crate) fn show_interpolate_modal(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.interpolate_modal.clone() else {
            return;
        };

        let Some(plot) = self.documents[self.active_document_idx]
            .plots
            .get(state.source_plot_idx)
            .cloned()
        else {
            self.interpolate_modal = None;
            return;
        };

        let Some(groups) = self.interpolation_source_groups(&plot) else {
            self.interpolate_modal = None;
            return;
        };

        let mut open = true;
        let mut create_plot = false;
        let mut cancel_clicked = false;
        egui::Window::new("Interpolate Plot")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Build a derived curve from {} sequence(s) / {} point(s).",
                        groups.len(),
                        groups.iter().map(Vec::len).sum::<usize>()
                    ))
                    .small()
                    .weak(),
                );
                ui.add_space(8.0);

                ui.label("Method");
                egui::ComboBox::from_id_salt("interpolation_method")
                    .selected_text(interpolation_kind_label(state.interpolation.kind))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.interpolation.kind,
                            CurveInterpolationKind::Linear,
                            interpolation_kind_label(CurveInterpolationKind::Linear),
                        );
                        ui.selectable_value(
                            &mut state.interpolation.kind,
                            CurveInterpolationKind::CatmullRom,
                            interpolation_kind_label(CurveInterpolationKind::CatmullRom),
                        );
                        ui.selectable_value(
                            &mut state.interpolation.kind,
                            CurveInterpolationKind::CentripetalCatmullRom,
                            interpolation_kind_label(
                                CurveInterpolationKind::CentripetalCatmullRom,
                            ),
                        );
                        ui.selectable_value(
                            &mut state.interpolation.kind,
                            CurveInterpolationKind::MovingAverage,
                            interpolation_kind_label(CurveInterpolationKind::MovingAverage),
                        );
                        ui.selectable_value(
                            &mut state.interpolation.kind,
                            CurveInterpolationKind::SavitzkyGolay,
                            interpolation_kind_label(CurveInterpolationKind::SavitzkyGolay),
                        );
                    });
                ui.checkbox(&mut state.interpolation.closed, "Closed loop");
                ui.add(
                    egui::Slider::new(
                        &mut state.interpolation.samples_per_segment,
                        1..=64,
                    )
                    .text("Samples per segment"),
                );
                if uses_smoothing_window(state.interpolation.kind) {
                    ui.add(
                        egui::Slider::new(&mut state.interpolation.smoothing_window, 3..=25)
                            .text("Smoothing Window"),
                    );
                    state.interpolation.smoothing_window =
                        normalized_window_value(state.interpolation.smoothing_window);
                }
                ui.label(
                    egui::RichText::new(match state.interpolation.kind {
                        CurveInterpolationKind::Linear => {
                            "Linear just connects the samples in order."
                        }
                        CurveInterpolationKind::CatmullRom => {
                            "Catmull-Rom passes through the samples with a smooth cubic spline."
                        }
                        CurveInterpolationKind::CentripetalCatmullRom => {
                            "Centripetal Catmull-Rom reduces overshoot and self-intersection risk."
                        }
                        CurveInterpolationKind::MovingAverage => {
                            "Moving average smooths noisy samples first, then resamples a smooth path."
                        }
                        CurveInterpolationKind::SavitzkyGolay => {
                            "Savitzky-Golay smooths while preserving local shape better than a plain average."
                        }
                    })
                    .small()
                    .weak(),
                );
                ui.add_space(8.0);
                ui.label("Output Name");
                ui.text_edit_singleline(&mut state.output_name);
                if !state.error.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(255, 110, 110), &state.error);
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Create Interpolated Plot").clicked() {
                        create_plot = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            });

        if cancel_clicked {
            open = false;
        }

        if create_plot {
            match self.create_interpolated_plot_from_modal(&state, &plot, groups) {
                Ok(()) => {
                    self.interpolate_modal = None;
                    return;
                }
                Err(error) => state.error = error,
            }
        }

        self.interpolate_modal = open.then_some(state);
    }

    fn open_interpolate_modal(&mut self, plot_idx: usize, plot: &PlotEntry) {
        self.interpolate_modal = Some(crate::InterpolateModalState {
            source_plot_idx: plot_idx,
            output_name: format!("Interpolated {}", plot.name),
            interpolation: CurveInterpolation {
                kind: CurveInterpolationKind::Linear,
                samples_per_segment: 1,
                closed: false,
                smoothing_window: 5,
            },
            error: String::new(),
        });
    }

    fn interpolation_source_groups(&self, plot: &PlotEntry) -> Option<Vec<Vec<[f32; 3]>>> {
        match &plot.kind {
            PlotKind::PointAnnotations { points, .. } => Some(vec![points
                .iter()
                .map(|point| point.position)
                .collect()]),
            PlotKind::ImportedTable { definition } => match definition.validate().ok()? {
                TableDataSet::Curve { groups, .. } => Some(
                    groups
                        .iter()
                        .map(|group| group.iter().map(|point| point.to_array()).collect())
                        .collect(),
                ),
                TableDataSet::Scatter { points, .. } => {
                    Some(vec![points.iter().map(|point| point.to_array()).collect()])
                }
                _ => None,
            },
            PlotKind::DerivedPolylineGroups { groups } => Some(groups.clone()),
            PlotKind::InterpolatedCurve { points, .. } => Some(vec![points.clone()]),
            _ => None,
        }
    }

    fn polyline_sample_groups(&self, plot: &PlotEntry) -> Option<Vec<Vec<[f32; 3]>>> {
        match &plot.kind {
            PlotKind::ImportedTable { definition } => match definition.validate().ok()? {
                TableDataSet::Curve { groups, .. } => Some(
                    groups
                        .iter()
                        .map(|group| group.iter().map(|point| point.to_array()).collect())
                        .collect(),
                ),
                _ => None,
            },
            PlotKind::DerivedPolylineGroups { groups } => Some(groups.clone()),
            PlotKind::InterpolatedCurve {
                points,
                interpolation,
            } => {
                let sampled = sample_curve_points(
                    &points
                        .iter()
                        .map(|point| glam::Vec3::from_array(*point))
                        .collect::<Vec<_>>(),
                    *interpolation,
                );
                Some(vec![sampled.iter().map(|point| point.to_array()).collect()])
            }
            _ => None,
        }
    }

    fn make_extracted_points_plot(
        &self,
        source: &PlotEntry,
        groups: Vec<Vec<[f32; 3]>>,
    ) -> PlotEntry {
        let positions = groups.into_iter().flatten().collect::<Vec<_>>();
        PlotEntry {
            name: format!("Points from {}", source.name),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: poincare_lib::PlotStyle {
                colour_mode: poincare_lib::ColourMode::Solid([0.35, 0.85, 1.0, 1.0]),
                point_size: 8.0,
                ..poincare_lib::PlotStyle::default()
            },
            kind: PlotKind::PointAnnotations {
                points: make_point_annotations(&positions, "Point"),
                show_labels: false,
            },
        }
    }

    fn create_interpolated_plot_from_modal(
        &mut self,
        state: &crate::InterpolateModalState,
        source_plot: &PlotEntry,
        groups: Vec<Vec<[f32; 3]>>,
    ) -> Result<(), String> {
        let name = state.output_name.trim();
        if name.is_empty() {
            return Err("Output name is required.".to_string());
        }
        if groups.is_empty() || groups.iter().all(Vec::is_empty) {
            return Err("The selected plot does not have usable point samples.".to_string());
        }
        if groups.iter().all(|group| group.len() < 2) {
            return Err("At least two points are required to interpolate a curve.".to_string());
        }

        if groups.len() == 1 {
            self.push_analysis_plot(
                self.active_document_idx,
                PlotEntry {
                    name: name.to_string(),
                    visible: true,
                    domain: source_plot.domain.clone(),
                    resolution: source_plot.resolution,
                    style: poincare_lib::PlotStyle {
                        colour_mode: poincare_lib::ColourMode::Solid([0.95, 0.7, 0.2, 1.0]),
                        line_width: 2.5,
                        ..poincare_lib::PlotStyle::default()
                    },
                    kind: PlotKind::InterpolatedCurve {
                        points: groups.into_iter().next().unwrap_or_default(),
                        interpolation: state.interpolation,
                    },
                },
            );
        } else {
            for (group_index, group) in groups.into_iter().enumerate() {
                if group.len() < 2 {
                    continue;
                }
                self.push_analysis_plot(
                    self.active_document_idx,
                    PlotEntry {
                        name: format!("{name} {}", group_index + 1),
                        visible: true,
                        domain: source_plot.domain.clone(),
                        resolution: source_plot.resolution,
                        style: poincare_lib::PlotStyle {
                            colour_mode: poincare_lib::ColourMode::Solid([0.95, 0.7, 0.2, 1.0]),
                            line_width: 2.5,
                            ..poincare_lib::PlotStyle::default()
                        },
                        kind: PlotKind::InterpolatedCurve {
                            points: group,
                            interpolation: state.interpolation,
                        },
                    },
                );
            }
        }

        Ok(())
    }
}

fn interpolation_kind_label(kind: CurveInterpolationKind) -> &'static str {
    match kind {
        CurveInterpolationKind::Linear => "Polyline (Linear)",
        CurveInterpolationKind::CatmullRom => "Interpolation (Catmull-Rom)",
        CurveInterpolationKind::CentripetalCatmullRom => {
            "Interpolation (Centripetal Catmull-Rom)"
        }
        CurveInterpolationKind::MovingAverage => "Smoothing (Moving Average)",
        CurveInterpolationKind::SavitzkyGolay => "Smoothing (Savitzky-Golay)",
    }
}

fn uses_smoothing_window(kind: CurveInterpolationKind) -> bool {
    matches!(
        kind,
        CurveInterpolationKind::MovingAverage | CurveInterpolationKind::SavitzkyGolay
    )
}

fn normalized_window_value(window: u32) -> u32 {
    let mut normalized = window.max(3);
    if normalized % 2 == 0 {
        normalized += 1;
    }
    normalized
}

fn sampled_curve_positions(points: &[[f32; 3]], interpolation: CurveInterpolation) -> Vec<[f32; 3]> {
    sample_curve_points(
        &points
            .iter()
            .map(|point| glam::Vec3::from_array(*point))
            .collect::<Vec<_>>(),
        interpolation,
    )
    .into_iter()
    .map(|point| point.to_array())
    .collect()
}

fn plot_properties_summary(plot: &PlotEntry) -> String {
    match &plot.kind {
        PlotKind::ContouredSurface { contour_values, .. } => {
            format!("Contoured surface, {} contour levels", contour_values.len())
        }
        PlotKind::SphericalHarmonic => "Spherical harmonic surface".to_string(),
        PlotKind::HelixCurve => "Curve".to_string(),
        PlotKind::ScatterCloud => "Points".to_string(),
        PlotKind::VectorField => "Vector field".to_string(),
        PlotKind::GridSurface => "Surface".to_string(),
        PlotKind::Streamlines { seeds } => format!("Streamlines, {} seed points", seeds.len()),
        PlotKind::VolumeRender { .. } => "Volume".to_string(),
        PlotKind::Isosurface { isovalues, .. } => {
            format!("Isosurface, {} levels", isovalues.len())
        }
        PlotKind::ExprCartesian { .. } => "Cartesian surface".to_string(),
        PlotKind::ExprCurve { .. } => "Parametric curve".to_string(),
        PlotKind::ExprCartesianLine { .. } => "Cartesian line".to_string(),
        PlotKind::ExprSpherical { .. } => "Spherical surface".to_string(),
        PlotKind::ExprCylindrical { .. } => "Cylindrical surface".to_string(),
        PlotKind::ExprPolar { .. } => "Polar surface".to_string(),
        PlotKind::ExprParametricSurface { .. } => "Parametric surface".to_string(),
        PlotKind::ImportedTable { definition } => match definition.validate() {
            Ok(TableDataSet::SurfaceGrid { xs, ys, .. }) => {
                format!("Imported surface grid, {}x{} samples", xs.len(), ys.len())
            }
            Ok(TableDataSet::Curve { groups, .. }) => {
                let point_count: usize = groups.iter().map(Vec::len).sum();
                format!(
                    "Imported polylines, {} points across {} curve(s)",
                    point_count,
                    groups.len()
                )
            }
            Ok(TableDataSet::Scatter { points, .. }) => {
                format!("Imported points, {} points", points.len())
            }
            Ok(TableDataSet::VectorField { samples, .. }) => {
                format!("Imported vector field, {} samples", samples.len())
            }
            Err(_) => format!("Imported {}", definition.target.label().to_lowercase()),
        },
        PlotKind::ScalarSlice { contour_values, .. } => {
            format!("Scalar slice, {} contour levels", contour_values.len())
        }
        PlotKind::VectorSlice { .. } => "Vector slice".to_string(),
        PlotKind::GradientField { .. } => "Gradient field".to_string(),
        PlotKind::DivergenceField { .. } => "Divergence volume".to_string(),
        PlotKind::CurlField { .. } => "Curl field".to_string(),
        PlotKind::PointAnnotations { points, .. } => {
            format!("Points, {} points", points.len())
        }
        PlotKind::ArrowAnnotations { arrows, .. } => {
            format!("Arrow annotations, {} arrows", arrows.len())
        }
        PlotKind::DerivedPolylineGroups { groups } => {
            let point_count: usize = groups.iter().map(Vec::len).sum();
            format!(
                "Polylines, {} points across {} curve(s)",
                point_count,
                groups.len()
            )
        }
        PlotKind::InterpolatedCurve {
            points,
            interpolation,
        } => {
            let sampled_count = sampled_curve_positions(points, *interpolation).len();
            format!(
                "Interpolated polyline, {} sampled points from {} control points, {}",
                sampled_count,
                points.len(),
                interpolation_kind_label(interpolation.kind)
            )
        }
        PlotKind::ExprVectorField { .. } => "Vector field".to_string(),
        PlotKind::ExprVolume { .. } => "Volume".to_string(),
        PlotKind::ExprIsosurface { isovalues, .. } => {
            format!("Isosurface, {} levels", isovalues.len())
        }
        PlotKind::ExprStreamlines {
            step_size,
            max_steps,
            ..
        } => format!("Streamlines, step {:.3}, max {} steps", step_size, max_steps),
    }
}
