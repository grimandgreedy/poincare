use eframe::egui;
use poincare_lib::{
    AnalysisKind, AnalysisOutput, AnalysisRequest, AnalysisTarget, CurveInterpolation,
    CurveInterpolationKind, SampleGroupsKind, available_analyses, run_analysis,
    run_curve_surface_frame_analysis, run_curve_surface_measurement_analysis,
    run_surface_mesh_analysis, sample_curve_points, sample_groups,
};
use serde_json::json;
use viewport_lib::{Easing, Projection, ViewPreset};

use crate::App;
use crate::CameraCommand;
use crate::InspectorTab;
use crate::SurfaceCurvatureQuantityUi;
use crate::dock::DockTab;
use crate::document::{
    ExportFormat, ExportMode, default_export_dir, ensure_export_dir_exists, export_mode_for_format,
};
use crate::panels::left_panel::{PlotMarkerKind, paint_plot_marker};
use crate::plot::analysis::{
    PointAnnotation, intersect_surface_meshes, make_arrow_annotation, make_point_annotations,
};
use crate::plot::entry::PlotEntry;
use crate::plot::kind::{PlotKind, PlotKindExt, StyleCaps, evenly_spaced_isovalues};
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
        let mut copy_metadata_for_plot = None;

        ui.horizontal(|ui| {
            if let Some(index) = selected_plot {
                if let Some(plot) = self.documents[doc_idx].plots.get(index) {
                    let color = self.representative_plot_color(plot);
                    let marker_kind = PlotMarkerKind::from_plot_kind(&plot.kind);
                    let (marker_rect, marker_response) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    paint_plot_marker(ui.painter(), marker_rect, color, marker_kind);
                    marker_response.on_hover_text(marker_kind.label());
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&plot.name).strong());
                        ui.label(
                            egui::RichText::new(plot_properties_summary(plot))
                                .small()
                                .weak(),
                        );
                    });
                    let copy_button = ui
                        .button(egui::RichText::new("").family(egui::FontFamily::Monospace))
                        .on_hover_text("Copy structured plot metadata");
                    if copy_button.clicked() {
                        copy_metadata_for_plot = Some(index);
                    }
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

        if let Some(index) = copy_metadata_for_plot
            && let Some(plot) = self.documents[doc_idx].plots.get(index)
        {
            let metadata_json = plot_metadata_clipboard_json(plot);
            if let Ok(text) = serde_json::to_string_pretty(&metadata_json) {
                ui.ctx().copy_text(text);
                self.documents[doc_idx].export_status =
                    "Copied structured plot metadata to clipboard.".to_string();
            }
        }

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
            self.documents[self.active_document_idx]
                .saved_views
                .remove(slot);
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
            let (mut dir, mut filename) = crate::split_export_path(
                &self.documents[self.active_document_idx].export_path,
                current_format,
            );
            ui.horizontal(|ui| {
                ui.label("Mode");
                let image_clicked = ui
                    .selectable_value(&mut mode, ExportMode::Image, "Image")
                    .clicked();
                let video_clicked = ui
                    .selectable_value(&mut mode, ExportMode::Video, "Video")
                    .clicked();
                if image_clicked
                    && self.documents[self.active_document_idx].export_format != ExportFormat::Png
                {
                    self.documents[self.active_document_idx].export_format = ExportFormat::Png;
                    dir = default_export_dir(ExportMode::Image);
                    let _ = ensure_export_dir_exists(&dir);
                    filename = "poincare-export.png".to_string();
                }
                if video_clicked
                    && self.documents[self.active_document_idx].export_format == ExportFormat::Png
                {
                    self.documents[self.active_document_idx].export_format = ExportFormat::Mp4;
                    dir = default_export_dir(ExportMode::Video);
                    let _ = ensure_export_dir_exists(&dir);
                    filename = "poincare-export.mp4".to_string();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Directory");
                let mut dir_text = dir.to_string_lossy().into_owned();
                ui.add(egui::TextEdit::singleline(&mut dir_text).desired_width(320.0));
                if ui.button("Choose…").clicked() {
                    let start_dir = if dir.as_os_str().is_empty() {
                        default_export_dir(mode)
                    } else {
                        dir.clone()
                    };
                    if let Some(chosen) = rfd::FileDialog::new()
                        .set_directory(start_dir)
                        .pick_folder()
                    {
                        dir = chosen;
                    }
                }
                if dir_text != dir.to_string_lossy() {
                    dir = std::path::PathBuf::from(dir_text);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Filename");
                ui.add(egui::TextEdit::singleline(&mut filename).desired_width(220.0));
            });
            let full_export_path = crate::export_path_from_parts(
                &dir,
                &filename,
                self.documents[self.active_document_idx].export_format,
            );
            self.documents[self.active_document_idx].export_path =
                full_export_path.to_string_lossy().into_owned();
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(
                        &mut self.documents[self.active_document_idx].export_width,
                    )
                    .speed(1)
                    .range(256..=8192)
                    .prefix("W "),
                );
                ui.add(
                    egui::DragValue::new(
                        &mut self.documents[self.active_document_idx].export_height,
                    )
                    .speed(1)
                    .range(256..=8192)
                    .prefix("H "),
                );
            });
            if mode == ExportMode::Video {
                egui::ComboBox::from_label("Video Format")
                    .selected_text(
                        match self.documents[self.active_document_idx].export_format {
                            ExportFormat::Gif => "GIF",
                            ExportFormat::Mp4 => "MP4",
                            ExportFormat::Png => "MP4",
                        },
                    )
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
                egui::RichText::new(
                    match self.documents[self.active_document_idx].export_format {
                        ExportFormat::Png => {
                            "Images default to ~/Pictures/Poincare and use .png files."
                        }
                        ExportFormat::Gif => {
                            "Videos default to ~/Videos/Poincare and use .gif files."
                        }
                        ExportFormat::Mp4 => {
                            "Videos default to ~/Videos/Poincare and use .mp4 files."
                        }
                    },
                )
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
        let selected_spec = selected.to_plot_spec();
        let capabilities = available_analyses(&selected_spec);
        let has_analysis = |kind| capabilities.iter().any(|cap| cap.kind == kind);
        let has_derived_tools = has_analysis(AnalysisKind::ScalarSlice)
            || has_analysis(AnalysisKind::VectorSlice)
            || has_analysis(AnalysisKind::GradientField)
            || has_analysis(AnalysisKind::DivergenceField)
            || has_analysis(AnalysisKind::CurlField);
        let has_annotations = self.documents[doc_idx].last_probe_hit.is_some()
            || !self.documents[doc_idx].pinned_probes.is_empty();
        let has_data_analysis = has_analysis(AnalysisKind::PointCloudStatistics)
            || has_analysis(AnalysisKind::DataQualityChecks);
        let curve_groups = sample_groups(&selected_spec, SampleGroupsKind::Curve).ok();
        let has_curve_analysis = curve_groups.is_some();
        let selected_plot_id = self.documents[doc_idx].plots[plot_idx].plot_id;
        let relevant_frame_fields = self.documents[doc_idx]
            .frame_fields
            .iter()
            .filter(|field| field.source_plot_ids.contains(&selected_plot_id))
            .map(|field| (field.id, field.title.clone()))
            .collect::<Vec<_>>();
        let has_frame_playback = !relevant_frame_fields.is_empty();
        let interpolation_groups =
            sample_groups(&selected_spec, SampleGroupsKind::InterpolationSource).ok();
        let has_interpolation = interpolation_groups.is_some();
        let polyline_groups = sample_groups(&selected_spec, SampleGroupsKind::Polyline).ok();
        let has_point_extraction = polyline_groups.is_some();
        let has_surface_geometry =
            has_analysis(AnalysisKind::SurfaceNormals) || has_analysis(AnalysisKind::SurfaceArea);
        let surface_intersection_candidates =
            self.surface_intersection_candidates(doc_idx, plot_idx);
        let has_curve_intersections = !self.documents[doc_idx].intersection_cache.is_empty();
        let has_surface_intersections = selected.kind.supports_surface_intersection()
            && !surface_intersection_candidates.is_empty();
        let has_intersections = has_curve_intersections || has_surface_intersections;

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.analysis_show_all, "Show all");
        });
        let show_all = self.analysis_show_all;

        if show_all || has_derived_tools {
            ui.label("Derived Tools");
            if has_derived_tools {
                ui.horizontal(|ui| {
                    if has_analysis(AnalysisKind::ScalarSlice) && ui.button("Add Z Slice").clicked()
                    {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::ScalarSlice,
                            vec![("axis".to_string(), "z".to_string())],
                        );
                    }
                    if has_analysis(AnalysisKind::GradientField)
                        && ui.button("Add Gradient Field").clicked()
                    {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::GradientField,
                            vec![],
                        );
                    }
                    if has_analysis(AnalysisKind::VectorSlice)
                        && ui.button("Add Z Vector Slice").clicked()
                    {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::VectorSlice,
                            vec![("axis".to_string(), "z".to_string())],
                        );
                    }
                    if has_analysis(AnalysisKind::DivergenceField)
                        && ui.button("Add Divergence Volume").clicked()
                    {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::DivergenceField,
                            vec![],
                        );
                    }
                    if has_analysis(AnalysisKind::CurlField)
                        && ui.button("Add Curl Field").clicked()
                    {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::CurlField,
                            vec![],
                        );
                    }
                });
                if has_analysis(AnalysisKind::ScalarSlice) {
                    ui.label(
                        egui::RichText::new("Slices include contour cross-sections.")
                            .small()
                            .weak(),
                    );
                }
            } else {
                ui.label(egui::RichText::new(
                "Select a scalar field or vector field plot to generate slices or derived fields.",
            ).small().weak());
            }
        }

        if show_all || has_annotations {
            ui.add_space(8.0);
            ui.separator();
            ui.label("Annotations");

            if let Some(hit) = self.documents[doc_idx].last_probe_hit.clone() {
                ui.horizontal(|ui| {
                    if ui.button("Annotate Probe Point").clicked() {
                        self.push_analysis_plot(
                            doc_idx,
                            PlotEntry {
                                plot_id: 0,
                                parent_plot_id: None,
                                relationship: crate::plot::entry::PlotRelationship::Primary,
                                name: "Probe Annotation".to_string(),
                                visible: true,
                                domain: self.documents[doc_idx].plots[plot_idx].domain.clone(),
                                resolution: self.documents[doc_idx].plots[plot_idx].resolution,
                                style: poincare_lib::PlotStyle {
                                    colour_mode: poincare_lib::ColourMode::Solid([
                                        1.0, 0.95, 0.35, 1.0,
                                    ]),
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
                                plot_id: 0,
                                parent_plot_id: None,
                                relationship: crate::plot::entry::PlotRelationship::Primary,
                                name: "Probe Direction".to_string(),
                                visible: true,
                                domain: self.documents[doc_idx].plots[plot_idx].domain.clone(),
                                resolution: self.documents[doc_idx].plots[plot_idx].resolution,
                                style: poincare_lib::PlotStyle {
                                    colour_mode: poincare_lib::ColourMode::Solid([
                                        0.35, 0.85, 1.0, 1.0,
                                    ]),
                                    glyph_scale: 1.0,
                                    shading: poincare_lib::ShadingMode::Unlit,
                                    ..poincare_lib::PlotStyle::default()
                                },
                                kind: PlotKind::ArrowAnnotations {
                                    arrows: vec![make_arrow_annotation(
                                        hit.world_pos,
                                        hit.normal,
                                        if hit.snapped {
                                            "Snapped Direction"
                                        } else {
                                            "Probe Direction"
                                        },
                                    )],
                                    show_labels: true,
                                },
                            },
                        );
                    }
                });
            } else {
                ui.label(
                    egui::RichText::new(
                        "Use probe mode to create point, normal, or tangent annotations.",
                    )
                    .small()
                    .weak(),
                );
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
                        plot_id: 0,
                        parent_plot_id: None,
                        relationship: crate::plot::entry::PlotRelationship::Primary,
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
                            show_labels: false,
                        },
                    },
                );
            }
        }

        if show_all || has_data_analysis {
            ui.add_space(8.0);
            ui.separator();
            ui.label("Data Analysis");
            if has_data_analysis {
                if let Ok(groups) = sample_groups(&selected_spec, SampleGroupsKind::SampleData) {
                    let point_count: usize = groups.iter().map(Vec::len).sum();
                    ui.label(
                    egui::RichText::new(format!(
                        "{} sampled point(s) across {} sequence(s) are available for statistics and data-quality checks.",
                        point_count,
                        groups.len()
                    ))
                    .small()
                    .weak(),
                );
                }
                ui.horizontal(|ui| {
                    if has_analysis(AnalysisKind::PointCloudStatistics)
                        && ui.button("Point Statistics").clicked()
                    {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::PointCloudStatistics,
                            vec![],
                        );
                    }
                    if has_analysis(AnalysisKind::DataQualityChecks)
                        && ui.button("Data Quality Checks").clicked()
                    {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::DataQualityChecks,
                            vec![],
                        );
                    }
                });
            } else {
                ui.label(
                egui::RichText::new(
                    "Point statistics and data-quality checks are available for sampled point sets and ordered sample data.",
                )
                .small()
                .weak(),
            );
            }
        }

        if show_all || has_curve_analysis {
            ui.add_space(8.0);
            ui.separator();
            ui.label("Curve Analysis");
            if let Some(groups) = curve_groups {
                let point_count: usize = groups.iter().map(Vec::len).sum();
                ui.label(
                egui::RichText::new(format!(
                    "{} sampled point(s) across {} curve(s) are available for derivative, tangent, and normalized integral plots.",
                    point_count,
                    groups.len()
                ))
                .small()
                .weak(),
            );
                ui.horizontal(|ui| {
                    if ui.button("Create Derivative Curve").clicked() {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::DifferentiateCurve,
                            vec![],
                        );
                    }
                    if ui.button("Create Integral Curve").clicked() {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::IntegralCurve,
                            vec![],
                        );
                    }
                    if ui.button("Create Tangent Curve").clicked() {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::TangentField,
                            vec![],
                        );
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Differentiate by Axis...").clicked() {
                        self.open_axis_derivative_modal(plot_idx, &selected);
                    }
                    if ui.button("Create Arc Length Curve").clicked() {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::ArcLengthCurve,
                            vec![],
                        );
                    }
                    if ui.button("Create Curvature Curve").clicked() {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::CurvatureCurve,
                            vec![],
                        );
                    }
                    if ui.button("Create Normal Curve").clicked() {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::NormalField,
                            vec![],
                        );
                    }
                    if ui.button("Create Binormal Curve").clicked() {
                        self.run_single_plot_analysis(
                            doc_idx,
                            &selected_spec,
                            AnalysisKind::BinormalField,
                            vec![],
                        );
                    }
                });
                ui.add_space(6.0);
                ui.label("Curve fitting");
                ui.horizontal(|ui| {
                    if ui.button("Fit Curve...").clicked() {
                        self.open_fit_curve_modal(plot_idx, &selected);
                    }
                });
                ui.label(
                egui::RichText::new(
                    "Create fitted curves, optional control points, residual plots, and fit diagnostics.",
                )
                .small()
                .weak(),
            );
                ui.add_space(6.0);
                ui.label("Moving frames");
                ui.horizontal(|ui| {
                    if has_analysis(AnalysisKind::FrenetFrame)
                        && ui.button("Frenet Frame").clicked()
                    {
                        self.open_moving_frame_modal(plot_idx, AnalysisKind::FrenetFrame, None);
                    }
                    if has_analysis(AnalysisKind::BishopFrame)
                        && ui.button("Bishop Frame").clicked()
                    {
                        self.open_moving_frame_modal(plot_idx, AnalysisKind::BishopFrame, None);
                    }
                });
                ui.label(
                egui::RichText::new(
                    "Generate reusable sampled frame tables plus tangent, normal, and binormal triad plots. Bishop frames are rotation-minimizing and more stable near inflections.",
                )
                .small()
                .weak(),
            );
                let surface_candidates = self.surface_frame_candidates(doc_idx, plot_idx);
                if !surface_candidates.is_empty() {
                    ui.add_space(6.0);
                    ui.label("Surface-coupled frames");
                    ui.horizontal(|ui| {
                        if ui.button("Darboux Frame").clicked() {
                            self.open_moving_frame_modal(
                                plot_idx,
                                AnalysisKind::DarbouxFrame,
                                surface_candidates.first().map(|(index, _)| *index),
                            );
                        }
                        if ui.button("Surface-Aligned Frame").clicked() {
                            self.open_moving_frame_modal(
                                plot_idx,
                                AnalysisKind::SurfaceAlignedFrame,
                                surface_candidates.first().map(|(index, _)| *index),
                            );
                        }
                    });
                    ui.label(
                    egui::RichText::new(
                        "Use a selected curve together with a cached target surface to build Darboux or surface-normal-aligned frames.",
                    )
                    .small()
                    .weak(),
                );
                }
                if !surface_candidates.is_empty() {
                    ui.add_space(6.0);
                    ui.label("Curve-on-surface measurement");
                    ui.horizontal(|ui| {
                        if ui.button("Measure Against Surface...").clicked() {
                            self.open_curve_surface_measurement_modal(
                                plot_idx,
                                surface_candidates.first().map(|(index, _)| *index),
                            );
                        }
                    });
                    ui.label(
                    egui::RichText::new(
                        "Project the selected curve onto a target surface and report projected length and deviation.",
                    )
                    .small()
                    .weak(),
                );
                }
                self.show_inline_moving_frame_controls(
                    ui,
                    doc_idx,
                    plot_idx,
                    &selected,
                    &surface_candidates,
                );
            } else {
                ui.label(
                egui::RichText::new(
                    "Curve calculus tools are available for curve and polyline plots. Use the axis-derivative modal for outputs like dy/dx or dz/dx.",
                )
                .small()
                .weak(),
            );
            }
        }

        if show_all || has_frame_playback {
            ui.add_space(8.0);
            ui.separator();
            ui.label("Frame Playback");
            if has_frame_playback {
                let selected_frame_id = self.documents[doc_idx]
                    .frame_playback
                    .selected_frame_field
                    .filter(|frame_id| relevant_frame_fields.iter().any(|(id, _)| id == frame_id))
                    .or_else(|| relevant_frame_fields.first().map(|(id, _)| *id));
                self.documents[doc_idx].frame_playback.selected_frame_field = selected_frame_id;
                let current_label = selected_frame_id
                    .and_then(|id| {
                        relevant_frame_fields
                            .iter()
                            .find(|(field_id, _)| *field_id == id)
                            .map(|(_, title)| title.clone())
                    })
                    .unwrap_or_else(|| "Select frame field".to_string());
                egui::ComboBox::from_label("Stored FrameField")
                    .selected_text(current_label)
                    .show_ui(ui, |ui| {
                        for (field_id, title) in &relevant_frame_fields {
                            ui.selectable_value(
                                &mut self.documents[doc_idx].frame_playback.selected_frame_field,
                                Some(*field_id),
                                title,
                            );
                        }
                    });
                ui.horizontal(|ui| {
                    let playing = self.documents[doc_idx].frame_playback.playing;
                    if ui.button(if playing { "Pause" } else { "Play" }).clicked() {
                        self.documents[doc_idx].frame_playback.playing = !playing;
                    }
                    if ui.button("Reset").clicked() {
                        self.documents[doc_idx].frame_playback.phase = 0.0;
                        self.documents[doc_idx].frame_playback.playing = false;
                    }
                    if let Some(frame_id) =
                        self.documents[doc_idx].frame_playback.selected_frame_field
                        && ui.button("Open Data").clicked()
                    {
                        self.open_stored_frame_field_panel(doc_idx, frame_id);
                    }
                });
                ui.add(
                    egui::Slider::new(&mut self.documents[doc_idx].frame_playback.phase, 0.0..=1.0)
                        .text("Frame Phase"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.documents[doc_idx].frame_playback.speed,
                        0.05..=2.0,
                    )
                    .text("Playback Speed"),
                );
                if let Some(frame_id) = self.documents[doc_idx].frame_playback.selected_frame_field
                {
                    ui.label("Attachments");
                    for attachment in self.documents[doc_idx]
                        .frame_attachments
                        .iter_mut()
                        .filter(|attachment| attachment.frame_field_id == frame_id)
                    {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut attachment.enabled, &attachment.name);
                            match attachment.kind {
                                crate::document::FrameAttachmentKind::Marker
                                | crate::document::FrameAttachmentKind::Triad
                                | crate::document::FrameAttachmentKind::ProfileRing => {
                                    ui.add(
                                        egui::Slider::new(&mut attachment.scale, 0.1..=4.0)
                                            .text("Scale"),
                                    );
                                }
                                crate::document::FrameAttachmentKind::Camera => {
                                    ui.label(
                                        egui::RichText::new("Follows camera target only")
                                            .small()
                                            .weak(),
                                    );
                                }
                            }
                        });
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new(
                        "Generate a moving frame for the selected plot to enable frame playback.",
                    )
                    .small()
                    .weak(),
                );
            }
        }

        if show_all || has_interpolation {
            ui.add_space(8.0);
            ui.separator();
            ui.label("Interpolation");
            if let Some(groups) = interpolation_groups {
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
        }

        if show_all || has_point_extraction {
            ui.add_space(8.0);
            ui.separator();
            ui.label("Point Extraction");
            if let Some(groups) = polyline_groups {
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
                    self.run_single_plot_analysis(
                        doc_idx,
                        &selected_spec,
                        AnalysisKind::ExtractPoints,
                        vec![],
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
        }

        if show_all || has_surface_geometry {
            ui.add_space(8.0);
            ui.separator();
            ui.label("Surface Geometry");
            if has_surface_geometry {
                ui.label(
                    egui::RichText::new(
                        "Run geometry analysis on the selected surface mesh, including normals and area.",
                    )
                    .small()
                    .weak(),
                );
                ui.horizontal(|ui| {
                    if has_analysis(AnalysisKind::SurfaceNormals)
                        && ui.button("Visualize Normals").clicked()
                    {
                        self.open_surface_normals_modal(plot_idx);
                    }
                    if has_analysis(AnalysisKind::SurfaceArea)
                        && ui.button("Surface Area").clicked()
                    {
                        self.run_surface_plot_analysis(
                            doc_idx,
                            plot_idx,
                            AnalysisKind::SurfaceArea,
                            vec![],
                        );
                    }
                });
            } else {
                ui.label(
                    egui::RichText::new(
                        "Surface geometry analysis is available for surface-like plots with cached mesh geometry.",
                    )
                    .small()
                    .weak(),
                );
            }
        }

        if !(show_all || has_intersections) {
            return;
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label("Intersections");
        ui.label("Curves");
        if self.documents[doc_idx].intersection_cache.is_empty() {
            ui.label(
                egui::RichText::new("No cached curve intersections in the current scene.")
                    .small()
                    .weak(),
            );
        } else if ui.button("Create Intersection Markers").clicked() {
            let points = self.documents[doc_idx]
                .intersection_cache
                .iter()
                .map(|point| point.to_array())
                .collect::<Vec<_>>();
            self.push_analysis_plot(
                doc_idx,
                PlotEntry {
                    plot_id: 0,
                    parent_plot_id: None,
                    relationship: crate::plot::entry::PlotRelationship::Primary,
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
                        show_labels: false,
                    },
                },
            );
        }

        ui.add_space(6.0);
        ui.label("Surfaces");
        if !selected.kind.supports_surface_intersection() {
            ui.label(
                egui::RichText::new("Select a surface-like plot to compute surface intersections.")
                    .small()
                    .weak(),
            );
            return;
        }

        let candidates = self.surface_intersection_candidates(doc_idx, plot_idx);
        if candidates.is_empty() {
            ui.label(
                egui::RichText::new(
                    "No other compatible surface plots are available in this document.",
                )
                .small()
                .weak(),
            );
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
        let selected_idx = self.append_plot_entry(doc_idx, plot);
        self.documents[doc_idx].selected_plot = Some(selected_idx);
        self.documents[doc_idx].viewport_selection_hidden_for = None;
        self.mark_dirty();
    }

    fn push_analysis_output(&mut self, doc_idx: usize, output: AnalysisOutput) {
        let source_plot_idx = self.documents[doc_idx].selected_plot;
        let source_plot_id = source_plot_idx.and_then(|idx| {
            self.documents[doc_idx]
                .plots
                .get(idx)
                .map(|plot| plot.plot_id)
        });
        match output {
            AnalysisOutput::DerivedPlots { plots, .. } => {
                for plot in plots {
                    let entry = PlotEntry::from_plot_spec(plot);
                    let entry = if let Some(parent_plot_id) = source_plot_id {
                        entry.as_analysis_child(parent_plot_id)
                    } else {
                        entry
                    };
                    self.push_analysis_plot(doc_idx, entry);
                }
            }
            AnalysisOutput::Composite {
                plots,
                reports,
                tables,
                diagnostics,
                frame_fields,
                provenance,
            } => {
                if !frame_fields.is_empty() {
                    let source_plot_ids = provenance
                        .source_plots
                        .iter()
                        .filter_map(|name| {
                            self.documents[doc_idx]
                                .plots
                                .iter()
                                .find(|plot| plot.name == *name)
                                .map(|plot| plot.plot_id)
                        })
                        .collect::<Vec<_>>();
                    self.store_frame_fields(
                        doc_idx,
                        source_plot_ids,
                        provenance.source_plots.clone(),
                        frame_fields.clone(),
                    );
                }
                for plot in plots {
                    let entry = PlotEntry::from_plot_spec(plot);
                    let entry = if let Some(parent_plot_id) = source_plot_id {
                        entry.as_analysis_child(parent_plot_id)
                    } else {
                        entry
                    };
                    self.push_analysis_plot(doc_idx, entry);
                }
                if let Some(report) = reports.first() {
                    self.documents[doc_idx].export_status = report
                        .values
                        .iter()
                        .map(|(label, value)| format!("{label}: {value}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                }
                if matches!(
                    provenance.kind,
                    AnalysisKind::PointCloudStatistics
                        | AnalysisKind::DataQualityChecks
                        | AnalysisKind::FrenetFrame
                        | AnalysisKind::BishopFrame
                        | AnalysisKind::SurfaceCurvature
                        | AnalysisKind::SurfaceMeshQuality
                        | AnalysisKind::SurfaceArea
                        | AnalysisKind::CurveSurfaceMeasurement
                ) && (!reports.is_empty()
                    || !tables.is_empty()
                    || !diagnostics.is_empty()
                    || !frame_fields.is_empty())
                {
                    if let Some(source_plot_idx) = source_plot_idx {
                        self.set_selected_plot(doc_idx, Some(source_plot_idx));
                    }
                    self.open_analysis_results_panel(
                        format!("Analysis: {}", provenance.source_plots.join(", ")),
                        doc_idx,
                        source_plot_idx.unwrap_or_default(),
                        reports,
                        tables,
                        diagnostics,
                        frame_fields,
                        provenance,
                    );
                }
            }
            AnalysisOutput::Report { report, provenance } => {
                self.documents[doc_idx].export_status = report
                    .values
                    .iter()
                    .cloned()
                    .map(|(label, value)| format!("{label}: {value}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                if matches!(
                    provenance.kind,
                    AnalysisKind::PointCloudStatistics
                        | AnalysisKind::DataQualityChecks
                        | AnalysisKind::FrenetFrame
                        | AnalysisKind::BishopFrame
                        | AnalysisKind::SurfaceCurvature
                        | AnalysisKind::SurfaceMeshQuality
                        | AnalysisKind::SurfaceArea
                        | AnalysisKind::CurveSurfaceMeasurement
                ) {
                    self.open_analysis_results_panel(
                        report.title.clone(),
                        doc_idx,
                        source_plot_idx.unwrap_or_default(),
                        vec![report],
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        provenance,
                    );
                }
            }
            AnalysisOutput::Table { table, provenance } => {
                self.documents[doc_idx].export_status =
                    format!("Generated analysis table with {} row(s).", table.rows.len());
                if matches!(
                    provenance.kind,
                    AnalysisKind::PointCloudStatistics
                        | AnalysisKind::DataQualityChecks
                        | AnalysisKind::FrenetFrame
                        | AnalysisKind::BishopFrame
                        | AnalysisKind::SurfaceCurvature
                        | AnalysisKind::SurfaceMeshQuality
                        | AnalysisKind::SurfaceArea
                        | AnalysisKind::CurveSurfaceMeasurement
                ) {
                    self.open_analysis_results_panel(
                        format!("Analysis Table: {}", provenance.source_plots.join(", ")),
                        doc_idx,
                        source_plot_idx.unwrap_or_default(),
                        Vec::new(),
                        vec![table],
                        Vec::new(),
                        Vec::new(),
                        provenance,
                    );
                }
            }
        }
    }

    pub(crate) fn run_single_plot_analysis(
        &mut self,
        doc_idx: usize,
        plot: &poincare_lib::PlotSpec,
        kind: AnalysisKind,
        parameters: Vec<(String, String)>,
    ) {
        let request = AnalysisRequest {
            kind,
            target: AnalysisTarget::Plot {
                index: doc_idx,
                name: Some(plot.name.clone()),
            },
            parameters,
        };
        match run_analysis(plot, &request) {
            Ok(output) => {
                self.documents[doc_idx].export_status.clear();
                self.push_analysis_output(doc_idx, output);
            }
            Err(error) => {
                self.documents[doc_idx].export_status = error.diagnostic.to_string();
            }
        }
    }

    pub(crate) fn run_surface_plot_analysis(
        &mut self,
        doc_idx: usize,
        plot_idx: usize,
        kind: AnalysisKind,
        parameters: Vec<(String, String)>,
    ) {
        let plot_spec = self.documents[doc_idx].plots[plot_idx].to_plot_spec();
        let pick_id = (plot_idx + 1) as u64;
        let probe_data = self.documents[doc_idx].scene.probe_data();
        let surfaces = probe_data
            .surfaces
            .iter()
            .filter(|surface| surface.pick_id == pick_id)
            .map(|surface| surface.mesh)
            .collect::<Vec<_>>();
        if surfaces.is_empty() {
            self.documents[doc_idx].export_status =
                "Surface analysis failed: the selected plot has no cached surface mesh."
                    .to_string();
            return;
        }

        match run_surface_mesh_analysis(&plot_spec, kind, &surfaces, &parameters) {
            Ok(output) => {
                self.documents[doc_idx].export_status.clear();
                self.push_analysis_output(doc_idx, output);
            }
            Err(error) => {
                self.documents[doc_idx].export_status = error.diagnostic.to_string();
            }
        }
    }

    fn surface_intersection_candidates(
        &self,
        doc_idx: usize,
        source_idx: usize,
    ) -> Vec<(usize, String)> {
        self.documents[doc_idx]
            .plots
            .iter()
            .enumerate()
            .filter(|(index, plot)| {
                *index != source_idx && plot.kind.supports_surface_intersection()
            })
            .map(|(index, plot)| (index, plot.name.clone()))
            .collect()
    }

    pub(crate) fn surface_frame_candidates(
        &self,
        doc_idx: usize,
        source_idx: usize,
    ) -> Vec<(usize, String)> {
        self.surface_intersection_candidates(doc_idx, source_idx)
    }

    fn run_curve_surface_frame_analysis(
        &mut self,
        doc_idx: usize,
        curve_plot_idx: usize,
        surface_plot_idx: usize,
        kind: AnalysisKind,
        parameters: Vec<(String, String)>,
    ) {
        let curve_spec = self.documents[doc_idx].plots[curve_plot_idx].to_plot_spec();
        let surface_plot = self.documents[doc_idx].plots[surface_plot_idx].clone();
        let surface_pick_id = (surface_plot_idx + 1) as u64;
        let probe_data = self.documents[doc_idx].scene.probe_data();
        let surfaces = probe_data
            .surfaces
            .iter()
            .filter(|surface| surface.pick_id == surface_pick_id)
            .map(|surface| surface.mesh)
            .collect::<Vec<_>>();
        if surfaces.is_empty() {
            self.documents[doc_idx].export_status =
                "Surface-coupled frame analysis failed: target surface has no cached mesh."
                    .to_string();
            return;
        }
        match run_curve_surface_frame_analysis(
            &curve_spec,
            &surface_plot.name,
            kind,
            &surfaces,
            &parameters,
        ) {
            Ok(output) => {
                self.documents[doc_idx].export_status.clear();
                self.push_analysis_output(doc_idx, output);
            }
            Err(error) => {
                self.documents[doc_idx].export_status = error.diagnostic.to_string();
            }
        }
    }

    fn run_curve_surface_measurement_analysis(
        &mut self,
        doc_idx: usize,
        curve_plot_idx: usize,
        surface_plot_idx: usize,
        parameters: Vec<(String, String)>,
    ) {
        let curve_spec = self.documents[doc_idx].plots[curve_plot_idx].to_plot_spec();
        let surface_plot = self.documents[doc_idx].plots[surface_plot_idx].clone();
        let surface_pick_id = (surface_plot_idx + 1) as u64;
        let probe_data = self.documents[doc_idx].scene.probe_data();
        let surfaces = probe_data
            .surfaces
            .iter()
            .filter(|surface| surface.pick_id == surface_pick_id)
            .map(|surface| surface.mesh)
            .collect::<Vec<_>>();
        if surfaces.is_empty() {
            self.documents[doc_idx].export_status =
                "Curve-surface measurement failed: target surface has no cached mesh.".to_string();
            return;
        }
        match run_curve_surface_measurement_analysis(
            &curve_spec,
            &surface_plot.name,
            &surfaces,
            &parameters,
        ) {
            Ok(output) => {
                self.documents[doc_idx].export_status.clear();
                self.push_analysis_output(doc_idx, output);
            }
            Err(error) => {
                self.documents[doc_idx].export_status = error.diagnostic.to_string();
            }
        }
    }

    fn create_surface_intersection_plots(
        &mut self,
        doc_idx: usize,
        source_idx: usize,
        target_idx: usize,
    ) {
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
                    plot_id: 0,
                    parent_plot_id: None,
                    relationship: crate::plot::entry::PlotRelationship::Primary,
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
            let points = all_points
                .iter()
                .map(|point| point.to_array())
                .collect::<Vec<_>>();
            self.push_analysis_plot(
                doc_idx,
                PlotEntry {
                    plot_id: 0,
                    parent_plot_id: None,
                    relationship: crate::plot::entry::PlotRelationship::Primary,
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
                        show_labels: false,
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

        let plot_spec = plot.to_plot_spec();
        let Ok(groups) = sample_groups(&plot_spec, SampleGroupsKind::InterpolationSource) else {
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
            match self.create_interpolated_plot_from_modal(&state, &plot_spec) {
                Ok(()) => {
                    self.interpolate_modal = None;
                    return;
                }
                Err(error) => state.error = error,
            }
        }

        self.interpolate_modal = open.then_some(state);
    }

    pub(crate) fn show_axis_derivative_modal(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.axis_derivative_modal.clone() else {
            return;
        };

        let Some(plot) = self.documents[self.active_document_idx]
            .plots
            .get(state.source_plot_idx)
            .cloned()
        else {
            self.axis_derivative_modal = None;
            return;
        };

        let plot_spec = plot.to_plot_spec();
        let Ok(groups) = sample_groups(&plot_spec, SampleGroupsKind::Curve) else {
            self.axis_derivative_modal = None;
            return;
        };

        let mut open = true;
        let mut create_plot = false;
        let mut cancel_clicked = false;
        egui::Window::new("Differentiate by Axis")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Create a scalar derivative plot from {} sampled curve(s).",
                        groups.len()
                    ))
                    .small()
                    .weak(),
                );
                ui.add_space(8.0);

                egui::ComboBox::from_id_salt("axis_derivative_numerator")
                    .selected_text(axis_derivative_label(state.numerator_axis))
                    .show_ui(ui, |ui| {
                        for axis in 0..3 {
                            ui.selectable_value(
                                &mut state.numerator_axis,
                                axis,
                                axis_derivative_label(axis),
                            );
                        }
                    });
                egui::ComboBox::from_id_salt("axis_derivative_denominator")
                    .selected_text(axis_derivative_against_label(state.denominator_axis))
                    .show_ui(ui, |ui| {
                        for axis in 0..3 {
                            ui.selectable_value(
                                &mut state.denominator_axis,
                                axis,
                                axis_derivative_against_label(axis),
                            );
                        }
                    });
                ui.label(
                    egui::RichText::new(format!(
                        "Will create {}.",
                        axis_derivative_formula(state.numerator_axis, state.denominator_axis)
                    ))
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
                    if ui.button("Create Derivative Plot").clicked() {
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
            match self.create_axis_derivative_plot_from_modal(&state, &plot_spec) {
                Ok(()) => {
                    self.axis_derivative_modal = None;
                    return;
                }
                Err(error) => state.error = error,
            }
        }

        self.axis_derivative_modal = open.then_some(state);
    }

    pub(crate) fn show_fit_curve_modal(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.fit_curve_modal.clone() else {
            return;
        };

        let Some(plot) = self.documents[self.active_document_idx]
            .plots
            .get(state.source_plot_idx)
            .cloned()
        else {
            self.fit_curve_modal = None;
            return;
        };

        let plot_spec = plot.to_plot_spec();
        let Ok(groups) = sample_groups(&plot_spec, SampleGroupsKind::Curve) else {
            self.fit_curve_modal = None;
            return;
        };

        let mut open = true;
        let mut create_plot = false;
        let mut cancel_clicked = false;
        egui::Window::new("Fit Curve")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Fit {} sampled point(s) across {} curve(s).",
                        groups.iter().map(Vec::len).sum::<usize>(),
                        groups.len()
                    ))
                    .small()
                    .weak(),
                );
                ui.add_space(8.0);

                let previous_method = state.method;
                ui.label("Method");
                egui::ComboBox::from_id_salt("fit_curve_method")
                    .selected_text(curve_fit_method_label(state.method))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.method,
                            crate::FitCurveMethodUi::Polynomial,
                            curve_fit_method_label(crate::FitCurveMethodUi::Polynomial),
                        );
                        ui.selectable_value(
                            &mut state.method,
                            crate::FitCurveMethodUi::RobustPolynomial,
                            curve_fit_method_label(crate::FitCurveMethodUi::RobustPolynomial),
                        );
                        ui.selectable_value(
                            &mut state.method,
                            crate::FitCurveMethodUi::Spline,
                            curve_fit_method_label(crate::FitCurveMethodUi::Spline),
                        );
                        ui.selectable_value(
                            &mut state.method,
                            crate::FitCurveMethodUi::Fourier,
                            curve_fit_method_label(crate::FitCurveMethodUi::Fourier),
                        );
                    });
                if state.method != previous_method {
                    state.output_name =
                        format!("{} {}", curve_fit_method_label(state.method), plot.name);
                }

                match state.method {
                    crate::FitCurveMethodUi::Polynomial
                    | crate::FitCurveMethodUi::RobustPolynomial => {
                        ui.add(egui::Slider::new(&mut state.degree, 1..=12).text("Degree"));
                    }
                    crate::FitCurveMethodUi::Spline => {
                        ui.add(
                            egui::Slider::new(&mut state.smoothing_window, 3..=25)
                                .text("Smoothing Window"),
                        );
                        state.smoothing_window = normalized_window_value(state.smoothing_window);
                        ui.add(
                            egui::Slider::new(&mut state.samples_per_segment, 1..=64)
                                .text("Samples per segment"),
                        );
                    }
                    crate::FitCurveMethodUi::Fourier => {
                        ui.add(
                            egui::Slider::new(&mut state.harmonics, 1..=16).text("Harmonics"),
                        );
                    }
                }

                ui.checkbox(&mut state.show_control_points, "Show control points");
                ui.checkbox(&mut state.show_residual_plot, "Show residual plot");
                ui.label(
                    egui::RichText::new(match state.method {
                        crate::FitCurveMethodUi::Polynomial => {
                            "Least-squares polynomial fit for smooth trend estimation."
                        }
                        crate::FitCurveMethodUi::RobustPolynomial => {
                            "Huber-weighted polynomial fit that resists outliers better than plain least squares."
                        }
                        crate::FitCurveMethodUi::Spline => {
                            "Smooth the sampled data first, then rebuild a dense spline through the filtered path."
                        }
                        crate::FitCurveMethodUi::Fourier => {
                            "Fit a Fourier series to periodic data and resample the resulting waveform."
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
                    if ui.button("Create Fitted Plot").clicked() {
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
            match self.create_fit_curve_from_modal(&state, &plot_spec) {
                Ok(()) => {
                    self.fit_curve_modal = None;
                    return;
                }
                Err(error) => state.error = error,
            }
        }

        self.fit_curve_modal = open.then_some(state);
    }

    pub(crate) fn show_surface_normals_modal(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.surface_normals_modal.clone() else {
            return;
        };

        let Some(plot) = self.documents[self.active_document_idx]
            .plots
            .get(state.source_plot_idx)
            .cloned()
        else {
            self.surface_normals_modal = None;
            return;
        };

        let mut open = true;
        let mut create_plot = false;
        let mut cancel_clicked = false;
        egui::Window::new("Visualize Surface Normals")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Create a derived surface-normal plot for {}.",
                        plot.name
                    ))
                    .small()
                    .weak(),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::Slider::new(&mut state.max_samples, 16..=4096).text("Normal Count"),
                );
                ui.add(
                    egui::Slider::new(&mut state.vector_scale, 0.1..=4.0).text("Vector Scale"),
                );
                ui.label(
                    egui::RichText::new(
                        "Higher counts sample more cached surface vertices. Vector scale changes the displayed normal length.",
                    )
                    .small()
                    .weak(),
                );
                if !state.error.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(255, 110, 110), &state.error);
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Create Normal Plot").clicked() {
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
            self.run_surface_plot_analysis(
                self.active_document_idx,
                state.source_plot_idx,
                AnalysisKind::SurfaceNormals,
                vec![
                    ("max_samples".to_string(), state.max_samples.to_string()),
                    (
                        "vector_scale".to_string(),
                        format!("{:.4}", state.vector_scale),
                    ),
                ],
            );
            self.surface_normals_modal = None;
            return;
        }

        self.surface_normals_modal = open.then_some(state);
    }

    pub(crate) fn show_surface_curvature_modal(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.surface_curvature_modal.clone() else {
            return;
        };

        let Some(plot) = self.documents[self.active_document_idx]
            .plots
            .get(state.source_plot_idx)
            .cloned()
        else {
            self.surface_curvature_modal = None;
            return;
        };

        let mut open = true;
        let mut create_plot = false;
        let mut cancel_clicked = false;
        egui::Window::new("Surface Curvature")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Create a coloured curvature surface for {}.",
                        plot.name
                    ))
                    .small()
                    .weak(),
                );
                ui.add_space(8.0);
                egui::ComboBox::from_label("Quantity")
                    .selected_text(surface_curvature_quantity_label(state.quantity))
                    .show_ui(ui, |ui| {
                        for quantity in [
                            SurfaceCurvatureQuantityUi::Mean,
                            SurfaceCurvatureQuantityUi::Gaussian,
                            SurfaceCurvatureQuantityUi::PrincipalMax,
                            SurfaceCurvatureQuantityUi::PrincipalMin,
                        ] {
                            ui.selectable_value(
                                &mut state.quantity,
                                quantity,
                                surface_curvature_quantity_label(quantity),
                            );
                        }
                    });
                ui.checkbox(&mut state.show_extrema, "Add ridge/valley markers");
                if !state.error.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(255, 110, 110), &state.error);
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Create Curvature Surface").clicked() {
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
            self.run_surface_plot_analysis(
                self.active_document_idx,
                state.source_plot_idx,
                AnalysisKind::SurfaceCurvature,
                vec![
                    (
                        "quantity".to_string(),
                        surface_curvature_quantity_key(state.quantity).to_string(),
                    ),
                    ("show_extrema".to_string(), state.show_extrema.to_string()),
                ],
            );
            self.surface_curvature_modal = None;
            return;
        }

        self.surface_curvature_modal = open.then_some(state);
    }

    pub(crate) fn show_curve_surface_measurement_modal(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.curve_surface_measurement_modal.clone() else {
            return;
        };
        let doc_idx = self.active_document_idx;
        let surface_candidates = self.surface_frame_candidates(doc_idx, state.source_plot_idx);
        if state
            .target_surface_idx
            .is_none_or(|target| !surface_candidates.iter().any(|(index, _)| *index == target))
        {
            state.target_surface_idx = surface_candidates.first().map(|(index, _)| *index);
        }
        let Some(plot) = self.documents[doc_idx]
            .plots
            .get(state.source_plot_idx)
            .cloned()
        else {
            self.curve_surface_measurement_modal = None;
            return;
        };

        let mut open = true;
        let mut create_output = false;
        let mut cancel_clicked = false;
        egui::Window::new("Curve-on-Surface Measurement")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Project {} onto a target surface and measure deviation.",
                        plot.name
                    ))
                    .small()
                    .weak(),
                );
                ui.add_space(8.0);
                egui::ComboBox::from_label("Target Surface")
                    .selected_text(
                        state
                            .target_surface_idx
                            .and_then(|target| {
                                surface_candidates
                                    .iter()
                                    .find(|(index, _)| *index == target)
                                    .map(|(_, label)| label.clone())
                            })
                            .unwrap_or_else(|| "Select target".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        for (index, label) in &surface_candidates {
                            ui.selectable_value(&mut state.target_surface_idx, Some(*index), label);
                        }
                    });
                ui.add(egui::Slider::new(&mut state.max_samples, 16..=4096).text("Sample Count"));
                ui.add(
                    egui::Slider::new(&mut state.vector_scale, 0.1..=4.0)
                        .text("Deviation Vector Scale"),
                );
                if !state.error.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(255, 110, 110), &state.error);
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Measure").clicked() {
                        create_output = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            });

        if cancel_clicked {
            open = false;
        }
        if create_output {
            if let Some(target_surface_idx) = state.target_surface_idx {
                self.run_curve_surface_measurement_analysis(
                    doc_idx,
                    state.source_plot_idx,
                    target_surface_idx,
                    vec![
                        ("max_samples".to_string(), state.max_samples.to_string()),
                        (
                            "vector_scale".to_string(),
                            format!("{:.4}", state.vector_scale),
                        ),
                    ],
                );
                self.curve_surface_measurement_modal = None;
                return;
            }
            state.error = "Target surface is required.".to_string();
            self.curve_surface_measurement_modal = Some(state);
            return;
        }

        self.curve_surface_measurement_modal = open.then_some(state);
    }

    fn show_inline_moving_frame_controls(
        &mut self,
        ui: &mut egui::Ui,
        doc_idx: usize,
        plot_idx: usize,
        plot: &PlotEntry,
        surface_candidates: &[(usize, String)],
    ) {
        let Some(mut state) = self.moving_frame_modal.clone() else {
            return;
        };
        if state.source_plot_idx != plot_idx {
            return;
        }
        if requires_surface_target(state.analysis_kind)
            && state
                .target_surface_idx
                .is_none_or(|target| !surface_candidates.iter().any(|(index, _)| *index == target))
        {
            state.target_surface_idx = surface_candidates.first().map(|(index, _)| *index);
        }

        let mut create_output = false;
        let mut cancel = false;
        ui.add_space(6.0);
        ui.group(|ui| {
            ui.label(egui::RichText::new(frame_analysis_label(state.analysis_kind)).strong());
            ui.label(
                egui::RichText::new(format!(
                    "Build a reusable sampled frame field for {}.",
                    plot.name
                ))
                .small()
                .weak(),
            );
            ui.add_space(6.0);
            ui.add(egui::Slider::new(&mut state.max_samples, 8..=1024).text("Displayed Samples"));
            ui.add(egui::Slider::new(&mut state.vector_scale, 0.1..=4.0).text("Vector Scale"));
            if requires_surface_target(state.analysis_kind) {
                egui::ComboBox::from_label("Target Surface")
                    .selected_text(
                        state
                            .target_surface_idx
                            .and_then(|target| {
                                surface_candidates
                                    .iter()
                                    .find(|(index, _)| *index == target)
                                    .map(|(_, label)| label.clone())
                            })
                            .unwrap_or_else(|| "Select target".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        for (index, label) in surface_candidates {
                            ui.selectable_value(&mut state.target_surface_idx, Some(*index), label);
                        }
                    });
            }
            ui.label(
                egui::RichText::new(
                    "This stores frame samples for playback and attachments. It does not add tangent, normal, or binormal plots.",
                )
                .small()
                .weak(),
            );
            if !state.error.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(255, 110, 110), &state.error);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Create FrameField").clicked() {
                    create_output = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

        if cancel {
            self.moving_frame_modal = None;
            return;
        }
        if create_output {
            let parameters = vec![
                ("max_samples".to_string(), state.max_samples.to_string()),
                (
                    "vector_scale".to_string(),
                    format!("{:.4}", state.vector_scale),
                ),
            ];
            if requires_surface_target(state.analysis_kind) {
                if let Some(target_surface_idx) = state.target_surface_idx {
                    self.run_curve_surface_frame_analysis(
                        doc_idx,
                        state.source_plot_idx,
                        target_surface_idx,
                        state.analysis_kind,
                        parameters,
                    );
                    self.moving_frame_modal = None;
                    return;
                }
                state.error = "Target surface is required.".to_string();
                self.moving_frame_modal = Some(state);
                return;
            }
            self.run_single_plot_analysis(
                doc_idx,
                &plot.to_plot_spec(),
                state.analysis_kind,
                parameters,
            );
            self.moving_frame_modal = None;
            return;
        }
        self.moving_frame_modal = Some(state);
    }

    pub(crate) fn open_interpolate_modal(&mut self, plot_idx: usize, plot: &PlotEntry) {
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

    pub(crate) fn open_axis_derivative_modal(&mut self, plot_idx: usize, plot: &PlotEntry) {
        let numerator_axis = 1;
        let denominator_axis = 0;
        self.axis_derivative_modal = Some(crate::AxisDerivativeModalState {
            source_plot_idx: plot_idx,
            numerator_axis,
            denominator_axis,
            output_name: format!(
                "d{}/d{} {}",
                axis_name(numerator_axis),
                axis_name(denominator_axis),
                plot.name
            ),
            error: String::new(),
        });
    }

    pub(crate) fn open_fit_curve_modal(&mut self, plot_idx: usize, plot: &PlotEntry) {
        let method = crate::FitCurveMethodUi::Polynomial;
        self.fit_curve_modal = Some(crate::FitCurveModalState {
            source_plot_idx: plot_idx,
            method,
            output_name: format!("{} {}", curve_fit_method_label(method), plot.name),
            degree: 5,
            harmonics: 3,
            smoothing_window: 7,
            samples_per_segment: 8,
            show_control_points: true,
            show_residual_plot: true,
            error: String::new(),
        });
    }

    pub(crate) fn open_surface_normals_modal(&mut self, plot_idx: usize) {
        self.surface_normals_modal = Some(crate::SurfaceNormalsModalState {
            source_plot_idx: plot_idx,
            max_samples: 512,
            vector_scale: 1.0,
            error: String::new(),
        });
    }

    pub(crate) fn open_surface_curvature_modal(&mut self, plot_idx: usize) {
        self.surface_curvature_modal = Some(crate::SurfaceCurvatureModalState {
            source_plot_idx: plot_idx,
            quantity: SurfaceCurvatureQuantityUi::Mean,
            show_extrema: true,
            error: String::new(),
        });
    }

    pub(crate) fn open_curve_surface_measurement_modal(
        &mut self,
        plot_idx: usize,
        target_surface_idx: Option<usize>,
    ) {
        self.curve_surface_measurement_modal = Some(crate::CurveSurfaceMeasurementModalState {
            source_plot_idx: plot_idx,
            target_surface_idx,
            max_samples: 512,
            vector_scale: 1.0,
            error: String::new(),
        });
    }

    pub(crate) fn open_moving_frame_modal(
        &mut self,
        plot_idx: usize,
        kind: AnalysisKind,
        target_surface_idx: Option<usize>,
    ) {
        self.inspector_tab = crate::InspectorTab::Analysis;
        self.pending_focus_tab = Some(crate::dock::DockTab::PlotProperties);
        self.moving_frame_modal = Some(crate::MovingFrameModalState {
            source_plot_idx: plot_idx,
            analysis_kind: kind,
            target_surface_idx,
            max_samples: 128,
            vector_scale: 1.0,
            error: String::new(),
        });
    }

    fn create_interpolated_plot_from_modal(
        &mut self,
        state: &crate::InterpolateModalState,
        source_plot: &poincare_lib::PlotSpec,
    ) -> Result<(), String> {
        let name = state.output_name.trim();
        if name.is_empty() {
            return Err("Output name is required.".to_string());
        }
        self.run_single_plot_analysis(
            self.active_document_idx,
            source_plot,
            AnalysisKind::InterpolateCurve,
            vec![
                ("output_name".to_string(), name.to_string()),
                (
                    "interpolation_kind".to_string(),
                    interpolation_kind_key(state.interpolation.kind).to_string(),
                ),
                (
                    "samples_per_segment".to_string(),
                    state.interpolation.samples_per_segment.to_string(),
                ),
                ("closed".to_string(), state.interpolation.closed.to_string()),
                (
                    "smoothing_window".to_string(),
                    state.interpolation.smoothing_window.to_string(),
                ),
            ],
        );
        Ok(())
    }

    fn create_axis_derivative_plot_from_modal(
        &mut self,
        state: &crate::AxisDerivativeModalState,
        source_plot: &poincare_lib::PlotSpec,
    ) -> Result<(), String> {
        if state.numerator_axis == state.denominator_axis {
            return Err("Numerator and denominator axes must be different.".to_string());
        }
        let name = state.output_name.trim();
        if name.is_empty() {
            return Err("Output name is required.".to_string());
        }
        self.run_single_plot_analysis(
            self.active_document_idx,
            source_plot,
            AnalysisKind::AxisDerivativeCurve,
            vec![
                (
                    "numerator_axis".to_string(),
                    state.numerator_axis.to_string(),
                ),
                (
                    "denominator_axis".to_string(),
                    state.denominator_axis.to_string(),
                ),
                ("output_name".to_string(), name.to_string()),
            ],
        );
        Ok(())
    }

    fn create_fit_curve_from_modal(
        &mut self,
        state: &crate::FitCurveModalState,
        source_plot: &poincare_lib::PlotSpec,
    ) -> Result<(), String> {
        let name = state.output_name.trim();
        if name.is_empty() {
            return Err("Output name is required.".to_string());
        }
        self.run_single_plot_analysis(
            self.active_document_idx,
            source_plot,
            AnalysisKind::FitCurve,
            vec![
                (
                    "fit_method".to_string(),
                    curve_fit_method_key(state.method).to_string(),
                ),
                ("output_name".to_string(), name.to_string()),
                ("degree".to_string(), state.degree.to_string()),
                ("harmonics".to_string(), state.harmonics.to_string()),
                (
                    "smoothing_window".to_string(),
                    state.smoothing_window.to_string(),
                ),
                (
                    "samples_per_segment".to_string(),
                    state.samples_per_segment.to_string(),
                ),
                (
                    "show_control_points".to_string(),
                    state.show_control_points.to_string(),
                ),
                (
                    "show_residual_plot".to_string(),
                    state.show_residual_plot.to_string(),
                ),
            ],
        );
        if self.documents[self.active_document_idx]
            .export_status
            .starts_with("Method:")
        {
            Ok(())
        } else if self.documents[self.active_document_idx]
            .export_status
            .is_empty()
        {
            Ok(())
        } else {
            Err(self.documents[self.active_document_idx]
                .export_status
                .clone())
        }
    }
}

fn interpolation_kind_label(kind: CurveInterpolationKind) -> &'static str {
    match kind {
        CurveInterpolationKind::Linear => "Polyline (Linear)",
        CurveInterpolationKind::CatmullRom => "Interpolation (Catmull-Rom)",
        CurveInterpolationKind::CentripetalCatmullRom => "Interpolation (Centripetal Catmull-Rom)",
        CurveInterpolationKind::MovingAverage => "Smoothing (Moving Average)",
        CurveInterpolationKind::SavitzkyGolay => "Smoothing (Savitzky-Golay)",
    }
}

