use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::model::{
    polygon_vertices, star_vertices, Camera, Document, Shape, ShapeKind, WorldPoint, WorldRect,
};

const PROJECT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u32,
    pub document: Document,
    pub camera: Camera,
}

impl ProjectFile {
    pub fn new(document: Document, camera: Camera) -> Self {
        Self {
            version: PROJECT_VERSION,
            document,
            camera,
        }
    }
}

pub fn save_project(path: &Path, file: &ProjectFile) -> Result<()> {
    let data = serde_json::to_vec_pretty(file).context("failed to serialize project")?;
    fs::write(path, data).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn load_project(path: &Path) -> Result<ProjectFile> {
    let data = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let file: ProjectFile =
        serde_json::from_slice(&data).context("failed to deserialize project file")?;
    Ok(file)
}

pub fn autosave_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("io", "ldraw", "ldraw")
        .ok_or_else(|| anyhow!("unable to locate project dirs"))?;
    let state_dir = dirs
        .state_dir()
        .or_else(|| Some(dirs.data_local_dir()))
        .ok_or_else(|| anyhow!("unable to locate autosave directory"))?;
    fs::create_dir_all(state_dir).context("failed to create autosave dir")?;
    Ok(state_dir.join("autosave.ldrw"))
}

pub fn write_autosave(file: &ProjectFile) -> Result<PathBuf> {
    let path = autosave_path()?;
    save_project(&path, file)?;
    Ok(path)
}

pub fn read_autosave() -> Option<ProjectFile> {
    let path = autosave_path().ok()?;
    if !path.exists() {
        return None;
    }
    load_project(&path).ok()
}

pub fn export_svg(path: &Path, document: &Document) -> Result<()> {
    let svg = document_to_svg(document);
    fs::write(path, svg).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn export_png(path: &Path, document: &Document) -> Result<()> {
    let bounds = document
        .bounds()
        .map(|b| b.expand(160.0))
        .unwrap_or_else(|| WorldRect::from_center(WorldPoint::new(0.0, 0.0), 640.0, 480.0));
    let world_w = bounds.width().max(1.0);
    let world_h = bounds.height().max(1.0);
    let max_side = 2400.0;
    let scale = (max_side / world_w.max(world_h)).clamp(0.3, 3.0);
    let width = (world_w * scale).ceil() as u32;
    let height = (world_h * scale).ceil() as u32;

    let mut pixmap = Pixmap::new(width, height).ok_or_else(|| anyhow!("failed to allocate png"))?;
    pixmap.fill(tiny_skia::Color::from_rgba8(
        document.background.r,
        document.background.g,
        document.background.b,
        document.background.a,
    ));

    for shape in &document.shapes {
        draw_shape_to_pixmap(&mut pixmap, shape, bounds, scale as f32);
    }

    pixmap
        .save_png(path)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn document_to_svg(document: &Document) -> String {
    let bounds = document
        .bounds()
        .map(|b| b.expand(160.0))
        .unwrap_or_else(|| WorldRect::from_center(WorldPoint::new(0.0, 0.0), 640.0, 480.0));
    let width = bounds.width().max(1.0);
    let height = bounds.height().max(1.0);

    let mut body = String::new();
    body.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
        bounds.min.x,
        bounds.min.y,
        width,
        height,
        css_color(document.background)
    ));

    for shape in &document.shapes {
        body.push_str(&shape_to_svg(shape));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}" height="{}">
{}
</svg>
"#,
        bounds.min.x, bounds.min.y, width, height, width, height, body
    )
}

