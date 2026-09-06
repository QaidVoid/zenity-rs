//! Drawing for the file selection dialog.
//!
//! Split into a chrome layer that only changes on navigation or hover, and a
//! dynamic layer redrawn every frame on top of it.

use super::{
    BASE_ICON_SIZE, BASE_ROW_RADIUS, MountPoint, QuickAccess, SidebarGlyph, SidebarRow,
    ToolbarAction,
    breadcrumbs::draw_breadcrumbs,
    draw::{baseline_in, draw_mount_icon, draw_place_icon, file_icon_color, folder_tint},
    fs::{Browser, format_date, format_size, get_mount_icon},
    layout::Layout,
};
use crate::{
    render::{Canvas, Font},
    ui::{
        BASE_CORNER_RADIUS, Colors, ellipsize, icons,
        widgets::{Widget, button::Button, text_input::TextInput},
    },
};

/// Background, toolbar, sidebar, path bar and column headers.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_chrome(
    canvas: &mut Canvas,
    colors: &Colors,
    font: &Font,
    layout: &Layout,
    browser: &Browser,
    quick_access: &[QuickAccess],
    mounted_drives: &[MountPoint],
    hovered_sidebar: Option<SidebarRow>,
    hovered_toolbar: Option<ToolbarAction>,
    search_input: &TextInput,
) {
    let width = canvas.width() as f32;
    let height = canvas.height() as f32;
    let radius = BASE_CORNER_RADIUS * layout.scale;
    let pane_radius = 8.0 * layout.scale;
    let row_radius = BASE_ROW_RADIUS * layout.scale;
    let toolbar_icon = 17.0 * layout.scale;
    let sidebar_icon = 16.0 * layout.scale;

    canvas.fill_dialog_bg(
        width,
        height,
        colors.window_bg,
        colors.window_border,
        colors.window_shadow,
        radius,
    );

    // ===== TOOLBAR =====
    for button in &layout.toolbar_buttons {
        let enabled = match button.action {
            ToolbarAction::Back => browser.can_go_back(),
            ToolbarAction::Forward => browser.can_go_forward(),
            ToolbarAction::Up => browser.current_dir.parent().is_some(),
            ToolbarAction::Home | ToolbarAction::ToggleHidden => true,
        };
        let active = button.action == ToolbarAction::ToggleHidden && browser.show_hidden;

        if active {
            canvas.fill_rounded_rect(
                button.x as f32,
                button.y as f32,
                button.size as f32,
                button.size as f32,
                row_radius,
                colors.accent,
            );
        } else if enabled && hovered_toolbar == Some(button.action) {
            canvas.fill_rounded_rect(
                button.x as f32,
                button.y as f32,
                button.size as f32,
                button.size as f32,
                row_radius,
                colors.row_hover,
            );
        }

        let tint = if active {
            colors.accent_text
        } else if enabled {
            colors.text
        } else {
            colors.text_muted.with_alpha(110)
        };
        let ix = button.x as f32 + (button.size as f32 - toolbar_icon) / 2.0;
        let iy = button.y as f32 + (button.size as f32 - toolbar_icon) / 2.0;
        match button.action {
            ToolbarAction::Back => icons::chevron_left(canvas, ix, iy, toolbar_icon, tint),
            ToolbarAction::Forward => icons::chevron_right(canvas, ix, iy, toolbar_icon, tint),
            ToolbarAction::Up => icons::arrow_up(canvas, ix, iy, toolbar_icon, tint),
            ToolbarAction::Home => icons::home(canvas, ix, iy, toolbar_icon, tint),
            ToolbarAction::ToggleHidden => {
                if browser.show_hidden {
                    icons::eye(canvas, ix, iy, toolbar_icon, tint)
                } else {
                    icons::eye_off(canvas, ix, iy, toolbar_icon, tint)
                }
            }
        }
    }

    search_input.draw_to(canvas, colors, font);
    icons::search(
        canvas,
        (layout.search_x + search_input.width() as i32) as f32 - 26.0 * layout.scale,
        layout.search_y as f32 + (search_input.height() as f32 - 15.0 * layout.scale) / 2.0,
        15.0 * layout.scale,
        colors.text_muted,
    );

    canvas.fill_rect(
        1.0,
        (layout.padding + layout.toolbar_height) as f32 + layout.content_gap as f32 / 2.0,
        width - 2.0,
        1.0,
        colors.separator,
    );

    // ===== SIDEBAR =====
    for (row, y) in &layout.sidebar_rows {
        let y = *y;
        let (icon, label, is_current) = match row {
            SidebarRow::Header(label) => {
                let (text, base) = font
                    .render(label)
                    .with_color(colors.text_muted)
                    .finish_with_baseline();
                canvas.draw_canvas(
                    &text,
                    layout.sidebar_x + (8.0 * layout.scale) as i32,
                    baseline_in(y, layout.section_header_height, font) - base,
                );
                continue;
            }
            SidebarRow::Place(i) => {
                let qa = &quick_access[*i];
                (
                    SidebarGlyph::Place(qa.icon),
                    qa.name.clone(),
                    qa.path == browser.current_dir,
                )
            }
            SidebarRow::Drive(i) => {
                let drive = &mounted_drives[*i];
                let label = drive.label.clone().unwrap_or_else(|| {
                    drive
                        .mount_point
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&drive.device)
                        .to_string()
                });
                (
                    SidebarGlyph::Mount(get_mount_icon(&drive.device)),
                    label,
                    drive.mount_point == browser.current_dir,
                )
            }
        };

        let fill = if is_current {
            Some(colors.row_selected)
        } else if hovered_sidebar == Some(*row) {
            Some(colors.row_hover)
        } else {
            None
        };
        if let Some(fill) = fill {
            canvas.fill_rounded_rect(
                layout.sidebar_x as f32,
                y as f32,
                layout.sidebar_width as f32,
                layout.item_height as f32,
                row_radius,
                fill,
            );
        }

        let icon_tint = if is_current {
            colors.accent
        } else {
            colors.text_muted
        };
        let icon_x = layout.sidebar_x as f32 + 10.0 * layout.scale;
        let icon_y = y as f32 + (layout.item_height as f32 - sidebar_icon) / 2.0;
        match icon {
            SidebarGlyph::Place(kind) => {
                draw_place_icon(canvas, icon_x, icon_y, sidebar_icon, kind, icon_tint)
            }
            SidebarGlyph::Mount(kind) => {
                draw_mount_icon(canvas, icon_x, icon_y, sidebar_icon, kind, icon_tint)
            }
        }

        let text_x = layout.sidebar_x + (34.0 * layout.scale) as i32;
        let avail =
            (layout.sidebar_x + layout.sidebar_width as i32 - text_x - (8.0 * layout.scale) as i32)
                .max(0);
        let label = ellipsize(&label, font, avail as f32);
        let (text, base) = font
            .render(&label)
            .with_color(colors.text)
            .finish_with_baseline();
        canvas.draw_canvas(
            &text,
            text_x,
            baseline_in(y, layout.item_height, font) - base,
        );
    }

    // ===== FILE PANE =====
    canvas.fill_rounded_rect(
        layout.main_x as f32,
        layout.main_y as f32,
        layout.main_w as f32,
        layout.main_h as f32,
        pane_radius,
        colors.surface_alt,
    );

    draw_breadcrumbs(
        canvas,
        layout.main_x + (12.0 * layout.scale) as i32,
        baseline_in(layout.main_y, layout.path_bar_height, font),
        layout.main_w - (24.0 * layout.scale) as u32,
        &browser.current_dir,
        colors,
        font,
        layout.scale,
    );

    let header_y = layout.main_y + layout.path_bar_height as i32;
    canvas.fill_rect(
        layout.main_x as f32,
        header_y as f32,
        layout.main_w as f32,
        1.0,
        colors.separator,
    );

    let header_baseline = baseline_in(header_y, layout.header_offset, font);
    let mut header_label = |text: &str, x: i32, right_align: bool| {
        let (text, base) = font
            .render(text)
            .with_color(colors.text_muted)
            .finish_with_baseline();
        let x = if right_align {
            x - text.width() as i32
        } else {
            x
        };
        canvas.draw_canvas(&text, x, header_baseline - base);
    };
    header_label("Name", layout.main_x + (38.0 * layout.scale) as i32, false);
    if layout.size_col_width > 0 {
        header_label(
            "Size",
            layout.size_col_x + layout.size_col_width as i32,
            true,
        );
    }
    header_label("Modified", layout.date_col_x, false);

    canvas.fill_rect(
        layout.main_x as f32,
        (header_y + layout.header_offset as i32 - 1) as f32,
        layout.main_w as f32,
        1.0,
        colors.separator,
    );
}

