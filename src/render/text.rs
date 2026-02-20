use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use ab_glyph::{
    Font as _, FontArc, Glyph, GlyphId, GlyphImageFormat, OutlinedGlyph, PxScaleFont, Rect,
    ScaleFont, point,
};
use tiny_skia::Pixmap;

use super::{Canvas, Rgba, rgb};

const FALLBACK_FONT: &[u8] = include_bytes!("../../assets/Cantarell-Regular.ttf");

pub struct Font {
    primary: PxScaleFont<FontArc>,
    emoji: Option<PxScaleFont<FontArc>>,
    px_scale: ab_glyph::PxScale,
}

const BASE_FONT_SIZE: f32 = 18.0;

struct SystemFontEntry {
    path: PathBuf,
    priority: u8,
}

static SYSTEM_FONTS: OnceLock<Vec<SystemFontEntry>> = OnceLock::new();

// Only fonts that have been returned as a fallback match stay in memory.
// Fonts loaded-and-checked but not matching are dropped immediately.
struct CachedFont {
    font: Option<FontArc>,
    load_failed: bool,
}

static FALLBACK_CACHE: OnceLock<Mutex<Vec<CachedFont>>> = OnceLock::new();

fn discover_system_fonts() -> Vec<SystemFontEntry> {
    let mut font_dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];

    if let Some(home) = dirs::home_dir() {
        font_dirs.push(home.join(".fonts"));
        font_dirs.push(home.join(".local/share/fonts"));
        // NixOS user profile
        font_dirs.push(home.join(".nix-profile/share/fonts"));
    }

    // XDG data dirs
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for dir in xdg.split(':') {
            font_dirs.push(PathBuf::from(dir).join("fonts"));
            // NixOS often uses X11/fonts
            font_dirs.push(PathBuf::from(dir).join("X11/fonts"));
        }
    }

    // Parse fontconfig's configured font directories
    for dir in fontconfig_dirs() {
        font_dirs.push(dir);
    }

    let mut entries = Vec::new();

    for dir in font_dirs {
        collect_fonts_recursive(&dir, &mut entries);
    }

    // Deduplicate by resolved path
    let mut seen = HashSet::new();
    entries.retain(|e| {
        let key = e.path.canonicalize().unwrap_or_else(|_| e.path.clone());
        seen.insert(key)
    });

    entries.sort_by_key(|e| e.priority);
    entries
}

/// Parse `<dir>` entries from fontconfig's fonts.conf.
fn fontconfig_dirs() -> Vec<PathBuf> {
    let conf_paths = [
        PathBuf::from("/etc/fonts/fonts.conf"),
        PathBuf::from("/etc/fonts/conf.d"),
    ];

    let mut dirs = Vec::new();
    for conf in &conf_paths {
        if conf.is_file() {
            parse_fontconfig_file(conf, &mut dirs);
        } else if conf.is_dir() {
            if let Ok(rd) = std::fs::read_dir(conf) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("conf") {
                        parse_fontconfig_file(&path, &mut dirs);
                    }
                }
            }
        }
    }
    dirs
}

/// Extract `<dir>` element content from a fontconfig XML file (simple text parsing).
fn parse_fontconfig_file(path: &Path, dirs: &mut Vec<PathBuf>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Simple extraction: find <dir>...</dir> patterns
    let mut rest = content.as_str();
    while let Some(start) = rest.find("<dir>") {
        let after = &rest[start + 5..];
        if let Some(end) = after.find("</dir>") {
            let dir = after[..end].trim();
            // Expand ~ prefix
            let expanded = if let Some(stripped) = dir.strip_prefix('~') {
                dirs::home_dir().map(|h| h.join(stripped.strip_prefix('/').unwrap_or(stripped)))
            } else {
                Some(PathBuf::from(dir))
            };
            if let Some(p) = expanded {
                dirs.push(p);
            }
            rest = &after[end + 6..];
        } else {
            break;
        }
    }
}

fn collect_fonts_recursive(dir: &Path, entries: &mut Vec<SystemFontEntry>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_fonts_recursive(&path, entries);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_ascii_lowercase().as_str() {
                "ttf" | "otf" | "ttc" => {
                    let priority = font_priority(&path);
                    entries.push(SystemFontEntry {
                        path,
                        priority,
                    });
                }
                _ => {}
            }
        }
    }
}