fn shape_to_svg(shape: &Shape) -> String {
    let stroke_color = css_color(shape.style.effective_stroke());
    let fill_color = shape
        .style
        .effective_fill()
        .map(css_color)
        .unwrap_or_else(|| "none".to_string());
    let dash = if shape.style.dashed { "8 6" } else { "" };
    let opacity = shape.style.opacity.clamp(0.0, 1.0);
    let common = format!(
        r#"stroke="{}" fill="{}" stroke-width="{}" opacity="{}" stroke-linecap="round" stroke-linejoin="round" {}"#,
        stroke_color,
        fill_color,
        shape.style.stroke_width,
        opacity,
        if dash.is_empty() {
            String::new()
        } else {
            format!(r#"stroke-dasharray="{}""#, dash)
        }
    );

    match &shape.kind {
        ShapeKind::Freehand { points } => {
            if points.len() < 2 {
                return String::new();
            }
            let mut d = String::new();
            d.push_str(&format!("M {} {} ", points[0].pos.x, points[0].pos.y));
            for point in &points[1..] {
                d.push_str(&format!("L {} {} ", point.pos.x, point.pos.y));
            }
            format!(r#"<path d="{}" {} />"#, d, common)
        }
        ShapeKind::Line { start, end } => format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" {} />"#,
            start.x, start.y, end.x, end.y, common
        ),
        ShapeKind::Arrow { start, end } => {
            let angle = (end.y - start.y).atan2(end.x - start.x);
            let head = 14.0;
            let side = 8.0;
            let p1 = WorldPoint::new(
                end.x - head * angle.cos() + side * (angle + std::f64::consts::FRAC_PI_2).cos(),
                end.y - head * angle.sin() + side * (angle + std::f64::consts::FRAC_PI_2).sin(),
            );
            let p2 = WorldPoint::new(
                end.x - head * angle.cos() + side * (angle - std::f64::consts::FRAC_PI_2).cos(),
                end.y - head * angle.sin() + side * (angle - std::f64::consts::FRAC_PI_2).sin(),
            );
            format!(
                r#"<g><line x1="{}" y1="{}" x2="{}" y2="{}" {} /><polygon points="{},{} {},{} {},{}" fill="{}" /></g>"#,
                start.x,
                start.y,
                end.x,
                end.y,
                common,
                end.x,
                end.y,
                p1.x,
                p1.y,
                p2.x,
                p2.y,
                stroke_color,
            )
        }
        ShapeKind::Rectangle { from, to } | ShapeKind::Image { from, to, .. } => {
            let rect = WorldRect::from_points(*from, *to);
            format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" {} />"#,
                rect.min.x,
                rect.min.y,
                rect.width(),
                rect.height(),
                common
            )
        }
        ShapeKind::Ellipse { from, to } => {
            let rect = WorldRect::from_points(*from, *to);
            format!(
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" {} />"#,
                rect.center().x,
                rect.center().y,
                rect.width() * 0.5,
                rect.height() * 0.5,
                common
            )
        }
        ShapeKind::Diamond { from, to } => {
            let rect = WorldRect::from_points(*from, *to);
            let center = rect.center();
            format!(
                r#"<polygon points="{},{} {},{} {},{} {},{}" {} />"#,
                center.x,
                rect.min.y,
                rect.max.x,
                center.y,
                center.x,
                rect.max.y,
                rect.min.x,
                center.y,
                common
            )
        }
        ShapeKind::Triangle { from, to } => {
            let rect = WorldRect::from_points(*from, *to);
            format!(
                r#"<polygon points="{},{} {},{} {},{}" {} />"#,
                (rect.min.x + rect.max.x) * 0.5,
                rect.min.y,
                rect.max.x,
                rect.max.y,
                rect.min.x,
                rect.max.y,
                common
            )
        }
        ShapeKind::Polygon {
            center,
            radius,
            sides,
            rotation,
        } => {
            let points = polygon_vertices(*center, *radius, *sides, *rotation);
            let points = points
                .iter()
                .map(|p| format!("{},{}", p.x, p.y))
                .collect::<Vec<_>>()
                .join(" ");
            format!(r#"<polygon points="{}" {} />"#, points, common)
        }
        ShapeKind::Star {
            center,
            outer_radius,
            inner_ratio,
            points,
            rotation,
        } => {
            let points = star_vertices(*center, *outer_radius, *inner_ratio, *points, *rotation);
            let points = points
                .iter()
                .map(|p| format!("{},{}", p.x, p.y))
                .collect::<Vec<_>>()
                .join(" ");
            format!(r#"<polygon points="{}" {} />"#, points, common)
        }
        ShapeKind::Text { pos, text, size } => format!(
            r#"<text x="{}" y="{}" font-size="{}" fill="{}" font-family="sans-serif">{}</text>"#,
            pos.x,
            pos.y + *size as f64,
            size,
            stroke_color,
            xml_escape(text)
        ),
    }
}

fn css_color(color: crate::model::Rgba) -> String {
    let alpha = color.a as f64 / 255.0;
    format!("rgba({}, {}, {}, {:.4})", color.r, color.g, color.b, alpha)
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn draw_shape_to_pixmap(pixmap: &mut Pixmap, shape: &Shape, bounds: WorldRect, scale: f32) {
    let map = |p: WorldPoint| -> (f32, f32) {
        (
            ((p.x - bounds.min.x) as f32) * scale,
            ((p.y - bounds.min.y) as f32) * scale,
        )
    };

    let mut stroke_paint = Paint::default();
    let stroke_color = shape.style.effective_stroke();
    stroke_paint.set_color_rgba8(
        stroke_color.r,
        stroke_color.g,
        stroke_color.b,
        stroke_color.a,
    );
    stroke_paint.anti_alias = true;

    let fill_paint = shape.style.effective_fill().map(|fill| {
        let mut paint = Paint::default();
        paint.set_color_rgba8(fill.r, fill.g, fill.b, fill.a);
        paint.anti_alias = true;
        paint
    });

    let stroke = Stroke {
        width: (shape.style.stroke_width * scale).max(1.0),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };

    let draw_poly = |pixmap: &mut Pixmap,
                     points: &[WorldPoint],
                     fill: &Option<Paint>,
                     stroke_paint: &Paint,
                     stroke: &Stroke| {
        if points.len() < 2 {
            return;
        }
        let mut pb = PathBuilder::new();
        let (x0, y0) = map(points[0]);
        pb.move_to(x0, y0);
        for point in &points[1..] {
            let (x, y) = map(*point);
            pb.line_to(x, y);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            if let Some(fill) = fill {
                pixmap.fill_path(&path, fill, FillRule::Winding, Transform::identity(), None);
            }
            pixmap.stroke_path(&path, stroke_paint, stroke, Transform::identity(), None);
        }
    };

    match &shape.kind {
        ShapeKind::Freehand { points } => {
            if points.len() < 2 {
                return;
            }
            let mut pb = PathBuilder::new();
            let (x0, y0) = map(points[0].pos);
            pb.move_to(x0, y0);
            for point in &points[1..] {
                let (x, y) = map(point.pos);
                pb.line_to(x, y);
            }
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
            }
        }
        ShapeKind::Line { start, end } => {
            let mut pb = PathBuilder::new();
            let (x0, y0) = map(*start);
            let (x1, y1) = map(*end);
            pb.move_to(x0, y0);
            pb.line_to(x1, y1);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
            }
        }
        ShapeKind::Arrow { start, end } => {
            let mut pb = PathBuilder::new();
            let (x0, y0) = map(*start);
            let (x1, y1) = map(*end);
            pb.move_to(x0, y0);
            pb.line_to(x1, y1);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
            }

            let angle = (end.y - start.y).atan2(end.x - start.x);
            let head = 14.0;
            let side = 8.0;
            let p1 = WorldPoint::new(
                end.x - head * angle.cos() + side * (angle + std::f64::consts::FRAC_PI_2).cos(),
                end.y - head * angle.sin() + side * (angle + std::f64::consts::FRAC_PI_2).sin(),
            );
            let p2 = WorldPoint::new(
                end.x - head * angle.cos() + side * (angle - std::f64::consts::FRAC_PI_2).cos(),
                end.y - head * angle.sin() + side * (angle - std::f64::consts::FRAC_PI_2).sin(),
            );
            draw_poly(
                pixmap,
                &[*end, p1, p2],
                &Some(stroke_paint.clone()),
                &stroke_paint,
                &stroke,
            );
        }
        ShapeKind::Rectangle { from, to } | ShapeKind::Image { from, to, .. } => {
            let rect = WorldRect::from_points(*from, *to);
            let (x, y) = map(rect.min);
            let width = (rect.width() as f32 * scale).max(1.0);
            let height = (rect.height() as f32 * scale).max(1.0);
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
                let path = PathBuilder::from_rect(rect);
                if let Some(fill) = &fill_paint {
                    pixmap.fill_path(&path, fill, FillRule::Winding, Transform::identity(), None);
                }
                pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
            }
        }
        ShapeKind::Ellipse { from, to } => {
            let rect = WorldRect::from_points(*from, *to);
            let center = rect.center();
            let rx = rect.width() * 0.5;
            let ry = rect.height() * 0.5;
            let mut points = Vec::with_capacity(90);
            for i in 0..90 {
                let t = i as f64 / 90.0;
                let angle = t * std::f64::consts::TAU;
                points.push(WorldPoint::new(
                    center.x + rx * angle.cos(),
                    center.y + ry * angle.sin(),
                ));
            }
            draw_poly(pixmap, &points, &fill_paint, &stroke_paint, &stroke);
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
            draw_poly(pixmap, &points, &fill_paint, &stroke_paint, &stroke);
        }
        ShapeKind::Triangle { from, to } => {
            let rect = WorldRect::from_points(*from, *to);
            let points = [
                WorldPoint::new((rect.min.x + rect.max.x) * 0.5, rect.min.y),
                WorldPoint::new(rect.max.x, rect.max.y),
                WorldPoint::new(rect.min.x, rect.max.y),
            ];
            draw_poly(pixmap, &points, &fill_paint, &stroke_paint, &stroke);
        }
        ShapeKind::Polygon {
            center,
            radius,
            sides,
            rotation,
        } => {
            let points = polygon_vertices(*center, *radius, *sides, *rotation);
            draw_poly(pixmap, &points, &fill_paint, &stroke_paint, &stroke);
        }
        ShapeKind::Star {
            center,
            outer_radius,
            inner_ratio,
            points,
            rotation,
        } => {
            let points = star_vertices(*center, *outer_radius, *inner_ratio, *points, *rotation);
            draw_poly(pixmap, &points, &fill_paint, &stroke_paint, &stroke);
        }
        ShapeKind::Text { pos, size, .. } => {
            let marker = WorldRect::from_center(*pos, *size as f64 * 1.7, *size as f64 * 0.8);
            let pts = marker.corners();
            draw_poly(pixmap, &pts, &None, &stroke_paint, &stroke);
        }
    }
}
