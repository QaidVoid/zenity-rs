//! Physical geometry of the file selection dialog.
//!
//! Everything here is derived once from the window size and the compositor
//! scale, then handed to the drawing and hit-testing code so the two cannot
//! disagree about where anything sits.

use super::{SidebarRow, ToolbarButton};
use crate::ui::{BASE_MIN_THUMB, Thumb};

pub(super) struct Layout {
    pub(super) scale: f32,
    pub(super) window_width: u32,
    pub(super) padding: u32,
    pub(super) sidebar_x: i32,
    pub(super) sidebar_width: u32,
    pub(super) main_x: i32,
    pub(super) main_y: i32,
    pub(super) main_w: u32,
    pub(super) main_h: u32,
    pub(super) toolbar_height: u32,
    pub(super) path_bar_height: u32,
    pub(super) section_header_height: u32,
    pub(super) content_gap: u32,
    pub(super) header_offset: u32,
    pub(super) item_height: u32,
    /// Rows that fit in the file pane at once.
    pub(super) visible_items: usize,
    pub(super) list_y: i32,
    pub(super) list_h: u32,
    pub(super) size_col_x: i32,
    /// Zero in directory mode, where no size is shown.
    pub(super) size_col_width: u32,
    pub(super) date_col_x: i32,
    pub(super) search_x: i32,
    pub(super) search_y: i32,
    pub(super) button_y: i32,
    pub(super) filename_y: i32,
    /// Width reserved on the right of the file pane for the scrollbar.
    pub(super) scrollbar_gutter: u32,
    pub(super) toolbar_buttons: Vec<ToolbarButton>,
    pub(super) sidebar_rows: Vec<(SidebarRow, i32)>,
}

impl Layout {
    /// Scrollbar thumb rect (x, y, w, h) for a list of `total` entries scrolled
    /// to `scroll`, shared by drawing, hover, hit-testing and drag.
    ///
    /// `None` when every entry already fits and no bar should be drawn.
    pub(super) fn scrollbar_thumb(
        &self,
        total: usize,
        scroll: usize,
        hovered: bool,
    ) -> Option<(f32, f32, f32, f32)> {
        let thumb = Thumb::new(
            self.list_h as f32,
            self.visible_items as f32,
            total as f32,
            scroll as f32,
            BASE_MIN_THUMB * self.scale,
        )?;
        let w = if hovered {
            8.0 * self.scale
        } else {
            5.0 * self.scale
        };
        let x =
            self.main_x as f32 + self.main_w as f32 - self.scrollbar_gutter as f32 / 2.0 - w / 2.0;
        Some((x, self.list_y as f32 + thumb.offset, w, thumb.len))
    }

    /// The sidebar row under a point, ignoring section headers.
    pub(super) fn sidebar_row_at(&self, mx: i32, my: i32) -> Option<SidebarRow> {
        if mx < self.sidebar_x || mx >= self.sidebar_x + self.sidebar_width as i32 {
            return None;
        }
        self.sidebar_rows
            .iter()
            .find(|(row, y)| {
                !matches!(row, SidebarRow::Header(_))
                    && my >= *y
                    && my < *y + self.item_height as i32
            })
            .map(|(row, _)| *row)
    }
}