fn interpolation_kind_key(kind: CurveInterpolationKind) -> &'static str {
    match kind {
        CurveInterpolationKind::Linear => "linear",
        CurveInterpolationKind::CatmullRom => "catmull_rom",
        CurveInterpolationKind::CentripetalCatmullRom => "centripetal_catmull_rom",
        CurveInterpolationKind::MovingAverage => "moving_average",
        CurveInterpolationKind::SavitzkyGolay => "savitzky_golay",
    }
}

fn curve_fit_method_label(method: crate::FitCurveMethodUi) -> &'static str {
    match method {
        crate::FitCurveMethodUi::Polynomial => "Fit (Polynomial)",
        crate::FitCurveMethodUi::RobustPolynomial => "Fit (Robust Polynomial)",
        crate::FitCurveMethodUi::Spline => "Fit (Spline / Smoothed Catmull-Rom)",
        crate::FitCurveMethodUi::Fourier => "Fit (Fourier Series)",
    }
}

fn curve_fit_method_key(method: crate::FitCurveMethodUi) -> &'static str {
    match method {
        crate::FitCurveMethodUi::Polynomial => "polynomial",
        crate::FitCurveMethodUi::RobustPolynomial => "robust_polynomial",
        crate::FitCurveMethodUi::Spline => "spline",
        crate::FitCurveMethodUi::Fourier => "fourier",
    }
}

