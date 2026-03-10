use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, pos2, vec2, Align2, Area, Color32, Context, Id, Key, Painter, PointerButton, Pos2, Rect,
    Response, Sense, Shape as EguiShape, Slider, Stroke, TopBottomPanel,
};
use egui_phosphor::regular as ph;
use uuid::Uuid;

use crate::history::History;
use crate::io::{
    export_png, export_svg, load_project, read_autosave, save_project, write_autosave, ProjectFile,
};
use crate::model::{
    polygon_vertices, star_vertices, BlendMode, Camera, Document, Rgba, Shape, ShapeKind,
    StrokePoint, Style, WorldPoint, WorldRect,
};
use crate::tools::{Tool, TransformHandle};

const AUTOSAVE_EVERY: Duration = Duration::from_secs(12);
const HANDLE_SIZE_PX: f32 = 7.0;
const LASER_LIFETIME: Duration = Duration::from_millis(900);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewTheme {
    Dark,
    Light,
    Slate,
}

impl ViewTheme {
    fn icon(self) -> &'static str {
        match self {
            ViewTheme::Dark => "☾",
            ViewTheme::Light => "☀",
            ViewTheme::Slate => "◩",
        }
    }

    fn next(self) -> Self {
        match self {
            ViewTheme::Dark => ViewTheme::Light,
            ViewTheme::Light => ViewTheme::Slate,
            ViewTheme::Slate => ViewTheme::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GridView {
    Lines,
    Dots,
    Off,
}

impl GridView {
    fn icon(self) -> &'static str {
        match self {
            GridView::Lines => "#",
            GridView::Dots => "•",
            GridView::Off => "×",
        }
    }

    fn next(self) -> Self {
        match self {
            GridView::Lines => GridView::Dots,
            GridView::Dots => GridView::Off,
            GridView::Off => GridView::Lines,
        }
    }
}

pub struct LdrawApp {
    document: Document,
    camera: Camera,
    tool: Tool,
    styles: HashMap<Tool, Style>,
    selection: HashSet<Uuid>,
    history: History<Document>,
    interaction: Option<Interaction>,
    current_file: Option<PathBuf>,
    dirty: bool,
    status: String,
    autosave_at: Instant,
    snap_grid: bool,
    snap_angle: bool,
    grid_step: f64,
    polygon_sides: u8,
    star_points: u8,
    star_inner_ratio: f64,
    edit_fill: bool,
    pending_text: String,
    pending_image_path: String,
    path_input: String,
    export_svg_path: String,
    export_png_path: String,
    show_file_panel: bool,
    theme: ViewTheme,
    grid_view: GridView,
    minimap_collapsed: bool,
    minimap_indicator: Option<(WorldPoint, Instant)>,
    minimap_rect: Option<Rect>,
    pressure_enabled: bool,
    last_pressure: f32,
    laser_trails: Vec<LaserTrail>,
    last_canvas_rect: Option<Rect>,
}

#[derive(Clone)]
enum Interaction {
    Pan {
        start_screen: Pos2,
        start_center: WorldPoint,
    },
    Draw {
        tool: Tool,
        start: WorldPoint,
        current: WorldPoint,
        points: Vec<StrokePoint>,
    },
    Move {
        start: WorldPoint,
        seeds: Vec<Shape>,
    },
    Marquee {
        start: WorldPoint,
        current: WorldPoint,
    },
    Resize {
        handle: TransformHandle,
        anchor: WorldPoint,
        pivot: WorldPoint,
        seeds: Vec<Shape>,
    },
    Rotate {
        center: WorldPoint,
        start_angle: f64,
        seeds: Vec<Shape>,
    },
    Erase,
    Laser {
        points: Vec<WorldPoint>,
    },
}

struct LaserTrail {
    points: Vec<WorldPoint>,
    born: Instant,
}

impl Default for LdrawApp {
    fn default() -> Self {
        let mut styles = HashMap::new();
        for tool in Tool::LEFT_TOOLBAR {
            styles.insert(tool, default_style_for_tool(tool));
        }

        let (mut document, camera) = if let Some(file) = read_autosave() {
            (file.document, file.camera)
        } else {
            (Document::default(), Camera::default())
        };
        document.background = Rgba::rgb(0, 0, 0);

        Self {
            document,
            camera,
            tool: Tool::Select,
            styles,
            selection: HashSet::new(),
            history: History::new(300),
            interaction: None,
            current_file: None,
            dirty: false,
            status: "Ready".to_owned(),
            autosave_at: Instant::now(),
            snap_grid: true,
            snap_angle: true,
            grid_step: 24.0,
            polygon_sides: 6,
            star_points: 5,
            star_inner_ratio: 0.5,
            edit_fill: false,
            pending_text: "Text".to_owned(),
            pending_image_path: String::new(),
            path_input: "board.ldrw".to_owned(),
            export_svg_path: "board.svg".to_owned(),
            export_png_path: "board.png".to_owned(),
            show_file_panel: false,
            theme: ViewTheme::Dark,
            grid_view: GridView::Lines,
            minimap_collapsed: true,
            minimap_indicator: None,
            minimap_rect: None,
            pressure_enabled: true,
            last_pressure: 1.0,
            laser_trails: Vec::new(),
            last_canvas_rect: None,
        }
    }
}

impl eframe::App for LdrawApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.apply_theme_visuals(ctx);
        self.laser_trails
            .retain(|trail| trail.born.elapsed() < LASER_LIFETIME);
        if self
            .minimap_indicator
            .is_some_and(|(_, at)| at.elapsed() > Duration::from_millis(380))
        {
            self.minimap_indicator = None;
        }
        if !self.laser_trails.is_empty() {
            ctx.request_repaint();
        }

        self.refresh_tablet_pressure(ctx);
        self.handle_shortcuts(ctx);
        self.autosave_if_needed();

        TopBottomPanel::top("top_bar")
            .exact_height(34.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.visuals_mut().button_frame = true;
                    ui.label("File");
                    if ui.button("New").on_hover_text("Ctrl+N").clicked() {
                        self.new_document();
                    }
                    if ui.button("Open").on_hover_text("Ctrl+O").clicked() {
                        self.open_document();
                    }
                    if ui.button("Save").on_hover_text("Ctrl+S").clicked() {
                        self.save_document();
                    }
                    if ui.button("Save As").clicked() {
                        self.save_document_as();
                    }
                    if ui.button("Export SVG").clicked() {
                        self.export_document_svg();
                    }
                    if ui.button("Export PNG").clicked() {
                        self.export_document_png();
                    }
                    if ui.button("Paths").on_hover_text("File paths").clicked() {
                        self.show_file_panel = !self.show_file_panel;
                    }
                    ui.separator();
                    if ui
                        .button(self.theme.icon())
                        .on_hover_text("Cycle theme")
                        .clicked()
                    {
                        self.cycle_theme();
                    }
                    if ui
                        .button(self.grid_view.icon())
                        .on_hover_text("Cycle grid: lines/dots/off")
                        .clicked()
                    {
                        self.grid_view = self.grid_view.next();
                    }
                    ui.separator();
                    ui.label(format!("{:>3.0}%", self.camera.zoom * 100.0));
                    ui.separator();
                    ui.small(self.status.clone());
                });
            });

        self.draw_compact_toolbar(ctx);
        self.draw_style_panel(ctx);
        self.draw_file_panel(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            let canvas_size = ui.available_size();
            let (response, painter) = ui.allocate_painter(canvas_size, Sense::click_and_drag());
            let canvas = response.rect;
            self.last_canvas_rect = Some(canvas);

            painter.rect_filled(canvas, 0.0, self.document.background.to_egui());
            if !matches!(self.grid_view, GridView::Off) {
                self.draw_grid(&painter, canvas);
            }
            self.draw_scene(&painter, canvas);
            self.draw_laser_overlay(&painter, canvas);
            self.draw_selection_overlay(&painter, canvas);
            self.draw_draft_overlay(&painter, canvas);

            self.handle_canvas_input(ctx, &response, canvas);
        });

        self.draw_action_strip(ctx);
        self.draw_minimap(ctx);

        if !self.minimap_collapsed
            && ctx.input(|i| i.pointer.primary_pressed())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|pos| self.minimap_rect.is_some_and(|rect| !rect.contains(pos)))
        {
            self.minimap_collapsed = true;
        }
    }
}

