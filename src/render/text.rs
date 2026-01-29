use ab_glyph::{Font as _, Glyph, OutlinedGlyph, PxScaleFont, ScaleFont, point};
use tiny_skia::Pixmap;

use super::{Canvas, Rgba, rgb};

const FALLBACK_FONT: &[u8] = include_bytes!("../../assets/Cantarell-Regular.ttf");

pub struct Font {
    font: PxScaleFont<ab_glyph::FontRef<'static>>,
}

const BASE_FONT_SIZE: f32 = 18.0;

impl Font {
    /// Loads the font with the given scale factor for crisp rendering.
    pub fn load(scale: f32) -> Self {
        let inner = ab_glyph::FontRef::try_from_slice(FALLBACK_FONT).unwrap();
        Self {
            font: inner.into_scaled(BASE_FONT_SIZE * scale),
        }
    }

    /// Returns a renderer for the given text.
    pub fn render<'a>(&'a self, text: &'a str) -> TextRenderer<'a> {
        TextRenderer {
            font: self,
            text,
            color: rgb(255, 255, 255),
            max_width: f32::MAX,
        }
    }
}

pub struct TextRenderer<'a> {
    font: &'a Font,
    text: &'a str,
    color: Rgba,
    max_width: f32,
}

impl<'a> TextRenderer<'a> {
    pub fn with_color(self, color: Rgba) -> Self {
        Self {
            color,
            ..self
        }
    }

    pub fn with_max_width(self, max_width: f32) -> Self {
        Self {
            max_width,
            ..self
        }
    }

    /// Renders the text and returns a Canvas containing it.
    pub fn finish(self) -> Canvas {
        let glyphs = self.layout();

        if glyphs.is_empty() {
            return Canvas::new(1, 1);
        }

        let bounds = glyphs
            .iter()
            .map(|g| g.px_bounds())
            .reduce(|mut sum, next| {
                sum.min.x = f32::min(sum.min.x, next.min.x);
                sum.min.y = f32::min(sum.min.y, next.min.y);
                sum.max.x = f32::max(sum.max.x, next.max.x);
                sum.max.y = f32::max(sum.max.y, next.max.y);
                sum
            })
            .unwrap_or_default();

        // Add padding to avoid clipping
        let width = (bounds.width().ceil() as u32 + 2).max(1);
        let height = (bounds.height().ceil() as u32 + 2).max(1);

        let mut pixmap = Pixmap::new(width, height).unwrap();
        let pixels = pixmap.pixels_mut();

        // Offset to account for bounds.min (which can be negative for some glyphs)
        let base_x = -bounds.min.x.floor() as i32 + 1;
        let base_y = -bounds.min.y.floor() as i32 + 1;

        for g in glyphs {
            let glyph_bounds = g.px_bounds();
            // Use floor for proper pixel alignment
            let gx = glyph_bounds.min.x.floor() as i32 + base_x;
            let gy = glyph_bounds.min.y.floor() as i32 + base_y;

            g.draw(|x, y, c| {
                let px = gx + x as i32;
                let py = gy + y as i32;

                if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
                    let idx = (py as u32 * width + px as u32) as usize;
                    if let Some(pix) = pixels.get_mut(idx) {
                        // Premultiplied alpha blending
                        let a = (c * 255.0).round() as u8;
                        if a > 0 {
                            let r = (self.color.r as u32 * a as u32 / 255) as u8;
                            let g = (self.color.g as u32 * a as u32 / 255) as u8;
                            let b = (self.color.b as u32 * a as u32 / 255) as u8;

                            // Blend with existing pixel (SrcOver)
                            let existing = *pix;
                            if existing.alpha() == 0 {
                                *pix =
                                    tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a).unwrap();
                            } else {
                                // Alpha composite
                                let ea = existing.alpha() as u32;
                                let er = existing.red() as u32;
                                let eg = existing.green() as u32;
                                let eb = existing.blue() as u32;

                                let inv_a = 255 - a as u32;
                                let out_a = (a as u32 + ea * inv_a / 255).min(255) as u8;
                                let out_r = (r as u32 + er * inv_a / 255).min(255) as u8;
                                let out_g = (g as u32 + eg * inv_a / 255).min(255) as u8;
                                let out_b = (b as u32 + eb * inv_a / 255).min(255) as u8;

                                *pix = tiny_skia::PremultipliedColorU8::from_rgba(
                                    out_r, out_g, out_b, out_a,
                                )
                                .unwrap();
                            }
                        }
                    }
                }
            });
        }

        Canvas {
            pixmap,
        }
    }

    /// Computes the size of the rendered text without actually rendering it.
    pub fn measure(&self) -> (f32, f32) {
        let glyphs = self.layout();

        let bounds = glyphs
            .iter()
            .map(|g| g.px_bounds())
            .reduce(|mut sum, next| {
                sum.min.x = f32::min(sum.min.x, next.min.x);
                sum.min.y = f32::min(sum.min.y, next.min.y);
                sum.max.x = f32::max(sum.max.x, next.max.x);
                sum.max.y = f32::max(sum.max.y, next.max.y);
                sum
            })
            .unwrap_or_default();

        (bounds.width(), bounds.height())
    }

    /// Performs text layout with soft wrapping.
    fn layout(&self) -> Vec<OutlinedGlyph> {
        let mut glyphs: Vec<Glyph> = Vec::new();

        let mut y: f32 = 0.0;
        for line in self.text.lines() {
            let mut x: f32 = 0.0;
            let mut last_softbreak: Option<usize> = None;
            let mut last = None;

            for c in line.chars() {
                let mut glyph = self.font.font.scaled_glyph(c);
                if let Some(last) = last {
                    x += self.font.font.kern(last, glyph.id);
                }
                // Round positions to pixel boundaries for crisp text
                glyph.position = point(x.round(), y.round());
                last = Some(glyph.id);

                x += self.font.font.h_advance(glyph.id);

                if c == ' ' || c == ZWSP {
                    last_softbreak = Some(glyphs.len());
                } else {
                    glyphs.push(glyph);

                    if x > self.max_width {
                        if let Some(i) = last_softbreak {
                            // Soft line break
                            y += self.font.font.height() + self.font.font.line_gap();
                            let x_diff = glyphs.get(i).map(|g| g.position.x).unwrap_or(0.0);
                            for glyph in &mut glyphs[i..] {
                                glyph.position.x -= x_diff;
                                glyph.position.y = y;
                            }
                            x -= x_diff;
                            last_softbreak = None;
                        }
                    }
                }
            }
            y += self.font.font.height() + self.font.font.line_gap();
        }

        glyphs
            .into_iter()
            .filter_map(|g| self.font.font.outline_glyph(g))
            .collect()
    }
}

const ZWSP: char = '\u{200b}';