fn font_priority(path: &Path) -> u8 {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Skip variable fonts (brackets in name) and exotic scripts
    let is_exotic = name.contains("[")
        || name.contains("]")
        || name.contains("adlam")
        || name.contains("arabic")
        || name.contains("hebrew")
        || name.contains("thai")
        || name.contains("cjk")
        || name.contains("korean")
        || name.contains("japanese")
        || name.contains("indic")
        || name.contains("syriac")
        || name.contains("myanmar")
        || name.contains("ethiopic");

    if is_exotic {
        return 255;
    }

    // Skip emoji/symbol fonts for primary (keep them for fallback only)
    let is_emoji = name.contains("emoji")
        || name.contains("color")
        || name.contains("symbol")
        || name.contains("nerdfont");

    if is_emoji {
        return 8;
    }

    // Deprioritize bold/italic/mono variants
    let is_variant = name.contains("bold")
        || name.contains("italic")
        || name.contains("oblique")
        || name.contains("mono")
        || name.contains("condensed")
        || name.contains("light")
        || name.contains("thin")
        || name.contains("black")
        || name.contains("semibold")
        || name.contains("extrabold");

    // Prefer common sans-serif fonts
    let base = if name == "notosans" || name == "notosans-regular" {
        1
    } else if name == "dejavusans" {
        2
    } else if name.contains("liberation") && name.contains("sans") {
        3
    } else if name.contains("ubuntu") && name.contains("regular") {
        4
    } else if name == "cantarell" || name == "cantarell-regular" {
        5
    } else if name.contains("noto") && name.contains("sans") {
        6
    } else if name.contains("sans") {
        10
    } else {
        50
    };

    if is_variant { base + 50 } else { base }
}

fn ensure_fallback_cache() {
    let fonts = SYSTEM_FONTS.get_or_init(discover_system_fonts);
    FALLBACK_CACHE.get_or_init(|| {
        let cache: Vec<CachedFont> = fonts
            .iter()
            .map(|_| {
                CachedFont {
                    font: None,
                    load_failed: false,
                }
            })
            .collect();
        Mutex::new(cache)
    });
}

fn find_fallback_for_char(c: char) -> Option<FontArc> {
    ensure_fallback_cache();

    let fonts = SYSTEM_FONTS.get().unwrap();
    let cache_mutex = FALLBACK_CACHE.get().unwrap();
    let mut cache = cache_mutex.lock().unwrap();

    // Fast path: check active (matched) fonts already in memory
    for entry in cache.iter() {
        if let Some(ref font) = entry.font {
            if font.glyph_id(c).0 != 0 {
                return Some(font.clone());
            }
        }
    }

    // Slow path: progressively load and check fonts not yet in memory
    for i in 0..cache.len() {
        if cache[i].font.is_some() || cache[i].load_failed {
            continue;
        }

        let loaded = std::fs::read(&fonts[i].path)
            .ok()
            .and_then(|data| FontArc::try_from_vec(data).ok());

        match loaded {
            Some(font) => {
                if font.glyph_id(c).0 != 0 {
                    let result = font.clone();
                    cache[i].font = Some(font);
                    return Some(result);
                }
                // Doesn't have this glyph — drop font data, don't cache
            }
            None => {
                cache[i].load_failed = true;
            }
        }
    }

    None
}

struct PlacedGlyph {
    glyph: Glyph,
    fallback: Option<FontArc>,
}

enum RenderedGlyph {
    Outlined(OutlinedGlyph),
    Raster { pixmap: Pixmap, x: f32, y: f32 },
}

impl RenderedGlyph {
    fn bounds(&self) -> Rect {
        match self {
            Self::Outlined(g) => g.px_bounds(),
            Self::Raster {
                pixmap,
                x,
                y,
            } => {
                Rect {
                    min: point(*x, *y),
                    max: point(*x + pixmap.width() as f32, *y + pixmap.height() as f32),
                }
            }
        }
    }
}

impl Font {
    /// Loads the font with the given scale factor for crisp rendering.
    pub fn load(scale: f32) -> Self {
        let px_scale = ab_glyph::PxScale::from(BASE_FONT_SIZE * scale);
        let text_font = Self::load_text_font();
        let emoji_font = Self::load_emoji_font();
        Self {
            primary: text_font.into_scaled(px_scale),
            emoji: emoji_font.map(|f| f.into_scaled(px_scale)),
            px_scale,
        }
    }

    /// Loads the font with a specific size in pixels (already scaled).
    pub fn load_with_size(size: f32) -> Self {
        let px_scale = ab_glyph::PxScale::from(size);
        let text_font = Self::load_text_font();
        let emoji_font = Self::load_emoji_font();
        Self {
            primary: text_font.into_scaled(px_scale),
            emoji: emoji_font.map(|f| f.into_scaled(px_scale)),
            px_scale,
        }
    }

