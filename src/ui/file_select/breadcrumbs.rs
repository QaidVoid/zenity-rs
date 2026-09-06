//! Breadcrumb trail across the top of the file selection dialog.

use std::path::{Path, PathBuf};

use crate::{
    render::{Canvas, Font, rgb},
    ui::{Colors, icons},
};

/// One clickable breadcrumb segment.
pub(super) struct Crumb {
    pub(super) path: PathBuf,
    pub(super) x: i32,
    pub(super) w: i32,
}

/// Computes the breadcrumb layout for hit-testing.
///
/// Mirrors the rendering logic in [`draw_breadcrumbs`]: when the full path does not
/// fit, leading components are collapsed into an ellipsis and only the trailing
/// components remain. Returns each visible named segment with the accumulated
/// filesystem path it navigates to, its left edge (absolute x), and text width.
/// The collapsed ellipsis marker is intentionally excluded (it is not a target).
pub(super) fn breadcrumb_layout(path: &Path, x: i32, max_w: i32, font: &Font) -> Vec<Crumb> {
    let components: Vec<_> = path.components().collect();
    let num = components.len();
    if num == 0 {
        return Vec::new();
    }

    let measure = |s: &str| font.render(s).with_color(rgb(0, 0, 0)).finish().width() as i32;
    let sep_w = measure(" \u{203a} ");
    let ellipsis_w = measure("...") + 8;

    let mut widths = Vec::with_capacity(num);
    let mut accs: Vec<PathBuf> = Vec::with_capacity(num);
    let mut acc = PathBuf::new();
    let mut total = 0i32;
    for (i, comp) in components.iter().enumerate() {
        acc.push(comp);
        accs.push(acc.clone());
        let raw = comp.as_os_str().to_string_lossy();
        let w = measure(if raw.is_empty() { "/" } else { raw.as_ref() });
        widths.push(w);
        total += w;
        if i < num - 1 && !matches!(comp, std::path::Component::RootDir) {
            total += sep_w;
        }
    }

    let show = if total > max_w {
        (1..=num.min(4))
            .rev()
            .find(|&n| {
                let start = num - n;
                let mut t = if start > 0 { ellipsis_w } else { 0 };
                for (i, w) in widths.iter().enumerate().skip(start) {
                    t += w;
                    if i < num - 1 && !matches!(components[i], std::path::Component::RootDir) {
                        t += sep_w;
                    }
                }
                t <= max_w
            })
            .unwrap_or(1)
    } else {
        num
    };
    let start = num - show;

    let mut cx = x + if start > 0 { ellipsis_w } else { 0 };
    let mut out = Vec::with_capacity(show);
    for i in start..num {
        out.push(Crumb {
            path: accs[i].clone(),
            x: cx,
            w: widths[i],
        });
        cx += widths[i];
        if i < num - 1 && !matches!(components[i], std::path::Component::RootDir) {
            cx += sep_w;
        }
    }
    out
}

/// Draws the clickable path segments, with the current folder emphasized.
///
/// `baseline` is where the segment text sits; the chevrons are centered on it.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_breadcrumbs(
    canvas: &mut Canvas,
    x: i32,
    baseline: i32,
    max_w: u32,
    path: &Path,
    colors: &Colors,
    font: &Font,
    scale: f32,
) {
    let crumbs = breadcrumb_layout(path, x, max_w as i32, font);
    if crumbs.is_empty() {
        return;
    }

    let components: Vec<_> = path.components().collect();
    let shown = crumbs.len();
    let last = shown - 1;
    let chevron = 10.0 * scale;

    if shown < components.len() {
        let (ellipsis, base) = font
            .render("\u{2026}")
            .with_color(colors.text_muted)
            .finish_with_baseline();
        canvas.draw_canvas(&ellipsis, x, baseline - base);
    }

    for (i, crumb) in crumbs.iter().enumerate() {
        let component = &components[components.len() - shown + i];
        let name = component.as_os_str().to_string_lossy();
        let display = if name.is_empty() { "/" } else { &name };
        let color = if i == last {
            colors.text
        } else {
            colors.text_muted
        };
        let (text, base) = font
            .render(display)
            .with_color(color)
            .finish_with_baseline();
        canvas.draw_canvas(&text, crumb.x, baseline - base);

        if i != last && !matches!(component, std::path::Component::RootDir) {
            // Center the chevron in the gap the layout left between the segments
            let gap_start = crumb.x + text.width() as i32;
            let gap_end = crumbs[i + 1].x;
            icons::chevron_right(
                canvas,
                (gap_start + gap_end) as f32 / 2.0 - chevron / 2.0,
                baseline as f32 - font.ascent() * 0.32 - chevron / 2.0,
                chevron,
                colors.text_muted,
            );
        }
    }
}
