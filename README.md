# rust-colorpicker

A small, framework-independent Rust package for native color selection.

The package is intentionally split into two layers:

- **Core API** — `Color` plus a stateful `ColorPicker` with no GUI-framework dependency.
- **Native Win32 field** — `field::ColorField`, a real child HWND for pure Win32 applications.

On Windows, `ColorPicker` calls the operating system's `ChooseColorW` common dialog rather than drawing a custom picker. This keeps behavior, keyboard handling, DPI behavior and system styling under Windows control.

## Why the field is separate

Putting an egui, iced, Slint, Tauri, or other framework widget in the core crate would couple every consumer to that framework. The standard approach is to keep the picker framework-independent and let framework-based applications render their own color swatch/button.

For pure Win32 applications, the crate includes a native `ColorField` because it is still framework-independent at the Rust level and is useful as a reusable HWND control.

## Install

```toml
[dependencies]
rust-colorpicker = { git = "https://github.com/emadgh/rust-colorpicker" }
```

The crate currently uses `windows-sys >=0.61,<0.62` only when compiling for Windows.

## Basic picker

```rust
use rust_colorpicker::{Color, ColorPicker};

fn choose() -> Result<(), Box<dyn std::error::Error>> {
    let mut picker = ColorPicker::new();
    let current = Color::rgb(30, 120, 220);

    if let Some(selected) = picker.pick(current)? {
        println!("selected: {selected}");
    }

    Ok(())
}
```

Keep the same `ColorPicker` instance when you want Windows' 16 custom swatches to persist between opens.

## Picker with a Win32 owner HWND

Passing the application's real top-level HWND is recommended because Windows can then handle modality, focus restoration and taskbar behavior correctly.

```rust
use rust_colorpicker::{Color, ColorPicker, NativeWindowHandle};

fn choose_for_window(
    hwnd: NativeWindowHandle,
    current: Color,
) -> Result<Option<Color>, rust_colorpicker::PickerError> {
    let mut picker = ColorPicker::new();
    picker.pick_with_owner(hwnd, current)
}
```

## Native Win32 color field

`ColorField` displays the selected color and automatically opens the native picker on click, Enter, or Space.

```rust
use rust_colorpicker::{
    Color,
    field::{ColorField, ColorFieldBounds},
};

const COLOR_ID: u16 = 1001;

# unsafe fn create(parent: rust_colorpicker::NativeWindowHandle) -> Result<(), Box<dyn std::error::Error>> {
let field = unsafe {
    ColorField::create(
        parent,
        COLOR_ID,
        ColorFieldBounds::new(20, 20, 80, 28),
        Color::rgb(40, 140, 220),
    )?
};
# let _ = field;
# Ok(())
# }
```

After the user accepts a different color, the control sends `WM_COMMAND` to its parent. The low word is the control ID and the high word is `field::COLOR_FIELD_CHANGED`. The parent can then read `field.color()`.

The field owns its child HWND and destroys it on `Drop`. It is intentionally `!Send` and `!Sync`, matching Win32 UI-thread rules.

## Framework-based applications

For `eframe/egui`, iced, Slint, Tauri and similar applications, render the swatch/button using that framework and call `ColorPicker::pick(...)` when it is clicked. This avoids embedding a native child HWND inside a framework surface and keeps the integration idiomatic.

## Color format

`Color` is 8-bit sRGB RGBA and supports:

- `Color::rgb(...)` / `Color::rgba(...)`
- `#RGB`
- `#RGBA`
- `#RRGGBB`
- `#RRGGBBAA`
- Win32 `COLORREF` conversion

The classic Windows `ChooseColorW` dialog selects RGB only. If the input color has alpha, this crate preserves that alpha in the returned color; the native dialog does not edit it.

## Platform behavior

Version 0.1 is Windows-native first. The core crate still compiles on non-Windows targets, where opening a picker returns `PickerError::UnsupportedPlatform` rather than silently displaying a non-native substitute.

This API shape leaves room for future native backends such as macOS without breaking consumers. Linux intentionally needs a desktop/toolkit-specific decision because there is no single universal OS color-picker API equivalent to Win32 `ChooseColorW`.

## License

MIT
