//! Icon and popup drawing for the file selection dialog.

use super::{
    POPUP_ITEM_HEIGHT,
    fs::{MountIcon, QuickAccessIcon},
};
use crate::{
    render::{Canvas, Font, Rgba, rgb},
    ui::{Colors, icons},
};

pub(super) const MAX_POPUP_ITEMS: usize = 8;

/// Truncates `text` to fit within `max_width` pixels, appending an ellipsis
/// ("…") when it had to be cut. Width is color-independent, so measurement uses
/// a placeholder color.
fn truncate_to_width(text: &str, max_width: i32, font: &Font) -> String {
    let measure = |s: &str| font.render(s).with_color(rgb(0, 0, 0)).finish().width() as i32;
    if measure(text) <= max_width {
        return text.to_string();
    }
    let ellipsis = "…";
    let ell_w = measure(ellipsis);
    if max_width <= ell_w {
        return ellipsis.to_string();
    }
    let mut out = String::new();
    let mut w = 0i32;
    for c in text.chars() {
        let cw = measure(&c.to_string());
        if w + cw > max_width - ell_w {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

pub(super) fn draw_completion_popup(
    canvas: &mut Canvas,
    font: &Font,
    colors: &Colors,
    matches: &[String],
    selected: usize,
    position: (i32, i32),
    width: u32,
) {
    let (x, y) = position;
    if matches.is_empty() {
        return;
    }
    let visible = matches.len().min(MAX_POPUP_ITEMS);
    let popup_h = (visible as i32) * POPUP_ITEM_HEIGHT + 2; // 1px border top+bottom

    // Background
    canvas.fill_rounded_rect(
        x as f32,
        y as f32,
        width as f32,
        popup_h as f32,
        6.0,
        colors.surface,
    );
    // Border
    canvas.stroke_rounded_rect(
        x as f32,
        y as f32,
        width as f32,
        popup_h as f32,
        6.0,
        colors.separator,
        1.0,
    );

    for (i, name) in matches.iter().take(visible).enumerate() {
        let item_y = y + 1 + (i as i32) * POPUP_ITEM_HEIGHT;

        // Highlight selected item
        if i == selected {
            canvas.fill_rounded_rect(
                (x + 3) as f32,
                item_y as f32,
                (width - 6) as f32,
                POPUP_ITEM_HEIGHT as f32,
                4.0,
                colors.row_selected,
            );
        }

        let display = truncate_to_width(name, width as i32 - 12, font);
        let (label, base) = font
            .render(&display)
            .with_color(colors.text)
            .finish_with_baseline();
        let text_y = item_y
            + ((POPUP_ITEM_HEIGHT as f32 - font.line_height()) / 2.0 + font.ascent()).round()
                as i32
            - base;
        canvas.draw_canvas(&label, x + 8, text_y);
    }
}

/// Which glyph a sidebar row shows.
#[derive(Clone, Copy)]
pub(super) enum SidebarGlyph {
    Place(QuickAccessIcon),
    Mount(MountIcon),
}

pub(super) fn draw_place_icon(
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    size: f32,
    icon: QuickAccessIcon,
    color: Rgba,
) {
    match icon {
        QuickAccessIcon::Home => icons::home(canvas, x, y, size, color),
        QuickAccessIcon::Desktop => icons::desktop(canvas, x, y, size, color),
        QuickAccessIcon::Documents => icons::documents(canvas, x, y, size, color),
        QuickAccessIcon::Downloads => icons::downloads(canvas, x, y, size, color),
        QuickAccessIcon::Pictures => icons::pictures(canvas, x, y, size, color),
        QuickAccessIcon::Music => icons::music(canvas, x, y, size, color),
        QuickAccessIcon::Videos => icons::videos(canvas, x, y, size, color),
        QuickAccessIcon::Folder => icons::folder_outline(canvas, x, y, size, color),
    }
}

pub(super) fn draw_mount_icon(
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    size: f32,
    icon: MountIcon,
    color: Rgba,
) {
    match icon {
        MountIcon::UsbDrive => icons::usb_drive(canvas, x, y, size, color),
        MountIcon::ExternalHdd | MountIcon::Generic => icons::hard_drive(canvas, x, y, size, color),
        MountIcon::Optical => icons::optical(canvas, x, y, size, color),
    }
}

/// Blends `a` towards `b`, with `t` in 0.0..=1.0.
fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgba::new(lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b), a.a)
}

/// Folder icons sit between the accent and the muted text tone, so a directory
/// listing does not read as a column of saturated blue.
pub(super) fn folder_tint(colors: &Colors) -> Rgba {
    mix(colors.accent, colors.text_muted, 0.3)
}

/// Muted tint for a file row icon, keyed on the file extension.
pub(super) fn file_icon_color(name: &str, colors: &Colors) -> Rgba {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "rs" => rgb(196, 120, 84),
        "py" => rgb(96, 140, 180),
        "js" | "ts" | "json" => rgb(196, 178, 96),
        "html" | "htm" | "xml" => rgb(190, 108, 84),
        "css" | "scss" => rgb(104, 132, 190),
        "toml" | "yaml" | "yml" | "ini" | "conf" => rgb(150, 150, 158),
        "md" | "txt" | "log" => rgb(168, 172, 176),
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => rgb(120, 172, 128),
        "zip" | "gz" | "xz" | "zst" | "tar" | "7z" => rgb(180, 150, 110),
        _ => colors.text_muted,
    }
}