impl LdrawApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        Self::default()
    }

    fn cycle_theme(&mut self) {
        self.theme = self.theme.next();
        self.document.background = match self.theme {
            ViewTheme::Dark => Rgba::rgb(0, 0, 0),
            ViewTheme::Light => Rgba::rgb(240, 242, 245),
            ViewTheme::Slate => Rgba::rgb(22, 27, 39),
        };
    }

    fn apply_theme_visuals(&self, ctx: &Context) {
        let visuals = match self.theme {
            ViewTheme::Dark | ViewTheme::Slate => egui::Visuals::dark(),
            ViewTheme::Light => egui::Visuals::light(),
        };
        ctx.set_visuals(visuals);
    }

    fn panel_fill(&self) -> Color32 {
        match self.theme {
            ViewTheme::Dark => Color32::from_rgba_unmultiplied(30, 33, 42, 236),
            ViewTheme::Light => Color32::from_rgba_unmultiplied(248, 248, 250, 228),
            ViewTheme::Slate => Color32::from_rgba_unmultiplied(34, 41, 58, 236),
        }
    }

    fn panel_border(&self) -> Color32 {
        match self.theme {
            ViewTheme::Dark => Color32::from_gray(78),
            ViewTheme::Light => Color32::from_gray(185),
            ViewTheme::Slate => Color32::from_gray(92),
        }
    }

    fn accent(&self) -> Color32 {
        match self.theme {
            ViewTheme::Dark => Color32::from_rgb(37, 125, 244),
            ViewTheme::Light => Color32::from_rgb(29, 108, 225),
            ViewTheme::Slate => Color32::from_rgb(70, 155, 255),
        }
    }

    fn chrome_button_bg(&self) -> Color32 {
        match self.theme {
            ViewTheme::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 16),
            ViewTheme::Light => Color32::from_rgba_unmultiplied(0, 0, 0, 14),
            ViewTheme::Slate => Color32::from_rgba_unmultiplied(255, 255, 255, 18),
        }
    }

    fn grid_color(&self) -> Color32 {
        match self.theme {
            ViewTheme::Dark => Color32::from_rgba_unmultiplied(190, 198, 214, 28),
            ViewTheme::Light => Color32::from_rgba_unmultiplied(100, 110, 130, 36),
            ViewTheme::Slate => Color32::from_rgba_unmultiplied(164, 199, 255, 32),
        }
    }

    fn draw_compact_toolbar(&mut self, ctx: &Context) {
        Area::new(Id::new("compact_toolbar"))
            .anchor(Align2::LEFT_TOP, vec2(10.0, 38.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(self.panel_fill())
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(Stroke::new(1.0, self.panel_border()))
                    .inner_margin(egui::Margin::same(6.0))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                        for tool in Tool::LEFT_TOOLBAR {
                            let selected = self.tool == tool;
                            let (rect, response) =
                                ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                            let fill = if selected {
                                self.accent()
                            } else {
                                self.chrome_button_bg()
                            };
                            ui.painter().rect_filled(rect, 6.0, fill);
                            ui.painter().rect_stroke(
                                rect,
                                6.0,
                                Stroke::new(1.0, self.panel_border()),
                            );
                            let icon_color = if selected {
                                Color32::WHITE
                            } else {
                                Color32::from_gray(215)
                            };
                            paint_tool_icon(ui.painter(), rect.shrink(7.0), tool, icon_color);
                            if response.clicked() {
                                self.tool = tool;
                            }
                            response.on_hover_text(format!("{} ({})", tool.name(), tool.hotkey()));
                        }
                    });
            });
    }

    fn draw_style_panel(&mut self, ctx: &Context) {
        Area::new(Id::new("style_panel_compact"))
            .anchor(Align2::RIGHT_TOP, vec2(-10.0, 38.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(self.panel_fill())
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(Stroke::new(1.0, self.panel_border()))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.set_width(118.0);
                        ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
                        ui.label(self.tool.name());

                        let style = self
                            .styles
                            .entry(self.tool)
                            .or_insert_with(|| default_style_for_tool(self.tool));

                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.edit_fill, false, "Stroke");
                            ui.selectable_value(&mut self.edit_fill, true, "Fill");
                        });

                        draw_palette_grid(ui, style, self.edit_fill);

                        ui.horizontal(|ui| {
                            ui.label("🎨");
                            let mut custom_color = if self.edit_fill {
                                style.fill.unwrap_or(style.stroke)
                            } else {
                                style.stroke
                            }
                            .to_egui();
                            if ui.color_edit_button_srgba(&mut custom_color).changed() {
                                let parsed = Rgba {
                                    r: custom_color.r(),
                                    g: custom_color.g(),
                                    b: custom_color.b(),
                                    a: custom_color.a(),
                                };
                                apply_color(style, self.edit_fill, parsed);
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("╍");
                            ui.add(
                                Slider::new(&mut style.stroke_width, 1.0..=48.0)
                                    .show_value(true)
                                    .trailing_fill(true),
                            )
                            .on_hover_text("Stroke width");
                        });
                        ui.horizontal(|ui| {
                            ui.label("◐");
                            ui.add(
                                Slider::new(&mut style.opacity, 0.05..=1.0)
                                    .show_value(true)
                                    .trailing_fill(true),
                            )
                            .on_hover_text("Opacity");
                        });
                        ui.checkbox(&mut style.dashed, "Dashed");
                        ui.checkbox(&mut self.snap_grid, "Snap grid");
                        ui.checkbox(&mut self.snap_angle, "Snap angle");
                        ui.checkbox(&mut self.pressure_enabled, "Tablet");

                        ui.horizontal(|ui| {
                            ui.label("Blend");
                            ui.selectable_value(&mut style.blend, BlendMode::Normal, "N");
                            ui.selectable_value(&mut style.blend, BlendMode::Highlighter, "H");
                        });

                        if matches!(self.tool, Tool::Polygon) {
                            ui.add(Slider::new(&mut self.polygon_sides, 3..=12).text("Sides"));
                        }
                        if matches!(self.tool, Tool::Star) {
                            ui.add(Slider::new(&mut self.star_points, 3..=12).text("Points"));
                            ui.add(
                                Slider::new(&mut self.star_inner_ratio, 0.2..=0.9).text("Inner"),
                            );
                        }
                        if matches!(self.tool, Tool::Text) {
                            ui.text_edit_singleline(&mut self.pending_text);
                        }
                        if matches!(self.tool, Tool::Image) {
                            ui.text_edit_singleline(&mut self.pending_image_path);
                        }
                    });
            });
    }

    fn draw_file_panel(&mut self, ctx: &Context) {
        if !self.show_file_panel {
            return;
        }
        Area::new(Id::new("file_panel"))
            .anchor(Align2::CENTER_TOP, vec2(0.0, 34.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(self.panel_fill())
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(Stroke::new(1.0, self.panel_border()))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.set_width(420.0);
                        ui.label("File paths");
                        ui.horizontal(|ui| {
                            ui.label("Project");
                            ui.text_edit_singleline(&mut self.path_input);
                        });
                        ui.horizontal(|ui| {
                            ui.label("SVG");
                            ui.text_edit_singleline(&mut self.export_svg_path);
                        });
                        ui.horizontal(|ui| {
                            ui.label("PNG");
                            ui.text_edit_singleline(&mut self.export_png_path);
                        });
                    });
            });
    }

    fn draw_laser_overlay(&self, painter: &Painter, canvas: Rect) {
        let now = Instant::now();
        for trail in &self.laser_trails {
            if trail.points.len() < 2 {
                continue;
            }
            let age = now.saturating_duration_since(trail.born).as_secs_f32();
            let t = 1.0 - (age / LASER_LIFETIME.as_secs_f32()).clamp(0.0, 1.0);
            let alpha = (t * 255.0) as u8;
            let color = Color32::from_rgba_unmultiplied(255, 52, 66, alpha);
            for segment in trail.points.windows(2) {
                let a = self.camera.world_to_screen(segment[0], canvas);
                let b = self.camera.world_to_screen(segment[1], canvas);
                painter.line_segment([a, b], Stroke::new(3.0, color));
            }
        }
    }

    fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn current_style(&self) -> Style {
        self.styles
            .get(&self.tool)
            .cloned()
            .unwrap_or_else(|| default_style_for_tool(self.tool))
    }

    fn autosave_if_needed(&mut self) {
        if !self.dirty || self.autosave_at.elapsed() < AUTOSAVE_EVERY {
            return;
        }

        let file = ProjectFile::new(self.document.clone(), self.camera);
        if write_autosave(&file).is_ok() {
            self.autosave_at = Instant::now();
            self.set_status("Autosaved");
        }
    }

    fn new_document(&mut self) {
        self.history.checkpoint(&self.document);
        self.document = Document::default();
        self.document.background = match self.theme {
            ViewTheme::Dark => Rgba::rgb(0, 0, 0),
            ViewTheme::Light => Rgba::rgb(240, 242, 245),
            ViewTheme::Slate => Rgba::rgb(22, 27, 39),
        };
        self.camera = Camera::default();
        self.selection.clear();
        self.current_file = None;
        self.mark_dirty();
        self.set_status("New document");
    }

    fn open_document(&mut self) {
        let path = PathBuf::from(self.path_input.trim());
        if path.as_os_str().is_empty() {
            self.set_status("Open failed: file path is empty");
            return;
        }

        match load_project(&path) {
            Ok(file) => {
                self.document = file.document;
                self.camera = file.camera;
                self.selection.clear();
                self.current_file = Some(path.clone());
                self.history.clear();
                self.dirty = false;
                self.path_input = path.to_string_lossy().to_string();
                self.set_status(format!("Opened {}", path.display()));
            }
            Err(err) => self.set_status(format!("Open failed: {err:#}")),
        }
    }

    fn save_document(&mut self) {
        if let Some(path) = self.current_file.clone() {
            self.save_to_path(path);
        } else {
            self.save_document_as();
        }
    }

    fn save_document_as(&mut self) {
        let path = PathBuf::from(self.path_input.trim());
        if path.as_os_str().is_empty() {
            self.set_status("Save failed: file path is empty");
            return;
        }
        self.current_file = Some(path.clone());
        self.save_to_path(path);
    }

    fn save_to_path(&mut self, path: PathBuf) {
        let file = ProjectFile::new(self.document.clone(), self.camera);
        match save_project(&path, &file) {
            Ok(()) => {
                self.dirty = false;
                self.set_status(format!("Saved {}", path.display()));
            }
            Err(err) => self.set_status(format!("Save failed: {err:#}")),
        }
    }

    fn export_document_svg(&mut self) {
        let path = PathBuf::from(self.export_svg_path.trim());
        if path.as_os_str().is_empty() {
            self.set_status("SVG export failed: path is empty");
            return;
        }
        match export_svg(&path, &self.document) {
            Ok(()) => self.set_status(format!("Exported {}", path.display())),
            Err(err) => self.set_status(format!("SVG export failed: {err:#}")),
        }
    }

    fn export_document_png(&mut self) {
        let path = PathBuf::from(self.export_png_path.trim());
        if path.as_os_str().is_empty() {
            self.set_status("PNG export failed: path is empty");
            return;
        }
        match export_png(&path, &self.document) {
            Ok(()) => self.set_status(format!("Exported {}", path.display())),
            Err(err) => self.set_status(format!("PNG export failed: {err:#}")),
        }
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let set_tool = |this: &mut Self, tool: Tool| {
            this.tool = tool;
            this.set_status(format!("Tool: {}", tool.name()));
        };

        let cmd = ctx.input(|i| i.modifiers.command);
        let shift = ctx.input(|i| i.modifiers.shift);

        if ctx.input(|i| i.key_pressed(Key::V)) {
            set_tool(self, Tool::Select);
        }
        if ctx.input(|i| i.key_pressed(Key::H)) {
            set_tool(self, Tool::Hand);
        }
        if ctx.input(|i| i.key_pressed(Key::K)) {
            set_tool(self, Tool::Laser);
        }
        if ctx.input(|i| i.key_pressed(Key::P)) {
            set_tool(self, Tool::Pen);
        }
        if ctx.input(|i| i.key_pressed(Key::N)) && !cmd {
            set_tool(self, Tool::Pencil);
        }
        if ctx.input(|i| i.key_pressed(Key::Y)) {
            set_tool(self, Tool::Highlighter);
        }
        if ctx.input(|i| i.key_pressed(Key::E)) {
            set_tool(self, Tool::Eraser);
        }
        if ctx.input(|i| i.key_pressed(Key::L)) {
            set_tool(self, Tool::Line);
        }
        if ctx.input(|i| i.key_pressed(Key::A)) {
            set_tool(self, Tool::Arrow);
        }
        if ctx.input(|i| i.key_pressed(Key::R)) {
            set_tool(self, Tool::Rectangle);
        }
        if ctx.input(|i| i.key_pressed(Key::O)) && !cmd {
            set_tool(self, Tool::Ellipse);
        }
        if ctx.input(|i| i.key_pressed(Key::D)) && !cmd {
            set_tool(self, Tool::Diamond);
        }
        if ctx.input(|i| i.key_pressed(Key::T)) && !cmd {
            set_tool(self, Tool::Triangle);
        }
        if ctx.input(|i| i.key_pressed(Key::G)) {
            set_tool(self, Tool::Polygon);
        }
        if ctx.input(|i| i.key_pressed(Key::S)) && !cmd {
            set_tool(self, Tool::Star);
        }
        if ctx.input(|i| i.key_pressed(Key::X)) {
            set_tool(self, Tool::Text);
        }
        if ctx.input(|i| i.key_pressed(Key::I)) {
            set_tool(self, Tool::Image);
        }

        if cmd && ctx.input(|i| i.key_pressed(Key::Z)) {
            if shift {
                if self.history.redo(&mut self.document) {
                    self.set_status("Redo");
                }
            } else if self.history.undo(&mut self.document) {
                self.selection.clear();
                self.set_status("Undo");
            }
        }

        if cmd && ctx.input(|i| i.key_pressed(Key::S)) {
            self.save_document();
        }
        if cmd && ctx.input(|i| i.key_pressed(Key::O)) {
            self.open_document();
        }
        if cmd && ctx.input(|i| i.key_pressed(Key::N)) {
            self.new_document();
        }
        if cmd && ctx.input(|i| i.key_pressed(Key::D)) {
            self.duplicate_selection();
        }

        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            self.delete_selection();
        }

        if ctx.input(|i| i.key_pressed(Key::Num0)) {
            self.camera.zoom = 1.0;
        }
        if ctx.input(|i| i.key_pressed(Key::Equals)) {
            self.camera.zoom = (self.camera.zoom * 1.12).clamp(0.05, 32.0);
        }
        if ctx.input(|i| i.key_pressed(Key::Minus)) {
            self.camera.zoom = (self.camera.zoom / 1.12).clamp(0.05, 32.0);
        }
    }

    fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.history.checkpoint(&self.document);
        self.document
            .shapes
            .retain(|shape| !self.selection.contains(&shape.id));
        self.selection.clear();
        self.mark_dirty();
        self.set_status("Deleted selection");
    }

    fn duplicate_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.history.checkpoint(&self.document);
        let mut new_ids = HashSet::new();
        let mut clones = Vec::new();
        for shape in &self.document.shapes {
            if self.selection.contains(&shape.id) {
                let mut cloned = shape.clone();
                cloned.id = Uuid::new_v4();
                cloned.translate(36.0, 36.0);
                new_ids.insert(cloned.id);
                clones.push(cloned);
            }
        }
        self.document.shapes.extend(clones);
        self.selection = new_ids;
        self.mark_dirty();
        self.set_status("Duplicated selection");
    }

    fn bring_selection_to_front(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.history.checkpoint(&self.document);
        let mut selected = Vec::new();
        self.document.shapes.retain(|shape| {
            if self.selection.contains(&shape.id) {
                selected.push(shape.clone());
                false
            } else {
                true
            }
        });
        self.document.shapes.extend(selected);
        self.mark_dirty();
    }

    fn send_selection_to_back(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.history.checkpoint(&self.document);
        let mut selected = Vec::new();
        self.document.shapes.retain(|shape| {
            if self.selection.contains(&shape.id) {
                selected.push(shape.clone());
                false
            } else {
                true
            }
        });
        selected.extend(std::mem::take(&mut self.document.shapes));
        self.document.shapes = selected;
        self.mark_dirty();
    }

    fn toggle_lock_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.history.checkpoint(&self.document);
        for shape in &mut self.document.shapes {
            if self.selection.contains(&shape.id) {
                shape.locked = !shape.locked;
            }
        }
        self.mark_dirty();
    }

    fn draw_action_strip(&mut self, ctx: &Context) {
        Area::new(Id::new("action_strip"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -14.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(self.panel_fill())
                    .rounding(egui::Rounding::same(9.0))
                    .stroke(Stroke::new(1.0, self.panel_border()))
                    .inner_margin(egui::Margin::same(6.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Undo").clicked() {
                                if self.history.undo(&mut self.document) {
                                    self.selection.clear();
                                }
                            }
                            if ui.button("Redo").clicked() {
                                self.history.redo(&mut self.document);
                            }
                            if ui.button("Duplicate").clicked() {
                                self.duplicate_selection();
                            }
                            if ui.button("Delete").clicked() {
                                self.delete_selection();
                            }
                            if ui.button("Lock").clicked() {
                                self.toggle_lock_selection();
                            }
                            if ui.button("Front").clicked() {
                                self.bring_selection_to_front();
                            }
                            if ui.button("Back").clicked() {
                                self.send_selection_to_back();
                            }
                        });
                    });
            });
    }

    fn draw_minimap(&mut self, ctx: &Context) {
        let area = Area::new(Id::new("minimap"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-10.0, -10.0))
            .show(ctx, |ui| {
                let size = if self.minimap_collapsed {
                    vec2(84.0, 30.0)
                } else {
                    vec2(118.0, 124.0)
                };
                let panel = egui::Frame::none()
                    .fill(self.panel_fill())
                    .stroke(Stroke::new(1.0, self.panel_border()))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(6.0))
                    .show(ui, |ui| {
                        ui.set_min_size(size);
                        ui.set_max_size(size);
                        if self.minimap_collapsed {
                            ui.centered_and_justified(|ui| {
                                ui.small(format!("{:>3.0}%", self.camera.zoom * 100.0));
                            });
                        } else {
                            ui.horizontal(|ui| {
                                if ui.small_button("-").clicked() {
                                    self.camera.zoom = (self.camera.zoom / 1.15).clamp(0.05, 32.0);
                                }
                                ui.small(format!("{:>3.0}%", self.camera.zoom * 100.0));
                                if ui.small_button("+").clicked() {
                                    self.camera.zoom = (self.camera.zoom * 1.15).clamp(0.05, 32.0);
                                }
                                if ui.small_button("□").on_hover_text("Fit view").clicked() {
                                    self.fit_to_scene();
                                }
                            });

                            let (mini_rect, response) =
                                ui.allocate_exact_size(vec2(102.0, 74.0), Sense::click_and_drag());
                            let painter = ui.painter();
                            painter.rect_filled(
                                mini_rect,
                                4.0,
                                Color32::from_rgba_unmultiplied(120, 130, 140, 22),
                            );

                            let doc_bounds = self
                                .document
                                .bounds()
                                .unwrap_or_else(|| {
                                    WorldRect::from_center(WorldPoint::new(0.0, 0.0), 1.0, 1.0)
                                })
                                .expand(80.0);
                            let sx = mini_rect.width() as f64 / doc_bounds.width().max(1.0);
                            let sy = mini_rect.height() as f64 / doc_bounds.height().max(1.0);
                            let scale = sx.min(sy);
                            let offset_x = mini_rect.left() as f64
                                + (mini_rect.width() as f64 - doc_bounds.width() * scale) * 0.5;
                            let offset_y = mini_rect.top() as f64
                                + (mini_rect.height() as f64 - doc_bounds.height() * scale) * 0.5;

                            let map = |p: WorldPoint| -> Pos2 {
                                pos2(
                                    (offset_x + (p.x - doc_bounds.min.x) * scale) as f32,
                                    (offset_y + (p.y - doc_bounds.min.y) * scale) as f32,
                                )
                            };
                            let unmap = |p: Pos2| -> WorldPoint {
                                WorldPoint::new(
                                    doc_bounds.min.x + (p.x as f64 - offset_x) / scale,
                                    doc_bounds.min.y + (p.y as f64 - offset_y) / scale,
                                )
                            };

                            for shape in &self.document.shapes {
                                let b = shape.bounds();
                                let r = Rect::from_two_pos(map(b.min), map(b.max));
                                painter.rect_stroke(
                                    r,
                                    1.0,
                                    Stroke::new(1.0, Color32::from_gray(120)),
                                );
                            }

                            if (response.hovered() && ctx.input(|i| i.pointer.primary_down()))
                                || response.clicked()
                            {
                                if let Some(pointer_pos) = response.interact_pointer_pos() {
                                    let clamped = pointer_pos.clamp(mini_rect.min, mini_rect.max);
                                    let world = unmap(clamped);
                                    self.camera.center = world;
                                    self.minimap_indicator = Some((world, Instant::now()));
                                }
                            }

                            if let Some((point, _)) = self.minimap_indicator {
                                let marker = map(point);
                                painter.circle_filled(marker, 3.0, Color32::from_rgb(242, 72, 72));
                                painter.circle_stroke(
                                    marker,
                                    4.0,
                                    Stroke::new(
                                        1.0,
                                        Color32::from_rgba_unmultiplied(242, 72, 72, 120),
                                    ),
                                );
                            }
                        }
                    });
                panel.response
            });

        self.minimap_rect = Some(area.inner.rect);
        if self.minimap_collapsed && area.inner.clicked() {
            self.minimap_collapsed = false;
        }
    }

    fn fit_to_scene(&mut self) {
        if let Some(bounds) = self.document.bounds() {
            self.camera.center = bounds.center();
            let target_w = bounds.width().max(1.0) * 1.2;
            let target_h = bounds.height().max(1.0) * 1.2;
            let (view_w, view_h) = if let Some(canvas) = self.last_canvas_rect {
                (canvas.width() as f64, canvas.height() as f64)
            } else {
                (1400.0, 900.0)
            };
            let zoom_x = view_w / target_w;
            let zoom_y = view_h / target_h;
            self.camera.zoom = zoom_x.min(zoom_y).clamp(0.05, 32.0);
        }
    }

    fn draw_grid(&self, painter: &Painter, canvas: Rect) {
        let world_min = self.camera.screen_to_world(canvas.left_top(), canvas);
        let world_max = self.camera.screen_to_world(canvas.right_bottom(), canvas);

        let mut step = self.grid_step;
        while step * self.camera.zoom < 18.0 {
            step *= 2.0;
        }

        let start_x = (world_min.x / step).floor() * step;
        let end_x = (world_max.x / step).ceil() * step;
        let start_y = (world_min.y / step).floor() * step;
        let end_y = (world_max.y / step).ceil() * step;

        let color = self.grid_color();
        match self.grid_view {
            GridView::Lines => {
                let mut x = start_x;
                while x <= end_x {
                    let a = self
                        .camera
                        .world_to_screen(WorldPoint::new(x, world_min.y), canvas);
                    let b = self
                        .camera
                        .world_to_screen(WorldPoint::new(x, world_max.y), canvas);
                    painter.line_segment([a, b], Stroke::new(1.0, color));
                    x += step;
                }

                let mut y = start_y;
                while y <= end_y {
                    let a = self
                        .camera
                        .world_to_screen(WorldPoint::new(world_min.x, y), canvas);
                    let b = self
                        .camera
                        .world_to_screen(WorldPoint::new(world_max.x, y), canvas);
                    painter.line_segment([a, b], Stroke::new(1.0, color));
                    y += step;
                }
            }
            GridView::Dots => {
                let mut x = start_x;
                while x <= end_x {
                    let mut y = start_y;
                    while y <= end_y {
                        let p = self.camera.world_to_screen(WorldPoint::new(x, y), canvas);
                        painter.circle_filled(p, 1.2, color);
                        y += step;
                    }
                    x += step;
                }
            }
            GridView::Off => {}
        }
    }

    fn draw_scene(&self, painter: &Painter, canvas: Rect) {
        for shape in &self.document.shapes {
            self.paint_shape(painter, canvas, shape, self.selection.contains(&shape.id));
        }
    }

    fn draw_selection_overlay(&self, painter: &Painter, canvas: Rect) {
        if self.selection.is_empty() {
            return;
        }
        if let Some(bounds) = self.selection_bounds() {
            let screen_bounds = Rect::from_two_pos(
                self.camera.world_to_screen(bounds.min, canvas),
                self.camera.world_to_screen(bounds.max, canvas),
            );
            painter.rect_filled(
                screen_bounds,
                2.0,
                Color32::from_rgba_unmultiplied(64, 118, 255, 20),
            );
            painter.rect_stroke(
                screen_bounds,
                2.0,
                Stroke::new(1.2, Color32::from_rgb(64, 118, 255)),
            );

            for (_handle, pos) in self.transform_handles(bounds) {
                let p = self.camera.world_to_screen(pos, canvas);
                painter.circle_filled(p, HANDLE_SIZE_PX, Color32::WHITE);
                painter.circle_stroke(
                    p,
                    HANDLE_SIZE_PX,
                    Stroke::new(1.2, Color32::from_rgb(64, 118, 255)),
                );
            }
        }

        if let Some(Interaction::Marquee { start, current }) = self.interaction.as_ref() {
            let rect = Rect::from_two_pos(
                self.camera.world_to_screen(*start, canvas),
                self.camera.world_to_screen(*current, canvas),
            );

            let world_rect = WorldRect::from_points(*start, *current);
            for shape in &self.document.shapes {
                if world_rect.intersects(shape.bounds()) {
                    let b = shape.bounds();
                    let hit_rect = Rect::from_two_pos(
                        self.camera.world_to_screen(b.min, canvas),
                        self.camera.world_to_screen(b.max, canvas),
                    );
                    painter.rect_filled(
                        hit_rect,
                        2.0,
                        Color32::from_rgba_unmultiplied(64, 118, 255, 16),
                    );
                }
            }

            painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(88, 138, 255)));
            painter.rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(88, 138, 255, 28));
        }
    }

    fn draw_draft_overlay(&self, painter: &Painter, canvas: Rect) {
        if let Some(Interaction::Draw {
            tool,
            start,
            current,
            points,
        }) = &self.interaction
        {
            let mut style = self.current_style();
            if matches!(tool, Tool::Highlighter) {
                style.blend = BlendMode::Highlighter;
            }
            let draft = draft_shape_from_points(
                *tool,
                *start,
                *current,
                points.clone(),
                &style,
                self.polygon_sides,
                self.star_points,
                self.star_inner_ratio,
            );
            self.paint_shape(painter, canvas, &draft, false);
        }

        if let Some(Interaction::Laser { points }) = &self.interaction {
            for segment in points.windows(2) {
                let a = self.camera.world_to_screen(segment[0], canvas);
                let b = self.camera.world_to_screen(segment[1], canvas);
                painter.line_segment([a, b], Stroke::new(3.0, Color32::from_rgb(255, 68, 82)));
            }
        }
    }

    fn handle_canvas_input(&mut self, ctx: &Context, response: &Response, canvas: Rect) {
        let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
        let primary_pressed = ctx.input(|i| i.pointer.button_pressed(PointerButton::Primary));
        let primary_down = ctx.input(|i| i.pointer.button_down(PointerButton::Primary));
        let primary_released = ctx.input(|i| i.pointer.button_released(PointerButton::Primary));
        let middle_down = ctx.input(|i| i.pointer.button_down(PointerButton::Middle));
        let wants_pan = self.tool == Tool::Hand || ctx.input(|i| i.key_down(Key::Space));

        if response.hovered() {
            let scroll = ctx.input(|i| i.raw_scroll_delta.y);
            if scroll.abs() > 0.0 {
                if let Some(mouse) = pointer_pos {
                    let before = self.camera.screen_to_world(mouse, canvas);
                    self.camera.zoom =
                        (self.camera.zoom * (1.0 + scroll as f64 * 0.0014)).clamp(0.05, 32.0);
                    let after = self.camera.screen_to_world(mouse, canvas);
                    self.camera.center.x += before.x - after.x;
                    self.camera.center.y += before.y - after.y;
                }
            }
        }

        if self.interaction.is_none() && (wants_pan || middle_down) && primary_pressed {
            if let Some(pos) = pointer_pos {
                self.interaction = Some(Interaction::Pan {
                    start_screen: pos,
                    start_center: self.camera.center,
                });
            }
        }

        if self.interaction.is_none() && primary_pressed {
            if let Some(pos) = pointer_pos {
                if !canvas.contains(pos) {
                    return;
                }
                let world = self.camera.screen_to_world(pos, canvas);
                self.start_interaction(world, pos, ctx, canvas);
            }
        }

        if primary_down {
            if let Some(pos) = pointer_pos {
                let world = self.camera.screen_to_world(pos, canvas);
                self.update_interaction(world, pos, ctx);
            }
        }

        if primary_released {
            if let Some(pos) = pointer_pos {
                let world = self.camera.screen_to_world(pos, canvas);
                self.end_interaction(world, pos, ctx);
            } else {
                self.interaction = None;
            }
        }
    }

    fn start_interaction(&mut self, world: WorldPoint, screen: Pos2, ctx: &Context, canvas: Rect) {
        if self.tool == Tool::Select {
            if let Some(bounds) = self.selection_bounds() {
                if let Some(handle) = self.pick_transform_handle(screen, bounds, canvas) {
                    let seeds = self.selected_shape_clones();
                    if !seeds.is_empty() {
                        self.history.checkpoint(&self.document);
                        if handle == TransformHandle::Rotate {
                            let center = bounds.center();
                            let start_angle = (world.y - center.y).atan2(world.x - center.x);
                            self.interaction = Some(Interaction::Rotate {
                                center,
                                start_angle,
                                seeds,
                            });
                        } else {
                            let (anchor, pivot) = resize_anchor_pivot(bounds, handle);
                            self.interaction = Some(Interaction::Resize {
                                handle,
                                anchor,
                                pivot,
                                seeds,
                            });
                        }
                        return;
                    }
                }
            }

            if let Some(hit) = self.hit_test(world) {
                if !self.selection.contains(&hit) {
                    self.selection.clear();
                    self.selection.insert(hit);
                }

                if self
                    .document
                    .shapes
                    .iter()
                    .find(|shape| shape.id == hit)
                    .is_some_and(|shape| shape.locked)
                {
                    return;
                }

                let seeds = self.selected_shape_clones();
                if !seeds.is_empty() {
                    self.history.checkpoint(&self.document);
                    self.interaction = Some(Interaction::Move {
                        start: world,
                        seeds,
                    });
                }
                return;
            }

            self.selection.clear();
            self.interaction = Some(Interaction::Marquee {
                start: world,
                current: world,
            });
            return;
        }

        if self.tool == Tool::Eraser {
            self.history.checkpoint(&self.document);
            self.erase_at(world);
            self.interaction = Some(Interaction::Erase);
            return;
        }

        if self.tool == Tool::Laser {
            self.interaction = Some(Interaction::Laser {
                points: vec![world],
            });
            return;
        }

        if self.tool == Tool::Text {
            self.history.checkpoint(&self.document);
            let text = self.pending_text.trim();
            let text = if text.is_empty() { "Text" } else { text };
            let style = self.current_style();
            let shape = Shape::new(
                ShapeKind::Text {
                    pos: world,
                    text: text.to_owned(),
                    size: (style.stroke_width * 7.0).clamp(14.0, 88.0),
                },
                style,
            );
            let id = shape.id;
            self.document.shapes.push(shape);
            self.selection.clear();
            self.selection.insert(id);
            self.tool = Tool::Select;
            self.mark_dirty();
            return;
        }

        if self.tool == Tool::Image {
            self.history.checkpoint(&self.document);
            let style = self.current_style();
            let shape = Shape::new(
                ShapeKind::Image {
                    from: world,
                    to: WorldPoint::new(world.x + 280.0, world.y + 180.0),
                    path: self.pending_image_path.clone(),
                },
                style,
            );
            let id = shape.id;
            self.document.shapes.push(shape);
            self.selection.clear();
            self.selection.insert(id);
            self.tool = Tool::Select;
            self.mark_dirty();
            return;
        }

        self.interaction = Some(Interaction::Draw {
            tool: self.tool,
            start: self.snap_world(world, ctx),
            current: self.snap_world(world, ctx),
            points: vec![StrokePoint {
                pos: self.snap_world(world, ctx),
                pressure: self.current_pressure(ctx),
            }],
        });
    }

    fn update_interaction(&mut self, world: WorldPoint, screen: Pos2, ctx: &Context) {
        let snapped_world = self.snap_world(world, ctx);
        let pressure = self.current_pressure(ctx);
        let zoom = self.camera.zoom;
        let should_snap_angle = self.snap_angle && ctx.input(|i| i.modifiers.shift);

        let Some(interaction) = self.interaction.as_mut() else {
            return;
        };

        match interaction {
            Interaction::Pan {
                start_screen,
                start_center,
            } => {
                let dx = (screen.x - start_screen.x) as f64 / self.camera.zoom;
                let dy = (screen.y - start_screen.y) as f64 / self.camera.zoom;
                self.camera.center = WorldPoint::new(start_center.x - dx, start_center.y - dy);
            }
            Interaction::Draw {
                tool,
                start,
                current,
                points,
            } => {
                *current = snapped_world;
                if tool.is_freehand() {
                    let candidate = StrokePoint {
                        pos: world,
                        pressure,
                    };
                    if points
                        .last()
                        .is_none_or(|last| point_distance(last.pos, candidate.pos) > 0.75 / zoom)
                    {
                        points.push(candidate);
                    }
                }
                let _ = start;
            }
            Interaction::Move { start, seeds } => {
                let dx = world.x - start.x;
                let dy = world.y - start.y;
                for seed in seeds {
                    if let Some(shape) = self.document.shapes.iter_mut().find(|s| s.id == seed.id) {
                        *shape = seed.clone();
                        shape.translate(dx, dy);
                    }
                }
                self.mark_dirty();
            }
            Interaction::Marquee { current, .. } => {
                *current = world;
            }
            Interaction::Resize {
                handle,
                anchor,
                pivot,
                seeds,
            } => {
                let (sx, sy) = scale_for_handle(*handle, *anchor, *pivot, world);
                for seed in seeds {
                    if let Some(shape) = self.document.shapes.iter_mut().find(|s| s.id == seed.id) {
                        *shape = seed.clone();
                        shape.scale_from(*anchor, sx, sy);
                    }
                }
                self.mark_dirty();
            }
            Interaction::Rotate {
                center,
                start_angle,
                seeds,
            } => {
                let mut delta = (world.y - center.y).atan2(world.x - center.x) - *start_angle;
                if should_snap_angle {
                    let snap = std::f64::consts::PI / 12.0;
                    delta = (delta / snap).round() * snap;
                }
                for seed in seeds {
                    if let Some(shape) = self.document.shapes.iter_mut().find(|s| s.id == seed.id) {
                        *shape = seed.clone();
                        shape.rotate_from(*center, delta);
                    }
                }
                self.mark_dirty();
            }
            Interaction::Erase => {
                self.erase_at(world);
            }
            Interaction::Laser { points } => {
                if points
                    .last()
                    .is_none_or(|last| point_distance(*last, world) > 1.2 / zoom)
                {
                    points.push(world);
                }
                ctx.request_repaint();
            }
        }
    }

    fn end_interaction(&mut self, world: WorldPoint, _screen: Pos2, ctx: &Context) {
        let Some(interaction) = self.interaction.take() else {
            return;
        };

        match interaction {
            Interaction::Draw {
                tool,
                start,
                current,
                mut points,
            } => {
                let style = self.current_style();
                if tool.is_freehand() {
                    points = simplify_stroke(points, 0.9 / self.camera.zoom);
                }

                let mut end = current;
                if self.snap_angle && ctx.input(|i| i.modifiers.shift) {
                    end = angle_snap(start, current);
                }

                let shape = draft_shape_from_points(
                    tool,
                    start,
                    end,
                    points,
                    &style,
                    self.polygon_sides,
                    self.star_points,
                    self.star_inner_ratio,
                );

                if is_visible_shape(&shape) {
                    self.history.checkpoint(&self.document);
                    let id = shape.id;
                    self.document.shapes.push(shape);
                    if !tool.is_freehand() {
                        self.selection.clear();
                        self.selection.insert(id);
                        self.tool = Tool::Select;
                    }
                    self.mark_dirty();
                }
            }
            Interaction::Marquee { start, current } => {
                let rect = WorldRect::from_points(start, current);
                self.selection.clear();
                for shape in &self.document.shapes {
                    if rect.intersects(shape.bounds()) {
                        self.selection.insert(shape.id);
                    }
                }
            }
            Interaction::Laser { points } => {
                if points.len() > 1 {
                    self.laser_trails.push(LaserTrail {
                        points,
                        born: Instant::now(),
                    });
                }
            }
            Interaction::Move { .. }
            | Interaction::Resize { .. }
            | Interaction::Rotate { .. }
            | Interaction::Erase
            | Interaction::Pan { .. } => {
                let _ = world;
            }
        }
    }

    fn draw_shape_vertices(&self, kind: &ShapeKind) -> Vec<WorldPoint> {
        match kind {
            ShapeKind::Rectangle { from, to } | ShapeKind::Image { from, to, .. } => {
                let rect = WorldRect::from_points(*from, *to);
                rect.corners().to_vec()
            }
            ShapeKind::Diamond { from, to } => {
                let rect = WorldRect::from_points(*from, *to);
                let center = rect.center();
                vec![
                    WorldPoint::new(center.x, rect.min.y),
                    WorldPoint::new(rect.max.x, center.y),
                    WorldPoint::new(center.x, rect.max.y),
                    WorldPoint::new(rect.min.x, center.y),
                ]
            }
            ShapeKind::Triangle { from, to } => {
                let rect = WorldRect::from_points(*from, *to);
                vec![
                    WorldPoint::new((rect.min.x + rect.max.x) * 0.5, rect.min.y),
                    WorldPoint::new(rect.max.x, rect.max.y),
                    WorldPoint::new(rect.min.x, rect.max.y),
                ]
            }
            ShapeKind::Polygon {
                center,
                radius,
                sides,
                rotation,
            } => polygon_vertices(*center, *radius, *sides, *rotation),
            ShapeKind::Star {
                center,
                outer_radius,
                inner_ratio,
                points,
                rotation,
            } => star_vertices(*center, *outer_radius, *inner_ratio, *points, *rotation),
            ShapeKind::Ellipse { from, to } => {
                let rect = WorldRect::from_points(*from, *to);
                ellipse_points(rect, 72)
            }
            _ => Vec::new(),
        }
    }

    fn paint_shape(&self, painter: &Painter, canvas: Rect, shape: &Shape, selected: bool) {
        let mut stroke_color = shape.style.effective_stroke().to_egui();
        let mut fill_color = shape.style.effective_fill().map(|fill| fill.to_egui());
        if matches!(shape.style.blend, BlendMode::Highlighter) {
            stroke_color = Color32::from_rgba_unmultiplied(
                stroke_color.r(),
                stroke_color.g(),
                stroke_color.b(),
                (stroke_color.a() as f32 * 0.45) as u8,
            );
            fill_color = fill_color.map(|fill| {
                Color32::from_rgba_unmultiplied(
                    fill.r(),
                    fill.g(),
                    fill.b(),
                    (fill.a() as f32 * 0.35) as u8,
                )
            });
        }

        let width = (shape.style.stroke_width as f64 * self.camera.zoom).clamp(0.6, 80.0) as f32;
        let stroke = Stroke::new(width, stroke_color);

        match &shape.kind {
            ShapeKind::Freehand { points } => {
                if points.len() < 2 {
                    return;
                }
                for seg in points.windows(2) {
                    let p0 = self.camera.world_to_screen(seg[0].pos, canvas);
                    let p1 = self.camera.world_to_screen(seg[1].pos, canvas);
                    let pressure = (seg[0].pressure + seg[1].pressure) * 0.5;
                    let w = (width * pressure.max(0.1)).clamp(0.4, 96.0);
                    painter.line_segment([p0, p1], Stroke::new(w, stroke_color));
                }
            }
            ShapeKind::Line { start, end } => {
                let a = self.camera.world_to_screen(*start, canvas);
                let b = self.camera.world_to_screen(*end, canvas);
                painter.line_segment([a, b], stroke);
            }
            ShapeKind::Arrow { start, end } => {
                let a = self.camera.world_to_screen(*start, canvas);
                let b = self.camera.world_to_screen(*end, canvas);
                painter.line_segment([a, b], stroke);

                let angle = (b.y - a.y).atan2(b.x - a.x);
                let head = (14.0 * self.camera.zoom as f32).clamp(10.0, 30.0);
                let wing = head * 0.5;
                let p1 = pos2(
                    b.x - head * angle.cos() + wing * (angle + std::f32::consts::FRAC_PI_2).cos(),
                    b.y - head * angle.sin() + wing * (angle + std::f32::consts::FRAC_PI_2).sin(),
                );
                let p2 = pos2(
                    b.x - head * angle.cos() + wing * (angle - std::f32::consts::FRAC_PI_2).cos(),
                    b.y - head * angle.sin() + wing * (angle - std::f32::consts::FRAC_PI_2).sin(),
                );
                painter.add(EguiShape::convex_polygon(
                    vec![b, p1, p2],
                    stroke_color,
                    Stroke::NONE,
                ));
            }
            ShapeKind::Rectangle { from, to } | ShapeKind::Image { from, to, .. } => {
                let rect = Rect::from_two_pos(
                    self.camera.world_to_screen(*from, canvas),
                    self.camera.world_to_screen(*to, canvas),
                );
                if let Some(fill) = fill_color {
                    painter.rect_filled(rect, 4.0, fill);
                }
                painter.rect_stroke(rect, 4.0, stroke);

                if let ShapeKind::Image { path, .. } = &shape.kind {
                    let name = PathBuf::from(path)
                        .file_name()
                        .and_then(|it| it.to_str())
                        .unwrap_or("image")
                        .to_owned();
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        name,
                        egui::FontId::proportional(14.0),
                        Color32::from_gray(60),
                    );
                }
            }
            ShapeKind::Ellipse { from, to } => {
                let points = ellipse_points(WorldRect::from_points(*from, *to), 84)
                    .into_iter()
                    .map(|p| self.camera.world_to_screen(p, canvas))
                    .collect::<Vec<_>>();
                if let Some(fill) = fill_color {
                    painter.add(EguiShape::convex_polygon(
                        points.clone(),
                        fill,
                        Stroke::NONE,
                    ));
                }
                painter.add(EguiShape::closed_line(points, stroke));
            }
            ShapeKind::Diamond { .. }
            | ShapeKind::Triangle { .. }
            | ShapeKind::Polygon { .. }
            | ShapeKind::Star { .. } => {
                let points = self
                    .draw_shape_vertices(&shape.kind)
                    .into_iter()
                    .map(|p| self.camera.world_to_screen(p, canvas))
                    .collect::<Vec<_>>();
                if points.len() >= 3 {
                    if let Some(fill) = fill_color {
                        painter.add(EguiShape::convex_polygon(
                            points.clone(),
                            fill,
                            Stroke::NONE,
                        ));
                    }
                    painter.add(EguiShape::closed_line(points, stroke));
                }
            }
            ShapeKind::Text { pos, text, size } => {
                painter.text(
                    self.camera.world_to_screen(*pos, canvas),
                    Align2::LEFT_TOP,
                    text,
                    egui::FontId::proportional(
                        (*size * self.camera.zoom as f32).clamp(10.0, 140.0),
                    ),
                    stroke_color,
                );
            }
        }

        if selected {
            let b = shape.bounds().expand(3.0 / self.camera.zoom);
            let r = Rect::from_two_pos(
                self.camera.world_to_screen(b.min, canvas),
                self.camera.world_to_screen(b.max, canvas),
            );
            painter.rect_stroke(
                r,
                2.0,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(64, 118, 255, 140)),
            );
        }
    }

    fn hit_test(&self, point: WorldPoint) -> Option<Uuid> {
        let tolerance = 8.0 / self.camera.zoom;
        self.document
            .shapes
            .iter()
            .rev()
            .find(|shape| shape.hit_test(point, tolerance))
            .map(|shape| shape.id)
    }

    fn erase_at(&mut self, point: WorldPoint) {
        let radius = 14.0 / self.camera.zoom;
        let before = self.document.shapes.len();
        self.document
            .shapes
            .retain(|shape| !shape.hit_test(point, radius));
        if self.document.shapes.len() != before {
            self.mark_dirty();
            self.selection.clear();
        }
    }

    fn selected_shape_clones(&self) -> Vec<Shape> {
        self.document
            .shapes
            .iter()
            .filter(|shape| self.selection.contains(&shape.id) && !shape.locked)
            .cloned()
            .collect()
    }

    fn selection_bounds(&self) -> Option<WorldRect> {
        let mut iter = self
            .document
            .shapes
            .iter()
            .filter(|shape| self.selection.contains(&shape.id));
        let first = iter.next()?;
        let mut bounds = first.bounds();
        for shape in iter {
            bounds = bounds.union(shape.bounds());
        }
        Some(bounds)
    }

    fn transform_handles(&self, bounds: WorldRect) -> Vec<(TransformHandle, WorldPoint)> {
        let c = bounds.center();
        let top = WorldPoint::new(c.x, bounds.min.y);
        let handle_offset = 32.0 / self.camera.zoom;
        vec![
            (TransformHandle::TopLeft, bounds.min),
            (TransformHandle::Top, top),
            (
                TransformHandle::TopRight,
                WorldPoint::new(bounds.max.x, bounds.min.y),
            ),
            (TransformHandle::Right, WorldPoint::new(bounds.max.x, c.y)),
            (TransformHandle::BottomRight, bounds.max),
            (TransformHandle::Bottom, WorldPoint::new(c.x, bounds.max.y)),
            (
                TransformHandle::BottomLeft,
                WorldPoint::new(bounds.min.x, bounds.max.y),
            ),
            (TransformHandle::Left, WorldPoint::new(bounds.min.x, c.y)),
            (
                TransformHandle::Rotate,
                WorldPoint::new(top.x, top.y - handle_offset),
            ),
        ]
    }

    fn pick_transform_handle(
        &self,
        screen: Pos2,
        bounds: WorldRect,
        canvas: Rect,
    ) -> Option<TransformHandle> {
        let radius = HANDLE_SIZE_PX + 2.0;
        self.transform_handles(bounds)
            .into_iter()
            .find(|(_, world)| {
                let pos = self.camera.world_to_screen(*world, canvas);
                pos.distance(screen) <= radius
            })
            .map(|(handle, _)| handle)
    }

    fn current_pressure(&self, ctx: &Context) -> f32 {
        if !self.pressure_enabled {
            return 1.0;
        }
        let _ = ctx;
        self.last_pressure
    }

    fn refresh_tablet_pressure(&mut self, ctx: &Context) {
        if !self.pressure_enabled {
            return;
        }
        let mut observed = None;
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Touch { force, .. } = event {
                    observed = Some(force.unwrap_or(1.0));
                }
            }
        });
        if let Some(force) = observed {
            self.last_pressure = force.clamp(0.1, 1.6);
        }
    }

    fn snap_world(&self, mut point: WorldPoint, ctx: &Context) -> WorldPoint {
        if !self.snap_grid || ctx.input(|i| i.modifiers.alt) {
            return point;
        }
        let step = self.grid_step;
        point.x = (point.x / step).round() * step;
        point.y = (point.y / step).round() * step;
        point
    }
}