/// The scrollable file list, its scrollbar, the inputs and the buttons.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_dynamic(
    canvas: &mut Canvas,
    colors: &Colors,
    font: &Font,
    layout: &Layout,
    browser: &Browser,
    title: &str,
    hovered_entry: Option<usize>,
    scrollbar_hovered: bool,
    hovered_toolbar: Option<ToolbarAction>,
    ok_button: &Button,
    cancel_button: &Button,
    filename_input: Option<&TextInput>,
) {
    let row_radius = BASE_ROW_RADIUS * layout.scale;
    let icon_size = BASE_ICON_SIZE as f32 * layout.scale;
    let row_inset = (3.0 * layout.scale).max(1.0);

    for (vi, &ei) in browser
        .filtered_entries
        .iter()
        .skip(browser.scroll_offset)
        .take(layout.visible_items)
        .enumerate()
    {
        let entry = &browser.all_entries[ei];
        let y = layout.list_y + (vi as u32 * layout.item_height) as i32;
        let is_selected = browser.selected_indices.contains(&ei);
        let is_hovered = hovered_entry == Some(ei);

        if is_selected || is_hovered {
            canvas.fill_rounded_rect(
                layout.main_x as f32 + row_inset,
                y as f32,
                layout.main_w as f32 - row_inset * 2.0,
                layout.item_height as f32,
                row_radius,
                if is_selected {
                    colors.row_selected
                } else {
                    colors.row_hover
                },
            );
        }

        let icon_x = layout.main_x as f32 + 10.0 * layout.scale;
        let icon_y = y as f32 + (layout.item_height as f32 - icon_size) / 2.0;
        if entry.is_dir {
            icons::folder(canvas, icon_x, icon_y, icon_size, folder_tint(colors));
        } else {
            icons::document(
                canvas,
                icon_x,
                icon_y,
                icon_size,
                file_icon_color(&entry.name, colors),
            );
        }

        let name_x = layout.main_x + (38.0 * layout.scale) as i32;
        let name_w = (layout.size_col_x - name_x - (12.0 * layout.scale) as i32).max(0) as f32;
        let baseline = baseline_in(y, layout.item_height, font);
        let name = ellipsize(&entry.name, font, name_w);
        let (name_canvas, base) = font
            .render(&name)
            .with_color(colors.text)
            .finish_with_baseline();
        canvas.draw_canvas(&name_canvas, name_x, baseline - base);

        if !entry.is_dir {
            let (size_canvas, base) = font
                .render(&format_size(entry.size))
                .with_color(colors.text_muted)
                .finish_with_baseline();
            canvas.draw_canvas(
                &size_canvas,
                layout.size_col_x + layout.size_col_width as i32 - size_canvas.width() as i32,
                baseline - base,
            );
        }

        let (date_canvas, base) = font
            .render(&format_date(entry.modified))
            .with_color(colors.text_muted)
            .finish_with_baseline();
        canvas.draw_canvas(&date_canvas, layout.date_col_x, baseline - base);
    }

    // Scrollbar
    if let Some((x, y, w, h)) = layout.scrollbar_thumb(
        browser.filtered_entries.len(),
        browser.scroll_offset,
        scrollbar_hovered,
    ) {
        canvas.fill_rounded_rect(
            x,
            y,
            w,
            h,
            w / 2.0,
            if scrollbar_hovered {
                colors.accent
            } else {
                colors.text_muted.with_alpha(130)
            },
        );
    }

    canvas.stroke_rounded_rect(
        layout.main_x as f32,
        layout.main_y as f32,
        layout.main_w as f32,
        layout.main_h as f32,
        8.0 * layout.scale,
        colors.separator,
        1.0,
    );

    // Filename input (save mode): label above, input below
    if let Some(fi) = filename_input {
        let label_canvas = font.render(title).with_color(colors.text_muted).finish();
        canvas.draw_canvas(
            &label_canvas,
            layout.main_x,
            layout.filename_y + (2.0 * layout.scale) as i32,
        );
        fi.draw_to(canvas, colors, font);
    }

    ok_button.draw_to(canvas, colors, font);
    cancel_button.draw_to(canvas, colors, font);

    let status = format!(
        "{} item{}",
        browser.filtered_entries.len(),
        if browser.filtered_entries.len() == 1 {
            ""
        } else {
            "s"
        }
    );
    let (status_canvas, base) = font
        .render(&status)
        .with_color(colors.text_muted)
        .finish_with_baseline();
    canvas.draw_canvas(
        &status_canvas,
        layout.padding as i32,
        baseline_in(layout.button_y, ok_button.height(), font) - base,
    );

    // Tooltip for the hovered toolbar button
    if let Some(button) = hovered_toolbar
        .and_then(|action| layout.toolbar_buttons.iter().find(|b| b.action == action))
    {
        let (label, base) = font
            .render(button.action.tooltip(browser.show_hidden))
            .with_color(colors.text)
            .finish_with_baseline();
        let pad_x = (8.0 * layout.scale) as i32;
        let tip_h = (24.0 * layout.scale) as u32;
        let tip_w = label.width() as i32 + pad_x * 2;
        let tip_x = (button.x + button.size as i32 / 2 - tip_w / 2).clamp(
            layout.padding as i32,
            layout.window_width as i32 - layout.padding as i32 - tip_w,
        );
        let tip_y = button.y + button.size as i32 + (6.0 * layout.scale) as i32;
        canvas.fill_rounded_rect(
            tip_x as f32,
            tip_y as f32,
            tip_w as f32,
            tip_h as f32,
            BASE_ROW_RADIUS * layout.scale,
            colors.surface,
        );
        canvas.stroke_rounded_rect(
            tip_x as f32,
            tip_y as f32,
            tip_w as f32,
            tip_h as f32,
            BASE_ROW_RADIUS * layout.scale,
            colors.separator,
            1.0,
        );
        canvas.draw_canvas(
            &label,
            tip_x + pad_x,
            baseline_in(tip_y, tip_h, font) - base,
        );
    }
}