    /// Loads the best available text font (not emoji).
    fn load_text_font() -> FontArc {
        let system_fonts = SYSTEM_FONTS.get_or_init(discover_system_fonts);

        for entry in system_fonts {
            // Skip emoji/symbol fonts for primary text
            let name = entry
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            if name.contains("emoji") || name.contains("color") || name.contains("symbol") {
                continue;
            }

            if let Ok(data) = std::fs::read(&entry.path) {
                if let Ok(font) = FontArc::try_from_vec(data) {
                    return font;
                }
            }
        }

        FontArc::try_from_slice(FALLBACK_FONT).unwrap()
    }

    /// Loads an emoji font if available.
    fn load_emoji_font() -> Option<FontArc> {
        let system_fonts = SYSTEM_FONTS.get_or_init(discover_system_fonts);

        for entry in system_fonts {
            let name = entry
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            // Only load emoji fonts
            if !name.contains("emoji") && !name.contains("color") {
                continue;
            }

            if let Ok(data) = std::fs::read(&entry.path) {
                if let Ok(font) = FontArc::try_from_vec(data) {
                    return Some(font);
                }
            }
        }

        None
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
        let placed = self.layout();
        let glyphs = self.resolve_glyphs(placed);

        if glyphs.is_empty() {
            return Canvas::new(1, 1);
        }

        let bounds = glyphs
            .iter()
            .map(|g| g.bounds())
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

        for g in &glyphs {
            match g {
                RenderedGlyph::Outlined(og) => {
                    let glyph_bounds = og.px_bounds();
                    let gx = glyph_bounds.min.x.floor() as i32 + base_x;
                    let gy = glyph_bounds.min.y.floor() as i32 + base_y;

                    let pixels = pixmap.pixels_mut();
                    og.draw(|x, y, c| {
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

                                    let existing = *pix;
                                    if existing.alpha() == 0 {
                                        *pix =
                                            tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a)
                                                .unwrap();
                                    } else {
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
                RenderedGlyph::Raster {
                    pixmap: src,
                    x,
                    y,
                } => {
                    let dx = x.round() as i32 + base_x;
                    let dy = y.round() as i32 + base_y;
                    pixmap.draw_pixmap(
                        dx,
                        dy,
                        src.as_ref(),
                        &tiny_skia::PixmapPaint::default(),
                        tiny_skia::Transform::identity(),
                        None,
                    );
                }
            }
        }

        Canvas {
            pixmap,
        }
    }

    /// Computes the size of the rendered text without actually rendering it.
    pub fn measure(&self) -> (f32, f32) {
        let placed = self.layout();
        let glyphs = self.resolve_glyphs(placed);

        let bounds = glyphs
            .iter()
            .map(|g| g.bounds())
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

    /// Converts placed glyphs into rendered form (outlined vectors or raster bitmaps).
    fn resolve_glyphs(&self, placed: Vec<PlacedGlyph>) -> Vec<RenderedGlyph> {
        let ppem = self.font.px_scale.y as u16;

        placed
            .into_iter()
            .filter_map(|pg| {
                // Try vector outline first (normal text glyphs)
                let outlined = if let Some(ref fb) = pg.fallback {
                    fb.as_scaled(self.font.px_scale)
                        .outline_glyph(pg.glyph.clone())
                } else {
                    self.font.primary.outline_glyph(pg.glyph.clone())
                };

                if let Some(og) = outlined {
                    return Some(RenderedGlyph::Outlined(og));
                }

                // Try raster image (colored emoji / bitmap glyphs)
                let font_ref: &FontArc = pg.fallback.as_ref().unwrap_or(&self.font.primary.font);

                if let Some(img) = font_ref.glyph_raster_image2(pg.glyph.id, ppem) {
                    if matches!(img.format, GlyphImageFormat::Png) {
                        if let Ok(src) = Pixmap::decode_png(img.data) {
                            let scale = self.font.px_scale.y / img.pixels_per_em as f32;
                            let target_w = (img.width as f32 * scale).round().max(1.0) as u32;
                            let target_h = (img.height as f32 * scale).round().max(1.0) as u32;
                            let scaled = scale_pixmap(&src, target_w, target_h);
                            // origin is offset from (baseline + ascent) in image pixels
                            let fb_ascent = font_ref.as_scaled(self.font.px_scale).ascent();
                            let x = pg.glyph.position.x + img.origin.x * scale;
                            let y = pg.glyph.position.y - fb_ascent + img.origin.y * scale;
                            return Some(RenderedGlyph::Raster {
                                pixmap: scaled,
                                x,
                                y,
                            });
                        }
                    }
                }

                None
            })
            .collect()
    }

    /// Performs text layout with soft wrapping and per-glyph font fallback.
    fn layout(&self) -> Vec<PlacedGlyph> {
        let mut glyphs: Vec<PlacedGlyph> = Vec::new();

        let mut y: f32 = 0.0;
        for line in self.text.lines() {
            let mut x: f32 = 0.0;
            let mut last_softbreak: Option<usize> = None;
            let mut last_primary_glyph: Option<GlyphId> = None;

            for c in line.chars() {
                let primary_glyph_id = self.font.primary.font.glyph_id(c);
                let (glyph_id, fallback) = if primary_glyph_id.0 != 0 {
                    // Primary text font has it
                    (primary_glyph_id, None)
                } else if let Some(ref emoji_font) = self.font.emoji {
                    let emoji_glyph_id = emoji_font.font.glyph_id(c);
                    if emoji_glyph_id.0 != 0 {
                        // Emoji font has it
                        (emoji_glyph_id, Some(emoji_font.font.clone()))
                    } else {
                        // Try system font fallback
                        if let Some(fb) = find_fallback_for_char(c) {
                            let fb_id = fb.glyph_id(c);
                            (fb_id, Some(fb))
                        } else {
                            (primary_glyph_id, None)
                        }
                    }
                } else {
                    // No emoji font loaded, try system font fallback
                    if let Some(fb) = find_fallback_for_char(c) {
                        let fb_id = fb.glyph_id(c);
                        (fb_id, Some(fb))
                    } else {
                        (primary_glyph_id, None)
                    }
                };

                // Only kern within the same (primary) font
                if fallback.is_none() {
                    if let Some(last_id) = last_primary_glyph {
                        x += self.font.primary.kern(last_id, glyph_id);
                    }
                }

                let glyph = Glyph {
                    id: glyph_id,
                    scale: self.font.px_scale,
                    position: point(x.round(), y.round()),
                };

                // Advance using the correct font
                let advance = if let Some(ref fb) = fallback {
                    let scaled: PxScaleFont<&FontArc> = fb.as_scaled(self.font.px_scale);
                    scaled.h_advance(glyph_id)
                } else {
                    self.font.primary.h_advance(glyph_id)
                };

                // Track last primary glyph for kerning
                if fallback.is_none() {
                    last_primary_glyph = Some(glyph_id);
                } else {
                    last_primary_glyph = None;
                }

                x += advance;

                if c == ' ' || c == ZWSP {
                    last_softbreak = Some(glyphs.len());
                } else {
                    glyphs.push(PlacedGlyph {
                        glyph,
                        fallback,
                    });

                    if x > self.max_width {
                        if let Some(i) = last_softbreak {
                            y += self.font.primary.height() + self.font.primary.line_gap();
                            let x_diff = glyphs.get(i).map(|g| g.glyph.position.x).unwrap_or(0.0);
                            for pg in &mut glyphs[i..] {
                                pg.glyph.position.x -= x_diff;
                                pg.glyph.position.y = y;
                            }
                            x -= x_diff;
                            last_softbreak = None;
                        }
                    }
                }
            }
            y += self.font.primary.height() + self.font.primary.line_gap();
        }

        glyphs
    }
}

/// Area-averaging downscale for raster emoji bitmaps.
fn scale_pixmap(src: &Pixmap, target_w: u32, target_h: u32) -> Pixmap {
    if src.width() == target_w && src.height() == target_h {
        return src.clone();
    }

    let mut dst = Pixmap::new(target_w, target_h).unwrap();
    let scale_x = src.width() as f32 / target_w as f32;
    let scale_y = src.height() as f32 / target_h as f32;
    let src_pixels = src.pixels();
    let dst_pixels = dst.pixels_mut();
    let src_w = src.width();

    for dy in 0..target_h {
        for dx in 0..target_w {
            let sx0 = (dx as f32 * scale_x) as u32;
            let sy0 = (dy as f32 * scale_y) as u32;
            let sx1 = (((dx + 1) as f32 * scale_x).ceil() as u32).min(src.width());
            let sy1 = (((dy + 1) as f32 * scale_y).ceil() as u32).min(src.height());

            let mut r_sum: u32 = 0;
            let mut g_sum: u32 = 0;
            let mut b_sum: u32 = 0;
            let mut a_sum: u32 = 0;
            let mut count: u32 = 0;

            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    let p = src_pixels[(sy * src_w + sx) as usize];
                    r_sum += p.red() as u32;
                    g_sum += p.green() as u32;
                    b_sum += p.blue() as u32;
                    a_sum += p.alpha() as u32;
                    count += 1;
                }
            }

            if count > 0 {
                dst_pixels[(dy * target_w + dx) as usize] =
                    tiny_skia::PremultipliedColorU8::from_rgba(
                        (r_sum / count) as u8,
                        (g_sum / count) as u8,
                        (b_sum / count) as u8,
                        (a_sum / count) as u8,
                    )
                    .unwrap();
            }
        }
    }

    dst
}

const ZWSP: char = '\u{200b}';