fn paint_tool_icon(painter: &Painter, rect: Rect, tool: Tool, color: Color32) {
    let icon = match tool {
        Tool::Select => ph::CURSOR,
        Tool::Hand => ph::HAND_GRABBING,
        Tool::Laser => ph::LIGHTNING,
        Tool::Pen => ph::PENCIL_SIMPLE,
        Tool::Pencil => ph::PENCIL,
        Tool::Highlighter => ph::HIGHLIGHTER_CIRCLE,
        Tool::Eraser => ph::ERASER,
        Tool::Line => ph::LINE_SEGMENT,
        Tool::Arrow => ph::ARROW_UP_RIGHT,
        Tool::Rectangle => ph::RECTANGLE,
        Tool::Ellipse => ph::CIRCLE,
        Tool::Diamond => ph::DIAMOND,
        Tool::Triangle => ph::TRIANGLE,
        Tool::Polygon => ph::POLYGON,
        Tool::Star => ph::STAR,
        Tool::Text => ph::TEXT_T,
        Tool::Image => ph::IMAGE,
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(17.0),
        color,
    );
}

fn default_style_for_tool(tool: Tool) -> Style {
    match tool {
        Tool::Laser => Style {
            stroke: Rgba::rgb(255, 57, 73),
            fill: None,
            stroke_width: 3.0,
            opacity: 0.9,
            dashed: false,
            blend: BlendMode::Normal,
        },
        Tool::Pen => Style {
            stroke: Rgba::rgb(230, 232, 237),
            fill: None,
            stroke_width: 2.8,
            opacity: 1.0,
            dashed: false,
            blend: BlendMode::Normal,
        },
        Tool::Pencil => Style {
            stroke: Rgba::rgb(175, 185, 205),
            fill: None,
            stroke_width: 2.2,
            opacity: 0.86,
            dashed: false,
            blend: BlendMode::Normal,
        },
        Tool::Highlighter => Style {
            stroke: Rgba::rgb(255, 231, 76),
            fill: None,
            stroke_width: 13.0,
            opacity: 0.55,
            dashed: false,
            blend: BlendMode::Highlighter,
        },
        Tool::Eraser => Style {
            stroke: Rgba::rgb(239, 68, 68),
            fill: None,
            stroke_width: 24.0,
            opacity: 1.0,
            dashed: false,
            blend: BlendMode::Normal,
        },
        Tool::Text => Style {
            stroke: Rgba::rgb(232, 236, 245),
            fill: None,
            stroke_width: 2.0,
            opacity: 1.0,
            dashed: false,
            blend: BlendMode::Normal,
        },
        _ => Style {
            stroke: Rgba::rgb(219, 224, 236),
            fill: Some(Rgba::rgb(37, 43, 59)),
            stroke_width: 2.0,
            opacity: 1.0,
            dashed: false,
            blend: BlendMode::Normal,
        },
    }
}

