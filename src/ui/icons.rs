//! Line-art icons drawn directly onto a [`Canvas`].
//!
//! Every glyph is designed on a 16x16 grid and scaled into the `size` box whose
//! top-left corner is `(x, y)`, so callers only pick a position, a size and a
//! color.

use crate::render::{Canvas, Rgba};

/// Maps 16x16 design coordinates onto the canvas.
struct Grid {
    x: f32,
    y: f32,
    size: f32,
}

impl Grid {
    fn new(x: f32, y: f32, size: f32) -> Self {
        Self {
            x,
            y,
            size,
        }
    }

    fn p(&self, gx: f32, gy: f32) -> (f32, f32) {
        (self.x + self.u(gx), self.y + self.u(gy))
    }

    fn u(&self, n: f32) -> f32 {
        n * self.size / 16.0
    }

    fn stroke(&self) -> f32 {
        (self.size / 11.0).max(1.0)
    }
}

/// Shifts a color towards white (positive) or black (negative).
fn shade(color: Rgba, amount: f32) -> Rgba {
    let mix = |c: u8| {
        let target = if amount >= 0.0 { 255.0 } else { 0.0 };
        (c as f32 + (target - c as f32) * amount.abs()).round() as u8
    };
    Rgba::new(mix(color.r), mix(color.g), mix(color.b), color.a)
}

pub(crate) fn chevron_left(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(
        &[g.p(10.5, 2.5), g.p(5.0, 8.0), g.p(10.5, 13.5)],
        color,
        g.stroke(),
    );
}

pub(crate) fn chevron_right(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(
        &[g.p(5.5, 2.5), g.p(11.0, 8.0), g.p(5.5, 13.5)],
        color,
        g.stroke(),
    );
}

pub(crate) fn arrow_up(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(&[g.p(8.0, 13.0), g.p(8.0, 3.5)], color, g.stroke());
    canvas.stroke_polyline(
        &[g.p(3.5, 8.0), g.p(8.0, 3.5), g.p(12.5, 8.0)],
        color,
        g.stroke(),
    );
}

pub(crate) fn home(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(
        &[g.p(2.5, 8.0), g.p(8.0, 3.0), g.p(13.5, 8.0)],
        color,
        g.stroke(),
    );
    canvas.stroke_polyline(
        &[
            g.p(4.5, 7.6),
            g.p(4.5, 13.0),
            g.p(11.5, 13.0),
            g.p(11.5, 7.6),
        ],
        color,
        g.stroke(),
    );
}

pub(crate) fn eye(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(&eye_outline(&g), color, g.stroke() * 0.9);
    canvas.stroke_circle(
        g.p(8.0, 8.0).0,
        g.p(8.0, 8.0).1,
        g.u(2.0),
        color,
        g.stroke() * 0.9,
    );
}

pub(crate) fn eye_off(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(&eye_outline(&g), color, g.stroke() * 0.9);
    canvas.stroke_polyline(&[g.p(3.0, 13.0), g.p(13.0, 3.0)], color, g.stroke() * 0.9);
}

fn eye_outline(g: &Grid) -> Vec<(f32, f32)> {
    vec![
        g.p(1.5, 8.0),
        g.p(4.5, 5.0),
        g.p(8.0, 4.0),
        g.p(11.5, 5.0),
        g.p(14.5, 8.0),
        g.p(11.5, 11.0),
        g.p(8.0, 12.0),
        g.p(4.5, 11.0),
        g.p(1.5, 8.0),
    ]
}

pub(crate) fn search(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    let (cx, cy) = g.p(6.8, 6.8);
    canvas.stroke_circle(cx, cy, g.u(4.2), color, g.stroke() * 0.9);
    canvas.stroke_polyline(&[g.p(10.2, 10.2), g.p(14.0, 14.0)], color, g.stroke() * 0.9);
}

/// Filled folder, used for directory rows and folder-like places.
pub(crate) fn folder(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    let (tab_x, tab_y) = g.p(1.5, 2.4);
    canvas.fill_rounded_rect(
        tab_x,
        tab_y,
        g.u(6.2),
        g.u(4.0),
        g.u(1.0),
        shade(color, -0.18),
    );
    let (body_x, body_y) = g.p(1.5, 5.2);
    canvas.fill_rounded_rect(body_x, body_y, g.u(13.0), g.u(8.4), g.u(1.4), color);
}

/// Outlined folder, used where the sidebar's line-art icons need to match.
pub(crate) fn folder_outline(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(
        &[
            g.p(2.0, 13.0),
            g.p(2.0, 3.5),
            g.p(6.0, 3.5),
            g.p(7.4, 5.5),
            g.p(14.0, 5.5),
            g.p(14.0, 13.0),
            g.p(2.0, 13.0),
        ],
        color,
        g.stroke() * 0.9,
    );
}

