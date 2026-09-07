#![cfg_attr(windows, windows_subsystem = "windows")]

use rust_colorpicker::{Color, ColorPicker, PickerError};

fn main() -> Result<(), PickerError> {
    let mut picker = ColorPicker::new();
    let current = Color::rgb(171, 206, 179);
    let _ = picker.pick(current)?;
    Ok(())
}