fn draw_palette_grid(ui: &mut egui::Ui, style: &mut Style, edit_fill: bool) {
    const LEFT_COLORS: [Rgba; 8] = [
        Rgba::rgb(9, 16, 38),
        Rgba::rgb(129, 31, 31),
        Rgba::rgb(161, 74, 11),
        Rgba::rgb(164, 102, 8),
        Rgba::rgb(20, 93, 55),
        Rgba::rgb(23, 97, 168),
        Rgba::rgb(64, 63, 154),
        Rgba::rgb(95, 36, 132),
    ];
    const RIGHT_COLORS: [Rgba; 8] = [
        Rgba::rgb(17, 24, 39),
        Rgba::rgb(220, 38, 38),
        Rgba::rgb(249, 115, 22),
        Rgba::rgb(250, 204, 21),
        Rgba::rgb(34, 197, 94),
        Rgba::rgb(59, 130, 246),
        Rgba::rgb(99, 102, 241),
        Rgba::rgb(168, 85, 247),
    ];

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 3.0);
        for column in [LEFT_COLORS, RIGHT_COLORS] {
            ui.vertical(|ui| {
                for color in column {
                    let (rect, response) = ui.allocate_exact_size(vec2(16.0, 14.0), Sense::click());
                    ui.painter().rect_filled(rect, 3.0, color.to_egui());
                    ui.painter()
                        .rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_gray(120)));
                    if response.clicked() {
                        apply_color(style, edit_fill, color);
                    }
                }
            });
        }
    });
}