/// Filled sheet with a folded corner, used for file rows.
pub(crate) fn document(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.fill_polygon(
        &[
            g.p(3.0, 2.0),
            g.p(9.5, 2.0),
            g.p(13.0, 5.5),
            g.p(13.0, 14.0),
            g.p(3.0, 14.0),
        ],
        color,
    );
    canvas.fill_polygon(
        &[g.p(9.5, 2.0), g.p(13.0, 5.5), g.p(9.5, 5.5)],
        shade(color, -0.3),
    );
}

pub(crate) fn desktop(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    let (bx, by) = g.p(2.0, 3.0);
    canvas.stroke_rounded_rect(
        bx,
        by,
        g.u(12.0),
        g.u(8.0),
        g.u(1.2),
        color,
        g.stroke() * 0.9,
    );
    canvas.stroke_polyline(&[g.p(6.0, 13.5), g.p(10.0, 13.5)], color, g.stroke() * 0.9);
}

pub(crate) fn documents(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(
        &[
            g.p(3.5, 2.5),
            g.p(9.5, 2.5),
            g.p(12.5, 5.5),
            g.p(12.5, 13.5),
            g.p(3.5, 13.5),
            g.p(3.5, 2.5),
        ],
        color,
        g.stroke() * 0.9,
    );
    canvas.stroke_polyline(
        &[g.p(9.5, 2.5), g.p(9.5, 5.5), g.p(12.5, 5.5)],
        color,
        g.stroke() * 0.9,
    );
}

pub(crate) fn downloads(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(&[g.p(8.0, 2.5), g.p(8.0, 9.5)], color, g.stroke() * 0.9);
    canvas.stroke_polyline(
        &[g.p(4.5, 6.5), g.p(8.0, 10.0), g.p(11.5, 6.5)],
        color,
        g.stroke() * 0.9,
    );
    canvas.stroke_polyline(
        &[
            g.p(3.0, 11.0),
            g.p(3.0, 13.5),
            g.p(13.0, 13.5),
            g.p(13.0, 11.0),
        ],
        color,
        g.stroke() * 0.9,
    );
}

pub(crate) fn pictures(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    let (bx, by) = g.p(2.0, 3.0);
    canvas.stroke_rounded_rect(
        bx,
        by,
        g.u(12.0),
        g.u(10.0),
        g.u(1.2),
        color,
        g.stroke() * 0.9,
    );
    let (sx, sy) = g.p(5.5, 6.5);
    canvas.fill_circle(sx, sy, g.u(1.2), color);
    canvas.stroke_polyline(
        &[
            g.p(3.0, 12.0),
            g.p(6.5, 8.5),
            g.p(9.5, 11.0),
            g.p(11.0, 9.5),
            g.p(13.0, 12.0),
        ],
        color,
        g.stroke() * 0.9,
    );
}

pub(crate) fn music(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    canvas.stroke_polyline(
        &[
            g.p(6.0, 12.0),
            g.p(6.0, 3.5),
            g.p(12.5, 2.5),
            g.p(12.5, 10.5),
        ],
        color,
        g.stroke() * 0.9,
    );
    let (n1x, n1y) = g.p(4.5, 12.0);
    canvas.fill_circle(n1x, n1y, g.u(1.6), color);
    let (n2x, n2y) = g.p(11.0, 10.5);
    canvas.fill_circle(n2x, n2y, g.u(1.6), color);
}

pub(crate) fn videos(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    let (bx, by) = g.p(2.0, 3.5);
    canvas.stroke_rounded_rect(
        bx,
        by,
        g.u(12.0),
        g.u(9.0),
        g.u(1.2),
        color,
        g.stroke() * 0.9,
    );
    canvas.fill_polygon(&[g.p(6.5, 5.8), g.p(10.5, 8.0), g.p(6.5, 10.2)], color);
}

pub(crate) fn usb_drive(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    let (bx, by) = g.p(4.5, 6.0);
    canvas.stroke_rounded_rect(
        bx,
        by,
        g.u(7.0),
        g.u(8.0),
        g.u(1.2),
        color,
        g.stroke() * 0.9,
    );
    canvas.stroke_polyline(
        &[g.p(6.5, 6.0), g.p(6.5, 2.5), g.p(9.5, 2.5), g.p(9.5, 6.0)],
        color,
        g.stroke() * 0.9,
    );
}

pub(crate) fn hard_drive(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    let (bx, by) = g.p(2.0, 5.0);
    canvas.stroke_rounded_rect(
        bx,
        by,
        g.u(12.0),
        g.u(6.0),
        g.u(1.2),
        color,
        g.stroke() * 0.9,
    );
    let (lx, ly) = g.p(11.5, 8.0);
    canvas.fill_circle(lx, ly, g.u(0.9), color);
}

pub(crate) fn optical(canvas: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba) {
    let g = Grid::new(x, y, size);
    let (cx, cy) = g.p(8.0, 8.0);
    canvas.stroke_circle(cx, cy, g.u(5.5), color, g.stroke() * 0.9);
    canvas.stroke_circle(cx, cy, g.u(1.6), color, g.stroke() * 0.9);
}
