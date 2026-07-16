use std::path::{Path, PathBuf};

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::document::{Document, human_size, load_document};

const DEFAULT_DOCUMENT_ZOOM: f32 = 1.40;
const MIN_ZOOM: f32 = 0.75;
const MAX_ZOOM: f32 = 2.00;
const ZOOM_STEP: f32 = 0.10;
const TOOLBAR_TEXT_SIZE: f32 = 16.0;

fn quiet_button(ui: &mut egui::Ui, text: &str, tooltip: &str) -> bool {
    ui.add(egui::Button::new(egui::RichText::new(text).size(TOOLBAR_TEXT_SIZE)).frame(false))
        .on_hover_text(tooltip)
        .clicked()
}

fn scale_document_style(ui: &mut egui::Ui, scale: f32) {
    for font in ui.style_mut().text_styles.values_mut() {
        font.size *= scale;
    }

    let spacing = ui.spacing_mut();
    spacing.item_spacing.y *= scale;
    spacing.interact_size *= scale;
    spacing.icon_width *= scale;
    spacing.icon_width_inner *= scale;
    spacing.icon_spacing *= scale;
    spacing.indent *= scale;
}

pub struct ViewerApp {
    document: Option<Document>,
    cache: CommonMarkCache,
    error: Option<String>,
    zoom: f32,
    dark_mode: bool,
    temporary_task_changes: bool,
}

