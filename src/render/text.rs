use std::sync::LazyLock;

use ab_glyph::{point, Font as _, FontRef, Glyph, OutlinedGlyph, PxScaleFont, ScaleFont};
use tiny_skia::{Pixmap, Transform};

use super::{rgb, Canvas, Rgba};

const FALLBACK_FONT: &[u8] = include_bytes!("../../assets/Cantarell-Regular.ttf");

/// Global cached font - parsed once and reused for all Font instances.
/// This is thread-safe and initialized lazily on first access.
static CACHED_FONT: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(FALLBACK_FONT).expect("Failed to parse fallback font")
});

pub struct Font {
    font: PxScaleFont<ab_glyph::FontRef<'static>>,
}

const BASE_FONT_SIZE: f32 = 18.0;

impl Font {
    /// Loads the font with the given scale factor for crisp rendering.
    /// Uses the globally cached font to avoid reparsing the font bytes on every call.
    pub fn load(scale: f32) -> Self {
        Self {
            font: CACHED_FONT.clone().into_scaled(BASE_FONT_SIZE * scale),
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

        // Offset to account for bounds.min (which can be negative for some glyphs)
        let base_x = -bounds.min.x.floor() as i32 + 1;
        let base_y = -bounds.min.y.floor() as i32 + 1;

        for g in glyphs {
            // Render glyph to its own pixmap
            if let Some(glyph_pixmap) = render_glyph_to_pixmap(&g, self.color) {
                let glyph_bounds = g.px_bounds();
                // Calculate position for this glyph
                let x = glyph_bounds.min.x.floor() as i32 + base_x;
                let y = glyph_bounds.min.y.floor() as i32 + base_y;

                // Use tiny-skia's native blitting to composite the glyph
                pixmap.draw_pixmap(
                    x,
                    y,
                    glyph_pixmap.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        }

        Canvas {
            pixmap,
            argb_cache: std::cell::RefCell::new(None),
            dirty: std::cell::RefCell::new(true),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_caching() {
        // Test that loading a font multiple times works correctly
        let font1 = Font::load(1.0);
        let font2 = Font::load(1.0);
        let font3 = Font::load(2.0);

        // Verify that fonts with the same scale produce the same measurements
        let text = "Hello, World!";
        let renderer1 = font1.render(text);
        let renderer2 = font2.render(text);
        let (w1, h1) = renderer1.measure();
        let (w2, h2) = renderer2.measure();

        // Fonts with same scale should have same measurements
        assert_eq!(w1, w2, "Fonts with same scale should have same width");
        assert_eq!(h1, h2, "Fonts with same scale should have same height");

        // Font with different scale should have different measurements
        let renderer3 = font3.render(text);
        let (w3, h3) = renderer3.measure();

        assert_ne!(
            w1, w3,
            "Fonts with different scales should have different widths"
        );
        assert_ne!(
            h1, h3,
            "Fonts with different scales should have different heights"
        );
    }

    #[test]
    fn test_cached_font_is_initialized() {
        // Force initialization of the cached font
        let _ = &*CACHED_FONT;

        // Verify the cached font is valid by checking it can provide glyph IDs
        let glyph_id = CACHED_FONT.glyph_id('A');
        assert_ne!(
            glyph_id,
            ab_glyph::GlyphId(0),
            "Should be able to get glyph ID for 'A'"
        );

        let glyph_id = CACHED_FONT.glyph_id('Z');
        assert_ne!(
            glyph_id,
            ab_glyph::GlyphId(0),
            "Should be able to get glyph ID for 'Z'"
        );
    }

    #[test]
    fn test_font_load_different_scales() {
        // Test that Font::load() works with various scale factors
        let scales = [0.5, 1.0, 1.5, 2.0, 3.0];

        for scale in scales {
            let font = Font::load(scale);
            let renderer = font.render("Test");
            let (width, height) = renderer.measure();

            // All fonts should produce valid measurements
            assert!(
                width > 0.0,
                "Font with scale {} should have positive width",
                scale
            );
            assert!(
                height > 0.0,
                "Font with scale {} should have positive height",
                scale
            );
        }
    }

    #[test]
    fn test_multiple_fonts_same_scale_are_independent() {
        // Test that multiple Font instances with the same scale are independent
        let font1 = Font::load(1.0);
        let font2 = Font::load(1.0);

        // Create two renderers from the same font scale
        let renderer1 = font1.render("First").with_color(rgb(255, 0, 0));
        let renderer2 = font2.render("Second").with_color(rgb(0, 255, 0));

        // Render to canvases
        let canvas1 = renderer1.finish();
        let canvas2 = renderer2.finish();

        // Canvases should be different (different text)
        assert!(canvas1.width() > 0);
        assert!(canvas2.width() > 0);
        // They might have similar dimensions but represent different text
    }

    #[test]
    fn test_cached_font_performance() {
        // Test that loading fonts is fast (indicating caching works)
        use std::time::Instant;

        // Warm up the cache
        let _ = Font::load(1.0);

        // Time loading the same font 1000 times
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = Font::load(1.0);
        }
        let duration = start.elapsed();

        // With caching, 1000 loads should take less than 10ms
        // If there were no caching, parsing the font would be much slower
        assert!(
            duration.as_millis() < 10,
            "Font loading should be fast with caching"
        );
    }

    #[test]
    fn test_text_rendering_creates_valid_canvas() {
        // Test that text rendering creates a valid canvas
        let font = Font::load(1.0);
        let renderer = font.render("Test Text");

        let canvas = renderer.finish();

        // Canvas should have positive dimensions
        assert!(canvas.width() > 0, "Canvas should have positive width");
        assert!(canvas.height() > 0, "Canvas should have positive height");

        // Canvas should have pixel data
        assert!(
            !canvas.pixmap.data().is_empty(),
            "Canvas should have pixel data"
        );
    }

    #[test]
    fn test_text_rendering_with_color() {
        // Test that text rendering with different colors works
        let font = Font::load(1.0);
        let white_text = font.render("Test").with_color(rgb(255, 255, 255));
        let red_text = font.render("Test").with_color(rgb(255, 0, 0));

        let white_canvas = white_text.finish();
        let red_canvas = red_text.finish();

        // Both should create valid canvases
        assert!(white_canvas.width() > 0);
        assert!(red_canvas.width() > 0);
    }

    #[test]
    fn test_empty_text_rendering() {
        // Test that empty text renders to a minimal canvas
        let font = Font::load(1.0);
        let renderer = font.render("");

        let canvas = renderer.finish();

        // Empty text should render to minimal 1x1 canvas
        assert_eq!(canvas.width(), 1, "Empty text should render to 1x1 canvas");
        assert_eq!(canvas.height(), 1, "Empty text should render to 1x1 canvas");
    }

    #[test]
    fn test_multiline_text_rendering() {
        // Test that multiline text renders correctly
        let font = Font::load(1.0);
        let single_line = font.render("Single line");
        let multi_line = font.render("Line 1\nLine 2");

        let single_canvas = single_line.finish();
        let multi_canvas = multi_line.finish();

        // Multiline text should be taller than single line
        assert!(single_canvas.height() > 0);
        assert!(multi_canvas.height() > 0);
        // Multiline should be at least as tall as single line
        assert!(multi_canvas.height() >= single_canvas.height());
    }
}

/// Renders a single glyph to a tiny-skia Pixmap with the given color.
/// This eliminates the per-pixel callback and uses native tiny-skia blitting.
fn render_glyph_to_pixmap(glyph: &OutlinedGlyph, color: Rgba) -> Option<Pixmap> {
    let bounds = glyph.px_bounds();
    let width = (bounds.width().ceil() as u32).max(1);
    let height = (bounds.height().ceil() as u32).max(1);

    if width == 0 || height == 0 {
        return None;
    }

    let mut pixmap = Pixmap::new(width, height)?;
    let pixels = pixmap.pixels_mut();

    // Pre-calculate color components for premultiplied alpha
    let r = color.r;
    let g = color.g;
    let b = color.b;

    // Use ab_glyph's draw callback, but keep it simple - just write directly to pixmap
    // No manual blending, just overwrite pixels (pixmap is transparent by default)
    glyph.draw(|x, y, coverage| {
        if x < width && y < height {
            let idx = (y * width + x) as usize;
            if let Some(pix) = pixels.get_mut(idx) {
                let alpha = (coverage * 255.0).round() as u8;
                if alpha > 0 {
                    // Write premultiplied alpha directly
                    let premultiplied_r = (r as u32 * alpha as u32 / 255) as u8;
                    let premultiplied_g = (g as u32 * alpha as u32 / 255) as u8;
                    let premultiplied_b = (b as u32 * alpha as u32 / 255) as u8;
                    *pix = tiny_skia::PremultipliedColorU8::from_rgba(
                        premultiplied_r,
                        premultiplied_g,
                        premultiplied_b,
                        alpha,
                    )
                    .unwrap();
                }
            }
        }
    });

    Some(pixmap)
}