fn apply_color(style: &mut Style, edit_fill: bool, color: Rgba) {
    if edit_fill {
        style.fill = Some(color);
    } else {
        style.stroke = color;
    }
}

fn draft_shape_from_points(
    tool: Tool,
    start: WorldPoint,
    current: WorldPoint,
    points: Vec<StrokePoint>,
    style: &Style,
    polygon_sides: u8,
    star_points: u8,
    star_inner_ratio: f64,
) -> Shape {
    let kind = match tool {
        Tool::Pen | Tool::Pencil | Tool::Highlighter => ShapeKind::Freehand { points },
        Tool::Line => ShapeKind::Line {
            start,
            end: current,
        },
        Tool::Arrow => ShapeKind::Arrow {
            start,
            end: current,
        },
        Tool::Rectangle => ShapeKind::Rectangle {
            from: start,
            to: current,
        },
        Tool::Ellipse => ShapeKind::Ellipse {
            from: start,
            to: current,
        },
        Tool::Diamond => ShapeKind::Diamond {
            from: start,
            to: current,
        },
        Tool::Triangle => ShapeKind::Triangle {
            from: start,
            to: current,
        },
        Tool::Polygon => {
            let radius = point_distance(start, current);
            ShapeKind::Polygon {
                center: start,
                radius,
                sides: polygon_sides.max(3),
                rotation: 0.0,
            }
        }
        Tool::Star => {
            let outer_radius = point_distance(start, current);
            ShapeKind::Star {
                center: start,
                outer_radius,
                inner_ratio: star_inner_ratio,
                points: star_points.max(3),
                rotation: 0.0,
            }
        }
        Tool::Image => ShapeKind::Image {
            from: start,
            to: current,
            path: String::new(),
        },
        Tool::Laser | Tool::Text | Tool::Eraser | Tool::Select | Tool::Hand => ShapeKind::Line {
            start,
            end: current,
        },
    };
    Shape::new(kind, style.clone())
}