fn frame_analysis_label(kind: AnalysisKind) -> &'static str {
    match kind {
        AnalysisKind::FrenetFrame => "Frenet Frame",
        AnalysisKind::BishopFrame => "Bishop Frame",
        _ => "Moving Frame",
    }
}

fn requires_surface_target(kind: AnalysisKind) -> bool {
    matches!(
        kind,
        AnalysisKind::DarbouxFrame | AnalysisKind::SurfaceAlignedFrame
    )
}

fn surface_curvature_quantity_label(quantity: SurfaceCurvatureQuantityUi) -> &'static str {
    match quantity {
        SurfaceCurvatureQuantityUi::Mean => "Mean Curvature",
        SurfaceCurvatureQuantityUi::Gaussian => "Gaussian Curvature",
        SurfaceCurvatureQuantityUi::PrincipalMax => "Principal Max",
        SurfaceCurvatureQuantityUi::PrincipalMin => "Principal Min",
    }
}

fn surface_curvature_quantity_key(quantity: SurfaceCurvatureQuantityUi) -> &'static str {
    match quantity {
        SurfaceCurvatureQuantityUi::Mean => "mean_curvature",
        SurfaceCurvatureQuantityUi::Gaussian => "gaussian_curvature",
        SurfaceCurvatureQuantityUi::PrincipalMax => "k_max",
        SurfaceCurvatureQuantityUi::PrincipalMin => "k_min",
    }
}

