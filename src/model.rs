use egui::{pos2, Color32, Pos2};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

impl WorldPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WorldRect {
    pub min: WorldPoint,
    pub max: WorldPoint,
}

impl WorldRect {
    pub fn from_points(a: WorldPoint, b: WorldPoint) -> Self {
        Self {
            min: WorldPoint::new(a.x.min(b.x), a.y.min(b.y)),
            max: WorldPoint::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn from_center(center: WorldPoint, half_w: f64, half_h: f64) -> Self {
        Self {
            min: WorldPoint::new(center.x - half_w, center.y - half_h),
            max: WorldPoint::new(center.x + half_w, center.y + half_h),
        }
    }

    pub fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f64 {
        self.max.y - self.min.y
    }

    pub fn center(self) -> WorldPoint {
        WorldPoint::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    pub fn expand(self, amount: f64) -> Self {
        Self {
            min: WorldPoint::new(self.min.x - amount, self.min.y - amount),
            max: WorldPoint::new(self.max.x + amount, self.max.y + amount),
        }
    }

    pub fn contains(self, point: WorldPoint) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: WorldPoint::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: WorldPoint::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    pub fn corners(self) -> [WorldPoint; 4] {
        [
            self.min,
            WorldPoint::new(self.max.x, self.min.y),
            self.max,
            WorldPoint::new(self.min.x, self.max.y),
        ]
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn with_alpha(self, alpha: f32) -> Self {
        let mut color = self;
        let clamped = alpha.clamp(0.0, 1.0);
        color.a = (clamped * 255.0).round() as u8;
        color
    }

    pub fn to_egui(self) -> Color32 {
        Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }
}

impl Default for Rgba {
    fn default() -> Self {
        Self::rgb(33, 37, 41)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Highlighter,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Style {
    pub stroke: Rgba,
    pub fill: Option<Rgba>,
    pub stroke_width: f32,
    pub opacity: f32,
    pub dashed: bool,
    pub blend: BlendMode,
}

impl Style {
    pub fn effective_stroke(&self) -> Rgba {
        self.stroke.with_alpha(self.opacity)
    }

    pub fn effective_fill(&self) -> Option<Rgba> {
        self.fill.map(|fill| fill.with_alpha(self.opacity))
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            stroke: Rgba::rgb(33, 37, 41),
            fill: None,
            stroke_width: 2.0,
            opacity: 1.0,
            dashed: false,
            blend: BlendMode::Normal,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrokePoint {
    pub pos: WorldPoint,
    pub pressure: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ShapeKind {
    Freehand {
        points: Vec<StrokePoint>,
    },
    Line {
        start: WorldPoint,
        end: WorldPoint,
    },
    Arrow {
        start: WorldPoint,
        end: WorldPoint,
    },
    Rectangle {
        from: WorldPoint,
        to: WorldPoint,
    },
    Ellipse {
        from: WorldPoint,
        to: WorldPoint,
    },
    Diamond {
        from: WorldPoint,
        to: WorldPoint,
    },
    Triangle {
        from: WorldPoint,
        to: WorldPoint,
    },
    Polygon {
        center: WorldPoint,
        radius: f64,
        sides: u8,
        rotation: f64,
    },
    Star {
        center: WorldPoint,
        outer_radius: f64,
        inner_ratio: f64,
        points: u8,
        rotation: f64,
    },
    Text {
        pos: WorldPoint,
        text: String,
        size: f32,
    },
    Image {
        from: WorldPoint,
        to: WorldPoint,
        path: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shape {
    pub id: Uuid,
    pub kind: ShapeKind,
    pub style: Style,
    pub locked: bool,
}

impl Shape {
    pub fn new(kind: ShapeKind, style: Style) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            style,
            locked: false,
        }
    }

    pub fn bounds(&self) -> WorldRect {
        match &self.kind {
            ShapeKind::Freehand { points } => {
                if points.is_empty() {
                    return WorldRect::from_center(WorldPoint::new(0.0, 0.0), 0.0, 0.0);
                }

                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;

                for point in points {
                    min_x = min_x.min(point.pos.x);
                    min_y = min_y.min(point.pos.y);
                    max_x = max_x.max(point.pos.x);
                    max_y = max_y.max(point.pos.y);
                }

                WorldRect {
                    min: WorldPoint::new(min_x, min_y),
                    max: WorldPoint::new(max_x, max_y),
                }
            }
            ShapeKind::Line { start, end }
            | ShapeKind::Arrow { start, end }
            | ShapeKind::Rectangle {
                from: start,
                to: end,
            }
            | ShapeKind::Ellipse {
                from: start,
                to: end,
            }
            | ShapeKind::Diamond {
                from: start,
                to: end,
            }
            | ShapeKind::Triangle {
                from: start,
                to: end,
            }
            | ShapeKind::Image {
                from: start,
                to: end,
                ..
            } => WorldRect::from_points(*start, *end),
            ShapeKind::Polygon { center, radius, .. } => {
                WorldRect::from_center(*center, *radius, *radius)
            }
            ShapeKind::Star {
                center,
                outer_radius,
                ..
            } => WorldRect::from_center(*center, *outer_radius, *outer_radius),
            ShapeKind::Text { pos, text, size } => {
                let width = text.chars().count() as f64 * (*size as f64 * 0.58);
                let height = *size as f64 * 1.2;
                WorldRect {
                    min: *pos,
                    max: WorldPoint::new(pos.x + width.max(1.0), pos.y + height.max(1.0)),
                }
            }
        }
    }

    pub fn hit_test(&self, point: WorldPoint, tolerance: f64) -> bool {
        if !self.bounds().expand(tolerance).contains(point) {
            return false;
        }

        match &self.kind {
            ShapeKind::Freehand { points } => {
                if points.len() < 2 {
                    return false;
                }

                points
                    .windows(2)
                    .any(|seg| distance_to_segment(point, seg[0].pos, seg[1].pos) <= tolerance)
            }
            ShapeKind::Line { start, end } | ShapeKind::Arrow { start, end } => {
                distance_to_segment(point, *start, *end) <= tolerance
            }
            ShapeKind::Rectangle { from, to } | ShapeKind::Image { from, to, .. } => {
                let rect = WorldRect::from_points(*from, *to);
                if self.style.fill.is_some() {
                    rect.contains(point)
                } else {
                    let points = rect.corners();
                    polygon_distance(point, &points) <= tolerance
                }
            }
            ShapeKind::Ellipse { from, to } => {
                let rect = WorldRect::from_points(*from, *to);
                let center = rect.center();
                let rx = rect.width() * 0.5;
                let ry = rect.height() * 0.5;
                if rx <= f64::EPSILON || ry <= f64::EPSILON {
                    return false;
                }
                let nx = (point.x - center.x) / rx;
                let ny = (point.y - center.y) / ry;
                let d = nx * nx + ny * ny;
                if self.style.fill.is_some() {
                    d <= 1.0
                } else {
                    (d - 1.0).abs() <= tolerance / rx.max(ry)
                }
            }
            ShapeKind::Diamond { from, to } => {
                let rect = WorldRect::from_points(*from, *to);
                let center = rect.center();
                let points = [
                    WorldPoint::new(center.x, rect.min.y),
                    WorldPoint::new(rect.max.x, center.y),
                    WorldPoint::new(center.x, rect.max.y),
                    WorldPoint::new(rect.min.x, center.y),
                ];
                if self.style.fill.is_some() {
                    point_in_polygon(point, &points)
                } else {
                    polygon_distance(point, &points) <= tolerance
                }
            }
            ShapeKind::Triangle { from, to } => {
                let rect = WorldRect::from_points(*from, *to);
                let points = [
                    WorldPoint::new((rect.min.x + rect.max.x) * 0.5, rect.min.y),
                    WorldPoint::new(rect.max.x, rect.max.y),
                    WorldPoint::new(rect.min.x, rect.max.y),
                ];
                if self.style.fill.is_some() {
                    point_in_polygon(point, &points)
                } else {
                    polygon_distance(point, &points) <= tolerance
                }
            }
            ShapeKind::Polygon {
                center,
                radius,
                sides,
                rotation,
            } => {
                let points = polygon_vertices(*center, *radius, (*sides).max(3), *rotation);
                if self.style.fill.is_some() {
                    point_in_polygon(point, &points)
                } else {
                    polygon_distance(point, &points) <= tolerance
                }
            }
            ShapeKind::Star {
                center,
                outer_radius,
                inner_ratio,
                points,
                rotation,
            } => {
                let points = star_vertices(
                    *center,
                    *outer_radius,
                    *inner_ratio,
                    (*points).max(3),
                    *rotation,
                );
                if self.style.fill.is_some() {
                    point_in_polygon(point, &points)
                } else {
                    polygon_distance(point, &points) <= tolerance
                }
            }
            ShapeKind::Text { .. } => self.bounds().contains(point),
        }
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        match &mut self.kind {
            ShapeKind::Freehand { points } => {
                for point in points {
                    point.pos.x += dx;
                    point.pos.y += dy;
                }
            }
            ShapeKind::Line { start, end }
            | ShapeKind::Arrow { start, end }
            | ShapeKind::Rectangle {
                from: start,
                to: end,
            }
            | ShapeKind::Ellipse {
                from: start,
                to: end,
            }
            | ShapeKind::Diamond {
                from: start,
                to: end,
            }
            | ShapeKind::Triangle {
                from: start,
                to: end,
            }
            | ShapeKind::Image {
                from: start,
                to: end,
                ..
            } => {
                start.x += dx;
                start.y += dy;
                end.x += dx;
                end.y += dy;
            }
            ShapeKind::Polygon { center, .. } | ShapeKind::Star { center, .. } => {
                center.x += dx;
                center.y += dy;
            }
            ShapeKind::Text { pos, .. } => {
                pos.x += dx;
                pos.y += dy;
            }
        }
    }

    pub fn scale_from(&mut self, center: WorldPoint, sx: f64, sy: f64) {
        match &mut self.kind {
            ShapeKind::Freehand { points } => {
                for point in points {
                    point.pos = scale_point(point.pos, center, sx, sy);
                }
            }
            ShapeKind::Line { start, end }
            | ShapeKind::Arrow { start, end }
            | ShapeKind::Rectangle {
                from: start,
                to: end,
            }
            | ShapeKind::Ellipse {
                from: start,
                to: end,
            }
            | ShapeKind::Diamond {
                from: start,
                to: end,
            }
            | ShapeKind::Triangle {
                from: start,
                to: end,
            }
            | ShapeKind::Image {
                from: start,
                to: end,
                ..
            } => {
                *start = scale_point(*start, center, sx, sy);
                *end = scale_point(*end, center, sx, sy);
            }
            ShapeKind::Polygon {
                center: c, radius, ..
            } => {
                *c = scale_point(*c, center, sx, sy);
                *radius *= sx.abs().max(sy.abs());
            }
            ShapeKind::Star {
                center: c,
                outer_radius,
                ..
            } => {
                *c = scale_point(*c, center, sx, sy);
                *outer_radius *= sx.abs().max(sy.abs());
            }
            ShapeKind::Text { pos, size, .. } => {
                *pos = scale_point(*pos, center, sx, sy);
                let scale = ((sx.abs() + sy.abs()) * 0.5) as f32;
                *size = (*size * scale).clamp(6.0, 220.0);
            }
        }
    }

    pub fn rotate_from(&mut self, center: WorldPoint, radians: f64) {
        match &mut self.kind {
            ShapeKind::Freehand { points } => {
                for point in points {
                    point.pos = rotate_point(point.pos, center, radians);
                }
            }
            ShapeKind::Line { start, end }
            | ShapeKind::Arrow { start, end }
            | ShapeKind::Rectangle {
                from: start,
                to: end,
            }
            | ShapeKind::Ellipse {
                from: start,
                to: end,
            }
            | ShapeKind::Diamond {
                from: start,
                to: end,
            }
            | ShapeKind::Triangle {
                from: start,
                to: end,
            }
            | ShapeKind::Image {
                from: start,
                to: end,
                ..
            } => {
                *start = rotate_point(*start, center, radians);
                *end = rotate_point(*end, center, radians);
            }
            ShapeKind::Polygon {
                center: c,
                rotation,
                ..
            }
            | ShapeKind::Star {
                center: c,
                rotation,
                ..
            } => {
                *c = rotate_point(*c, center, radians);
                *rotation += radians;
            }
            ShapeKind::Text { pos, .. } => {
                *pos = rotate_point(*pos, center, radians);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub shapes: Vec<Shape>,
    pub show_grid: bool,
    pub background: Rgba,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            shapes: Vec::new(),
            show_grid: true,
            background: Rgba::rgb(0, 0, 0),
        }
    }
}

impl Document {
    pub fn bounds(&self) -> Option<WorldRect> {
        let mut iter = self.shapes.iter();
        let first = iter.next()?;
        let mut acc = first.bounds();
        for shape in iter {
            acc = acc.union(shape.bounds());
        }
        Some(acc)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub center: WorldPoint,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: WorldPoint::new(0.0, 0.0),
            zoom: 1.0,
        }
    }
}

impl Camera {
    pub fn world_to_screen(&self, world: WorldPoint, canvas: egui::Rect) -> Pos2 {
        let cx = canvas.center().x as f64;
        let cy = canvas.center().y as f64;
        pos2(
            (cx + (world.x - self.center.x) * self.zoom) as f32,
            (cy + (world.y - self.center.y) * self.zoom) as f32,
        )
    }

    pub fn screen_to_world(&self, screen: Pos2, canvas: egui::Rect) -> WorldPoint {
        let cx = canvas.center().x as f64;
        let cy = canvas.center().y as f64;
        WorldPoint::new(
            (screen.x as f64 - cx) / self.zoom + self.center.x,
            (screen.y as f64 - cy) / self.zoom + self.center.y,
        )
    }
}

pub fn rotate_point(point: WorldPoint, center: WorldPoint, radians: f64) -> WorldPoint {
    let cos = radians.cos();
    let sin = radians.sin();
    let x = point.x - center.x;
    let y = point.y - center.y;
    WorldPoint::new(center.x + x * cos - y * sin, center.y + x * sin + y * cos)
}

pub fn scale_point(point: WorldPoint, center: WorldPoint, sx: f64, sy: f64) -> WorldPoint {
    WorldPoint::new(
        center.x + (point.x - center.x) * sx,
        center.y + (point.y - center.y) * sy,
    )
}

pub fn distance_to_segment(point: WorldPoint, a: WorldPoint, b: WorldPoint) -> f64 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let len_sq = ab_x * ab_x + ab_y * ab_y;
    if len_sq <= f64::EPSILON {
        return ((point.x - a.x).powi(2) + (point.y - a.y).powi(2)).sqrt();
    }
    let t = (((point.x - a.x) * ab_x + (point.y - a.y) * ab_y) / len_sq).clamp(0.0, 1.0);
    let proj_x = a.x + t * ab_x;
    let proj_y = a.y + t * ab_y;
    ((point.x - proj_x).powi(2) + (point.y - proj_y).powi(2)).sqrt()
}

pub fn polygon_distance(point: WorldPoint, points: &[WorldPoint]) -> f64 {
    if points.len() < 2 {
        return f64::MAX;
    }

    let mut min_d = f64::MAX;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        min_d = min_d.min(distance_to_segment(point, a, b));
    }
    min_d
}

pub fn point_in_polygon(point: WorldPoint, points: &[WorldPoint]) -> bool {
    if points.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let xi = points[i].x;
        let yi = points[i].y;
        let xj = points[j].x;
        let yj = points[j].y;

        let intersect = ((yi > point.y) != (yj > point.y))
            && (point.x < (xj - xi) * (point.y - yi) / (yj - yi + f64::EPSILON) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn polygon_vertices(
    center: WorldPoint,
    radius: f64,
    sides: u8,
    rotation: f64,
) -> Vec<WorldPoint> {
    let sides = sides.max(3) as usize;
    let step = std::f64::consts::TAU / sides as f64;
    (0..sides)
        .map(|idx| {
            let angle = rotation + step * idx as f64 - std::f64::consts::FRAC_PI_2;
            WorldPoint::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

pub fn star_vertices(
    center: WorldPoint,
    outer_radius: f64,
    inner_ratio: f64,
    points: u8,
    rotation: f64,
) -> Vec<WorldPoint> {
    let points = points.max(3) as usize;
    let step = std::f64::consts::TAU / points as f64;
    let inner_radius = outer_radius * inner_ratio.clamp(0.1, 0.95);

    let mut vertices = Vec::with_capacity(points * 2);
    for idx in 0..points {
        let outer_angle = rotation + idx as f64 * step - std::f64::consts::FRAC_PI_2;
        let inner_angle = outer_angle + step * 0.5;

        vertices.push(WorldPoint::new(
            center.x + outer_radius * outer_angle.cos(),
            center.y + outer_radius * outer_angle.sin(),
        ));
        vertices.push(WorldPoint::new(
            center.x + inner_radius * inner_angle.cos(),
            center.y + inner_radius * inner_angle.sin(),
        ));
    }

    vertices
}