fn is_visible_shape(shape: &Shape) -> bool {
    let b = shape.bounds();
    b.width().abs() > 1.0 || b.height().abs() > 1.0
}

fn angle_snap(start: WorldPoint, end: WorldPoint) -> WorldPoint {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= f64::EPSILON {
        return end;
    }
    let angle = dy.atan2(dx);
    let step = std::f64::consts::PI / 12.0;
    let snapped = (angle / step).round() * step;
    WorldPoint::new(
        start.x + distance * snapped.cos(),
        start.y + distance * snapped.sin(),
    )
}

fn point_distance(a: WorldPoint, b: WorldPoint) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn simplify_stroke(points: Vec<StrokePoint>, tolerance: f64) -> Vec<StrokePoint> {
    if points.len() <= 2 {
        return points;
    }
    let keep = rdp_indices(&points, tolerance);
    keep.into_iter().map(|idx| points[idx].clone()).collect()
}

fn rdp_indices(points: &[StrokePoint], epsilon: f64) -> Vec<usize> {
    fn recurse(points: &[StrokePoint], first: usize, last: usize, eps: f64, keep: &mut Vec<usize>) {
        if last <= first + 1 {
            return;
        }
        let a = points[first].pos;
        let b = points[last].pos;
        let mut max_dist = 0.0;
        let mut index = first;
        for i in (first + 1)..last {
            let p = points[i].pos;
            let dist = distance_to_segment(p, a, b);
            if dist > max_dist {
                max_dist = dist;
                index = i;
            }
        }
        if max_dist > eps {
            keep.push(index);
            recurse(points, first, index, eps, keep);
            recurse(points, index, last, eps, keep);
        }
    }

    use crate::model::distance_to_segment;

    let mut keep = vec![0, points.len() - 1];
    recurse(points, 0, points.len() - 1, epsilon, &mut keep);
    keep.sort_unstable();
    keep.dedup();
    keep
}