impl ViewerApp {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        initial_path: Option<PathBuf>,
    ) -> Self {
        egui_extras::install_image_loaders(&creation_context.egui_ctx);
        let dark_mode = creation_context.egui_ctx.theme() == egui::Theme::Dark;
        let mut app = Self {
            document: None,
            cache: CommonMarkCache::default(),
            error: None,
            zoom: DEFAULT_DOCUMENT_ZOOM,
            dark_mode,
            temporary_task_changes: false,
        };
        app.apply_theme(&creation_context.egui_ctx);
        if let Some(path) = initial_path {
            app.open_path(&path, &creation_context.egui_ctx);
        }
        app
    }

    fn apply_theme(&self, context: &egui::Context) {
        let mut visuals = if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = if self.dark_mode {
            egui::Color32::from_rgb(25, 27, 32)
        } else {
            egui::Color32::from_rgb(246, 247, 249)
        };
        visuals.extreme_bg_color = if self.dark_mode {
            egui::Color32::from_rgb(18, 20, 24)
        } else {
            egui::Color32::WHITE
        };
        context.set_visuals(visuals);
    }

    fn set_window_title(&self, context: &egui::Context) {
        let title = self
            .document
            .as_ref()
            .map(|document| format!("{} — MD Viewer", document.title()))
            .unwrap_or_else(|| "MD Viewer".to_owned());
        context.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    fn open_path(&mut self, path: &Path, context: &egui::Context) {
        match load_document(path) {
            Ok(document) => {
                self.document = Some(document);
                self.cache = CommonMarkCache::default();
                self.error = None;
                self.temporary_task_changes = false;
                self.set_window_title(context);
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn choose_file(&mut self, context: &egui::Context) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open Markdown file")
            .add_filter("Markdown", &["md", "markdown", "mdown", "mkd"]);
        if let Some(parent) = self
            .document
            .as_ref()
            .and_then(|document| document.path.parent())
        {
            dialog = dialog.set_directory(parent);
        }
        if let Some(path) = dialog.pick_file() {
            self.open_path(&path, context);
        }
    }

    fn reload(&mut self, context: &egui::Context) {
        if let Some(path) = self.document.as_ref().map(|document| document.path.clone()) {
            self.open_path(&path, context);
        }
    }

    fn change_zoom(&mut self, delta: f32) {
        self.zoom = (self.zoom + delta).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn reset_zoom(&mut self) {
        self.zoom = DEFAULT_DOCUMENT_ZOOM;
    }

    fn consume_shortcuts(&mut self, context: &egui::Context) {
        use egui::gui_zoom::kb_shortcuts;

        let open = context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::O,
            ))
        });
        let reload = context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::R,
            )) || input.consume_key(egui::Modifiers::NONE, egui::Key::F5)
        });
        let zoom_in = context.input_mut(|input| {
            input.consume_shortcut(&kb_shortcuts::ZOOM_IN)
                || input.consume_shortcut(&kb_shortcuts::ZOOM_IN_SECONDARY)
        });
        let zoom_out = context.input_mut(|input| input.consume_shortcut(&kb_shortcuts::ZOOM_OUT));
        let zoom_reset =
            context.input_mut(|input| input.consume_shortcut(&kb_shortcuts::ZOOM_RESET));

        if open {
            self.choose_file(context);
        }
        if reload {
            self.reload(context);
        }
        if zoom_in {
            self.change_zoom(ZOOM_STEP);
        }
        if zoom_out {
            self.change_zoom(-ZOOM_STEP);
        }
        if zoom_reset {
            self.reset_zoom();
        }
    }

    fn consume_dropped_files(&mut self, context: &egui::Context) {
        let dropped_files = context.input(|input| input.raw.dropped_files.clone());
        if let Some(path) = dropped_files.into_iter().find_map(|file| file.path) {
            self.open_path(&path, context);
        }
    }

    fn toolbar(&mut self, root_ui: &mut egui::Ui) {
        let context = root_ui.ctx().clone();
        let mut open_clicked = false;
        let mut reload_clicked = false;
        let mut zoom_in_clicked = false;
        let mut zoom_out_clicked = false;
        let mut zoom_reset_clicked = false;
        let mut theme_clicked = false;

        egui::Panel::top("toolbar")
            .exact_size(44.0)
            .show(root_ui, |ui| {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    open_clicked = quiet_button(ui, "Open", "Open a Markdown file · Ctrl+O");
                    ui.add_enabled_ui(self.document.is_some(), |ui| {
                        reload_clicked = quiet_button(ui, "Reload", "Reload · Ctrl+R or F5");
                    });

                    if let Some(document) = &self.document {
                        ui.label(
                            egui::RichText::new(document.title())
                                .size(TOOLBAR_TEXT_SIZE)
                                .weak(),
                        )
                        .on_hover_text(document.display_path());
                    } else {
                        ui.label(
                            egui::RichText::new("No file open")
                                .size(TOOLBAR_TEXT_SIZE)
                                .weak(),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        theme_clicked = quiet_button(
                            ui,
                            if self.dark_mode { "Light" } else { "Dark" },
                            "Toggle color theme",
                        );
                        zoom_in_clicked = quiet_button(ui, "+", "Zoom in · Ctrl++");
                        zoom_reset_clicked = quiet_button(
                            ui,
                            &format!("{:.0}%", self.zoom * 100.0),
                            "Reset zoom · Ctrl+0",
                        );
                        zoom_out_clicked = quiet_button(ui, "−", "Zoom out · Ctrl+-");
                    });
                });
            });

        if open_clicked {
            self.choose_file(&context);
        }
        if reload_clicked {
            self.reload(&context);
        }
        if zoom_in_clicked {
            self.change_zoom(ZOOM_STEP);
        }
        if zoom_out_clicked {
            self.change_zoom(-ZOOM_STEP);
        }
        if zoom_reset_clicked {
            self.reset_zoom();
        }
        if theme_clicked {
            self.dark_mode = !self.dark_mode;
            self.apply_theme(&context);
        }
    }

    fn error_banner(&mut self, root_ui: &mut egui::Ui) {
        let Some(error) = self.error.clone() else {
            return;
        };
        egui::Panel::top("error_banner").show(root_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), error);
                if ui.small_button("Dismiss").clicked() {
                    self.error = None;
                }
            });
        });
    }

    fn document_view(&mut self, ui: &mut egui::Ui) {
        let Some(document) = &mut self.document else {
            ui.with_layout(
                egui::Layout::top_down_justified(egui::Align::Center),
                |ui| {
                    ui.add_space(120.0);
                    ui.heading("Drop a Markdown file here");
                    ui.label("or use Open / Ctrl+O");
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Read-only · no editor · no browser engine").weak(),
                    );
                },
            );
            return;
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let available = ui.available_width();
                let side_space = (available * 0.025).clamp(12.0, 40.0);
                let content_width = (available - 2.0 * side_space).max(64.0);

                ui.horizontal(|ui| {
                    ui.add_space(side_space);
                    ui.vertical(|ui| {
                        ui.set_width(content_width);
                        ui.add_space(20.0);
                        scale_document_style(ui, self.zoom);
                        ui.style_mut().url_in_tooltip = true;
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

                        let mut viewer = CommonMarkViewer::new()
                            .max_image_width(Some(ui.available_width().max(64.0) as usize))
                            .default_width(Some(ui.available_width().max(64.0) as usize))
                            .enable_scroll_to_heading(true);
                        if let Some(base_uri) = &document.image_base_uri {
                            viewer = viewer.default_implicit_uri_scheme(base_uri);
                        }
                        let response = viewer.show_mut(ui, &mut self.cache, &mut document.content);
                        if response.response.changed() {
                            self.temporary_task_changes = true;
                        }
                        ui.add_space(32.0);
                    });
                });
            });
    }

    fn status_bar(&self, root_ui: &mut egui::Ui) {
        let Some(document) = &self.document else {
            return;
        };
        egui::Panel::bottom("status_bar")
            .exact_size(22.0)
            .show(root_ui, |ui| {
                ui.horizontal(|ui| {
                    let task_note = if self.temporary_task_changes {
                        " · temporary task checks"
                    } else {
                        ""
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "{} lines · {} · file read-only{}",
                            document.lines,
                            human_size(document.bytes),
                            task_note
                        ))
                        .small()
                        .weak(),
                    );
                });
            });
    }
}

impl eframe::App for ViewerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.consume_shortcuts(&context);
        self.consume_dropped_files(&context);
        self.toolbar(ui);
        self.error_banner(ui);
        self.status_bar(ui);

        egui::CentralPanel::default().show(ui, |ui| self.document_view(ui));

        if !context.input(|input| input.raw.hovered_files.is_empty()) {
            let painter = context.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop_overlay"),
            ));
            let rect = context.content_rect();
            painter.rect_filled(
                rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(40, 110, 190, 90),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop Markdown file to open",
                egui::FontId::proportional(24.0),
                egui::Color32::WHITE,
            );
        }
    }
}
