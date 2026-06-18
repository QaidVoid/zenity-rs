mod text;

pub(crate) use text::Font;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, PixmapRef, Rect, Transform};

/// A canvas backed by a tiny-skia Pixmap.
/// Stores pixels in RGBA format internally, but can convert to ARGB for X11/Wayland.
pub struct Canvas {
    pub(crate) pixmap: Pixmap,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            pixmap: Pixmap::new(width, height).expect("invalid canvas dimensions"),
        }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Fills the entire canvas with a color.
    pub fn fill(&mut self, color: Rgba) {
        self.pixmap.fill(color.into());
    }

    /// Fills a rectangle with a color.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        let rect = match Rect::from_xywh(x, y, w, h) {
            Some(r) => r,
            None => return,
        };
        let mut paint = Paint::default();
        paint.set_color(color.into());
        paint.anti_alias = true;
        self.pixmap
            .fill_rect(rect, &paint, Transform::identity(), None);
    }

    /// Fills a rounded rectangle with a color.
    pub fn fill_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Rgba) {
        let path = rounded_rect_path(x, y, w, h, radius);
        let mut paint = Paint::default();
        paint.set_color(color.into());
        paint.anti_alias = true;
        self.pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Strokes a rounded rectangle outline.
    #[allow(clippy::too_many_arguments)]
    pub fn stroke_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        color: Rgba,
        width: f32,
    ) {
        let path = rounded_rect_path(x, y, w, h, radius);
        let mut paint = Paint::default();
        paint.set_color(color.into());
        paint.anti_alias = true;
        let stroke = tiny_skia::Stroke {
            width,
            ..Default::default()
        };
        self.pixmap
            .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    /// Draws another canvas onto this one at the given position.
    pub fn draw_canvas(&mut self, other: &Canvas, x: i32, y: i32) {
        self.draw_pixmap(other.pixmap.as_ref(), x, y);
    }

    /// Draws a pixmap onto this canvas at the given position.
    pub fn draw_pixmap(&mut self, src: PixmapRef, x: i32, y: i32) {
        self.pixmap.draw_pixmap(
            x,
            y,
            src,
            &tiny_skia::PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    /// Copies a rectangular region from `src` directly into this canvas (no
    /// blending). Both source and destination are premultiplied RGBA, so a raw
    /// byte copy is exact. Used to restore cached static regions (e.g. the
    /// chrome layer behind dynamic widgets during partial redraws).
    #[allow(clippy::too_many_arguments)]
    pub fn blit_region(
        &mut self,
        src: &Canvas,
        sx: u32,
        sy: u32,
        w: u32,
        h: u32,
        dx: u32,
        dy: u32,
    ) {
        let spw = src.pixmap.width();
        let dpw = self.pixmap.width();
        let sdata = src.pixmap.data();
        let ddata = self.pixmap.data_mut();
        for row in 0..h {
            let s_start = (((sy + row) * spw + sx) as usize) * 4;
            let d_start = (((dy + row) * dpw + dx) as usize) * 4;
            let len = (w as usize) * 4;
            ddata[d_start..d_start + len].copy_from_slice(&sdata[s_start..s_start + len]);
        }
    }

    /// Converts the whole canvas to premultiplied ARGB, reusing `out`'s capacity.
    /// Output byte order is B, G, R, A (little-endian ARGB u32), matching X11/Wayland.
    pub fn argb_into(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.pixmap.data().len());
        swizzle_rgba_to_argb(self.pixmap.data(), out);
    }

    /// Converts a sub-rectangle to premultiplied ARGB, reusing `out`'s capacity.
    /// Pixels are written row-major with no padding.
    pub fn argb_rect_into(&self, x: u32, y: u32, w: u32, h: u32, out: &mut Vec<u8>) {
        let pw = self.pixmap.width();
        out.clear();
        out.reserve((w as usize) * (h as usize) * 4);
        let data = self.pixmap.data();
        for row in 0..h {
            let base = (((y + row) * pw + x) as usize) * 4;
            swizzle_rgba_to_argb(&data[base..base + (w as usize) * 4], out);
        }
    }

    /// Copies + swizzles a sub-rectangle into a destination buffer that holds a
    /// full window pixmap with the given stride (bytes per scanline). Used for
    /// Wayland SHM partial updates where the destination lives in `dst` at the
    /// same coordinates as in the canvas.
    pub fn blit_argb_rect(&self, x: u32, y: u32, w: u32, h: u32, dst: &mut [u8], dst_stride: u32) {
        let pw = self.pixmap.width();
        let data = self.pixmap.data();
        for row in 0..h {
            let src_base = (((y + row) * pw + x) as usize) * 4;
            let dst_base = ((y + row) * dst_stride + x * 4) as usize;
            let sb = &data[src_base..src_base + (w as usize) * 4];
            let db = &mut dst[dst_base..dst_base + (w as usize) * 4];
            for (s, d) in sb.chunks_exact(4).zip(db.chunks_exact_mut(4)) {
                d[0] = s[2]; // B
                d[1] = s[1]; // G
                d[2] = s[0]; // R
                d[3] = s[3]; // A
            }
        }
    }

    /// Fills a dialog background with subtle shadow and border.
    pub fn fill_dialog_bg(
        &mut self,
        width: f32,
        height: f32,
        bg_color: Rgba,
        border_color: Rgba,
        shadow_color: Rgba,
        radius: f32,
    ) {
        let shadow_offset = 3.0;
        let border_width = 1.0;

        // Draw shadow (slightly smaller to be fully covered by background)
        self.fill_rounded_rect(
            shadow_offset,
            shadow_offset,
            width - shadow_offset,
            height - shadow_offset,
            radius,
            shadow_color,
        );

        // Draw main background (covers shadow completely)
        self.fill_rounded_rect(0.0, 0.0, width, height, radius, bg_color);

        // Draw border (inset by half border width)
        let inset = border_width * 0.5;
        self.stroke_rounded_rect(
            inset,
            inset,
            width - inset * 2.0,
            height - inset * 2.0,
            radius,
            border_color,
            border_width,
        );
    }
}

/// Appends RGBA pixels (R,G,B,A byte order) as premultiplied ARGB
/// (B,G,R,A byte order) to `out`. tiny-skia already stores premultiplied alpha,
/// so no un-premultiplication is needed.
fn swizzle_rgba_to_argb(rgba: &[u8], out: &mut Vec<u8>) {
    for c in rgba.chunks_exact(4) {
        out.push(c[2]); // B
        out.push(c[1]); // G
        out.push(c[0]); // R
        out.push(c[3]); // A
    }
}

/// Creates a rounded rectangle path.
fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();

    // Clamp radius to half the smallest dimension
    let r = r.min(w / 2.0).min(h / 2.0);

    // Top-left corner
    pb.move_to(x + r, y);
    // Top edge
    pb.line_to(x + w - r, y);
    // Top-right corner
    pb.quad_to(x + w, y, x + w, y + r);
    // Right edge
    pb.line_to(x + w, y + h - r);
    // Bottom-right corner
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    // Bottom edge
    pb.line_to(x + r, y + h);
    // Bottom-left corner
    pb.quad_to(x, y + h, x, y + h - r);
    // Left edge
    pb.line_to(x, y + r);
    // Top-left corner
    pb.quad_to(x, y, x + r, y);

    pb.close();
    pb.finish().unwrap()
}

/// RGBA color with 8-bit components.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r,
            g,
            b,
            a,
        }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            a: 255,
        }
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        Self {
            a,
            ..self
        }
    }
}

impl From<Rgba> for Color {
    fn from(c: Rgba) -> Self {
        Color::from_rgba8(c.r, c.g, c.b, c.a)
    }
}

/// Convenience function to create an RGB color.
pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::rgb(r, g, b)
}