fn axis_derivative_label(axis: usize) -> &'static str {
    match axis {
        0 => "Differentiate X",
        1 => "Differentiate Y",
        _ => "Differentiate Z",
    }
}

fn axis_derivative_against_label(axis: usize) -> &'static str {
    match axis {
        0 => "Against X",
        1 => "Against Y",
        _ => "Against Z",
    }
}

fn axis_name(axis: usize) -> &'static str {
    match axis {
        0 => "x",
        1 => "y",
        _ => "z",
    }
}

fn axis_derivative_formula(numerator_axis: usize, denominator_axis: usize) -> String {
    format!(
        "d{}/d{} plotted on the selected curve axes",
        axis_name(numerator_axis),
        axis_name(denominator_axis)
    )
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

fn sampled_curve_positions(
    points: &[[f32; 3]],
    interpolation: CurveInterpolation,
) -> Vec<[f32; 3]> {
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
        PlotKind::DerivedSurfaceMesh {
            positions,
            value_name,
            ..
        } => {
            format!(
                "Derived surface, {} vertices coloured by {}",
                positions.len(),
                value_name
            )
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
        } => format!(
            "Streamlines, step {:.3}, max {} steps",
            step_size, max_steps
        ),
    }
}

fn plot_metadata_clipboard_json(plot: &PlotEntry) -> serde_json::Value {
    let plot_spec = plot.to_plot_spec();
    let metadata = plot_spec.metadata();
    let analyses = available_analyses(&plot_spec)
        .into_iter()
        .map(|capability| {
            json!({
                "kind": format!("{:?}", capability.kind),
                "target_kind": format!("{:?}", capability.target_kind),
                "output_kind": format!("{:?}", capability.output_kind),
                "parameters": capability.parameters,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "plot_name": plot.name,
        "plot_kind": plot_kind_name(&plot.kind),
        "summary": plot_properties_summary(plot),
        "visible": plot.visible,
        "metadata": {
            "coordinate_semantics": format!("{:?}", metadata.coordinate_semantics),
            "domain_editor": domain_editor_metadata_json(&metadata.domain_editor),
            "required_variables": metadata.required_variables,
            "uses_resolution": metadata.uses_resolution,
            "uses_seed_resolution": metadata.uses_seed_resolution,
            "supports_surface_intersection": metadata.supports_surface_intersection,
            "style_capabilities": {
                "mesh": metadata.style_caps.mesh,
                "line": metadata.style_caps.line,
                "point": metadata.style_caps.point,
                "glyph": metadata.style_caps.glyph,
            },
            "default_domain": metadata.default_domain,
            "default_resolution": metadata.default_resolution,
        },
        "data_shape": plot_data_shape_json(plot),
        "sample_groups": {
            "sample_data": sample_group_summary_json(&plot_spec, SampleGroupsKind::SampleData),
            "curve": sample_group_summary_json(&plot_spec, SampleGroupsKind::Curve),
            "polyline": sample_group_summary_json(&plot_spec, SampleGroupsKind::Polyline),
            "interpolation_source": sample_group_summary_json(
                &plot_spec,
                SampleGroupsKind::InterpolationSource,
            ),
        },
        "available_analyses": analyses,
    })
}

fn plot_kind_name(kind: &PlotKind) -> &'static str {
    match kind {
        PlotKind::ContouredSurface { .. } => "ContouredSurface",
        PlotKind::SphericalHarmonic => "SphericalHarmonic",
        PlotKind::HelixCurve => "HelixCurve",
        PlotKind::ScatterCloud => "ScatterCloud",
        PlotKind::VectorField => "VectorField",
        PlotKind::GridSurface => "GridSurface",
        PlotKind::Streamlines { .. } => "Streamlines",
        PlotKind::VolumeRender { .. } => "VolumeRender",
        PlotKind::Isosurface { .. } => "Isosurface",
        PlotKind::ExprCartesian { .. } => "ExprCartesian",
        PlotKind::ExprCurve { .. } => "ExprCurve",
        PlotKind::ExprCartesianLine { .. } => "ExprCartesianLine",
        PlotKind::ExprSpherical { .. } => "ExprSpherical",
        PlotKind::ExprCylindrical { .. } => "ExprCylindrical",
        PlotKind::ExprPolar { .. } => "ExprPolar",
        PlotKind::ExprParametricSurface { .. } => "ExprParametricSurface",
        PlotKind::ImportedTable { .. } => "ImportedTable",
        PlotKind::ScalarSlice { .. } => "ScalarSlice",
        PlotKind::VectorSlice { .. } => "VectorSlice",
        PlotKind::GradientField { .. } => "GradientField",
        PlotKind::DivergenceField { .. } => "DivergenceField",
        PlotKind::CurlField { .. } => "CurlField",
        PlotKind::PointAnnotations { .. } => "PointAnnotations",
        PlotKind::ArrowAnnotations { .. } => "ArrowAnnotations",
        PlotKind::DerivedSurfaceMesh { .. } => "DerivedSurfaceMesh",
        PlotKind::DerivedPolylineGroups { .. } => "DerivedPolylineGroups",
        PlotKind::InterpolatedCurve { .. } => "InterpolatedCurve",
        PlotKind::ExprVectorField { .. } => "ExprVectorField",
        PlotKind::ExprVolume { .. } => "ExprVolume",
        PlotKind::ExprIsosurface { .. } => "ExprIsosurface",
        PlotKind::ExprStreamlines { .. } => "ExprStreamlines",
    }
}

fn domain_editor_metadata_json(metadata: &poincare_lib::DomainEditorMetadata) -> serde_json::Value {
    match metadata {
        poincare_lib::DomainEditorMetadata::Fixed => json!({
            "kind": "Fixed",
            "editable_axis_count": metadata.editable_axis_count(),
        }),
        poincare_lib::DomainEditorMetadata::One { primary } => json!({
            "kind": "One",
            "editable_axis_count": metadata.editable_axis_count(),
            "primary": primary,
        }),
        poincare_lib::DomainEditorMetadata::Two { primary, secondary } => json!({
            "kind": "Two",
            "editable_axis_count": metadata.editable_axis_count(),
            "primary": primary,
            "secondary": secondary,
        }),
        poincare_lib::DomainEditorMetadata::Three { x, y, z } => json!({
            "kind": "Three",
            "editable_axis_count": metadata.editable_axis_count(),
            "x": x,
            "y": y,
            "z": z,
        }),
    }
}

fn plot_data_shape_json(plot: &PlotEntry) -> serde_json::Value {
    match &plot.kind {
        PlotKind::ContouredSurface { contour_values, .. } => json!({
            "contour_level_count": contour_values.len(),
        }),
        PlotKind::Streamlines { seeds } => json!({
            "seed_count": seeds.len(),
        }),
        PlotKind::Isosurface { isovalues, .. } | PlotKind::ExprIsosurface { isovalues, .. } => {
            json!({
                "isovalue_count": isovalues.len(),
            })
        }
        PlotKind::ImportedTable { definition } => {
            let preview = definition.preview();
            match definition.validate() {
                Ok(TableDataSet::SurfaceGrid { xs, ys, zs }) => json!({
                    "table_target": definition.target.label(),
                    "preview_row_count": preview.rows.len(),
                    "preview_column_count": preview.column_count,
                    "x_count": xs.len(),
                    "y_count": ys.len(),
                    "z_count": zs.len(),
                    "sample_count": zs.len(),
                }),
                Ok(TableDataSet::Curve { groups, .. }) => {
                    let point_count: usize = groups.iter().map(Vec::len).sum();
                    json!({
                        "table_target": definition.target.label(),
                        "preview_row_count": preview.rows.len(),
                        "preview_column_count": preview.column_count,
                        "group_count": groups.len(),
                        "point_count": point_count,
                    })
                }
                Ok(TableDataSet::Scatter {
                    points, scalars, ..
                }) => json!({
                    "table_target": definition.target.label(),
                    "preview_row_count": preview.rows.len(),
                    "preview_column_count": preview.column_count,
                    "point_count": points.len(),
                    "scalar_count": scalars.as_ref().map(Vec::len),
                }),
                Ok(TableDataSet::VectorField { samples, .. }) => json!({
                    "table_target": definition.target.label(),
                    "preview_row_count": preview.rows.len(),
                    "preview_column_count": preview.column_count,
                    "sample_count": samples.len(),
                }),
                Err(errors) => json!({
                    "table_target": definition.target.label(),
                    "preview_row_count": preview.rows.len(),
                    "preview_column_count": preview.column_count,
                    "validation_error_count": errors.len(),
                    "validation_errors": errors.into_iter().map(|error| json!({
                        "row": error.row,
                        "column": error.column,
                        "message": error.message,
                        "display": error.display(),
                    })).collect::<Vec<_>>(),
                }),
            }
        }
        PlotKind::PointAnnotations { points, .. } => json!({
            "point_count": points.len(),
        }),
        PlotKind::ArrowAnnotations { arrows, .. } => json!({
            "arrow_count": arrows.len(),
        }),
        PlotKind::DerivedPolylineGroups { groups } => {
            let point_count: usize = groups.iter().map(Vec::len).sum();
            json!({
                "group_count": groups.len(),
                "point_count": point_count,
            })
        }
        PlotKind::InterpolatedCurve {
            points,
            interpolation,
        } => json!({
            "control_point_count": points.len(),
            "sampled_point_count": sampled_curve_positions(points, *interpolation).len(),
            "interpolation_kind": interpolation_kind_label(interpolation.kind),
            "closed": interpolation.closed,
        }),
        PlotKind::ExprStreamlines {
            step_size,
            max_steps,
            ..
        } => json!({
            "step_size": step_size,
            "max_steps": max_steps,
        }),
        _ => json!({}),
    }
}

fn sample_group_summary_json(
    plot_spec: &poincare_lib::PlotSpec,
    kind: SampleGroupsKind,
) -> serde_json::Value {
    match sample_groups(plot_spec, kind) {
        Ok(groups) => {
            let point_count: usize = groups.iter().map(Vec::len).sum();
            json!({
                "supported": true,
                "group_count": groups.len(),
                "point_count": point_count,
                "group_sizes": groups.iter().map(Vec::len).collect::<Vec<_>>(),
            })
        }
        Err(error) => json!({
            "supported": false,
            "error": error.diagnostic.to_string(),
        }),
    }
}
