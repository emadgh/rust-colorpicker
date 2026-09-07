use rust_colorpicker::{Color, ColorPicker, PickerError};

fn main() -> Result<(), PickerError> {
    let mut picker = ColorPicker::new();
    let current = Color::rgb(32, 128, 224);

    match picker.pick(current)? {
        Some(selected) => println!("selected {selected}"),
        None => println!("selection cancelled"),
    }

    Ok(())
}
