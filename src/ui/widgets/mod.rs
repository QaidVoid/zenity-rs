//! Reusable UI widgets.

pub(crate) mod button;

use crate::backend::WindowEvent;
use crate::render::Canvas;
use crate::ui::Colors;

/// Trait for UI widgets.
pub(crate) trait Widget {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn x(&self) -> i32;
    fn y(&self) -> i32;
    fn set_position(&mut self, x: i32, y: i32);
    fn process_event(&mut self, event: &WindowEvent) -> bool;
    fn draw(&self, canvas: &mut Canvas, colors: &Colors);
}

/// Check if a point is within a rectangle.
pub(crate) fn point_in_rect(px: i32, py: i32, x: i32, y: i32, w: u32, h: u32) -> bool {
    px >= x && px < x + w as i32 && py >= y && py < y + h as i32
}