fn ellipse_points(rect: WorldRect, segments: usize) -> Vec<WorldPoint> {
    let center = rect.center();
    let rx = rect.width() * 0.5;
    let ry = rect.height() * 0.5;
    let count = segments.max(16);
    (0..count)
        .map(|idx| {
            let t = idx as f64 / count as f64;
            let angle = t * std::f64::consts::TAU;
            WorldPoint::new(center.x + rx * angle.cos(), center.y + ry * angle.sin())
        })
        .collect()
}

fn resize_anchor_pivot(bounds: WorldRect, handle: TransformHandle) -> (WorldPoint, WorldPoint) {
    match handle {
        TransformHandle::TopLeft => (bounds.max, bounds.min),
        TransformHandle::Top => (
            WorldPoint::new(bounds.center().x, bounds.max.y),
            WorldPoint::new(bounds.center().x, bounds.min.y),
        ),
        TransformHandle::TopRight => (
            WorldPoint::new(bounds.min.x, bounds.max.y),
            WorldPoint::new(bounds.max.x, bounds.min.y),
        ),
        TransformHandle::Right => (
            WorldPoint::new(bounds.min.x, bounds.center().y),
            WorldPoint::new(bounds.max.x, bounds.center().y),
        ),
        TransformHandle::BottomRight => (bounds.min, bounds.max),
        TransformHandle::Bottom => (
            WorldPoint::new(bounds.center().x, bounds.min.y),
            WorldPoint::new(bounds.center().x, bounds.max.y),
        ),
        TransformHandle::BottomLeft => (
            WorldPoint::new(bounds.max.x, bounds.min.y),
            WorldPoint::new(bounds.min.x, bounds.max.y),
        ),
        TransformHandle::Left => (
            WorldPoint::new(bounds.max.x, bounds.center().y),
            WorldPoint::new(bounds.min.x, bounds.center().y),
        ),
        TransformHandle::Rotate => (bounds.center(), bounds.center()),
    }
}

fn scale_for_handle(
    handle: TransformHandle,
    anchor: WorldPoint,
    pivot: WorldPoint,
    current: WorldPoint,
) -> (f64, f64) {
    let dx0 = pivot.x - anchor.x;
    let dy0 = pivot.y - anchor.y;
    let dx1 = current.x - anchor.x;
    let dy1 = current.y - anchor.y;

    let sx = if dx0.abs() <= f64::EPSILON {
        1.0
    } else {
        dx1 / dx0
    };
    let sy = if dy0.abs() <= f64::EPSILON {
        1.0
    } else {
        dy1 / dy0
    };

    match handle {
        TransformHandle::Top | TransformHandle::Bottom => (1.0, sy),
        TransformHandle::Left | TransformHandle::Right => (sx, 1.0),
        TransformHandle::Rotate => (1.0, 1.0),
        _ => (sx, sy),
    }
}
