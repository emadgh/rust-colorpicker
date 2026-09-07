# rust-colorpicker

A lightweight custom native color picker for Rust applications on Windows.

This crate does **not** wrap the legacy Windows `ChooseColorW` dialog. The picker UI is implemented by the crate itself with Win32/GDI and standard native child controls, with no egui, iced, Slint, Qt, WebView, or other GUI framework dependency.

## Picker UI

The Windows picker provides:

- large 2D Saturation / Value surface
- vertical Hue strip
- horizontal Alpha strip
- draggable selection indicators
- editable `#RRGGBB` / `#RRGGBBAA` hex field
- old-color and new-color previews
- Enter to accept and Escape to cancel
- click the old preview to restore the incoming color
- click the new preview to accept the current color
- modal owner-window behavior
- mouse drag interaction

The client layout follows the compact 356×427 reference used by the project: a large color surface, a narrow hue strip, a horizontal alpha mask, centered hex row, and before/after previews. The visible OK/Cancel button row is intentionally omitted; confirmation is handled by Enter or the new-color preview and cancellation by Escape or closing the popup.

## Architecture

The package is split into reusable layers:

- **Color core** — `Color`, hex parsing/formatting, RGBA handling, and HSV conversion internals.
- **Picker API** — framework-independent `ColorPicker` API.
- **Custom Win32 picker** — private native popup implementation rendered with Win32/GDI.
- **Native Win32 field** — `field::ColorField`, a reusable child HWND showing the current color and opening the custom picker when activated.

Framework-specific widgets are deliberately kept out of the core crate. An egui application, for example, should render its own swatch using egui and call `ColorPicker` when clicked. Pure Win32 applications can use the included `ColorField` directly.

## Install

```toml
[dependencies]
rust-colorpicker = { git = "https://github.com/emadgh/rust-colorpicker" }
```

The crate currently uses `windows-sys >=0.61,<0.62` only on Windows.

## Basic picker

```rust
use rust_colorpicker::{Color, ColorPicker};

fn choose() -> Result<(), Box<dyn std::error::Error>> {
    let mut picker = ColorPicker::new();
    let current = Color::rgba(30, 120, 220, 255);

    if let Some(selected) = picker.pick(current)? {
        println!("selected: {selected}");
    }

    Ok(())
}
```

Alpha editing is enabled by default. To expose RGB only:

```rust
let mut picker = rust_colorpicker::ColorPicker::new();
picker.set_show_alpha(false);
```

## Picker with a Win32 owner HWND

Passing the application's top-level HWND is recommended so the picker behaves modally and restores focus to the host window correctly.

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

`ColorField` is a real child HWND. It displays the selected color and opens this crate's custom picker on click, Enter, or Space.

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

The field owns its HWND and destroys it on `Drop`. It is intentionally `!Send` and `!Sync`, matching Win32 UI-thread rules.

## Color format

`Color` is 8-bit sRGB RGBA and supports:

- `Color::rgb(...)`
- `Color::rgba(...)`
- `#RGB`
- `#RGBA`
- `#RRGGBB`
- `#RRGGBBAA`
- Win32 `COLORREF` conversion

When alpha editing is enabled, the picker shows and edits `#RRGGBBAA` and exposes a horizontal alpha control. When alpha editing is disabled, the incoming alpha is preserved while RGB is edited.

## Windows demo artifact

CI builds the `examples/picker.rs` demo as a release-mode Windows executable and uploads it as the `rust-colorpicker-demo-windows` workflow artifact. This is intended for visual and interaction testing without publishing a release.

## Platform behavior

Version 0.2 is Windows-native first. The core crate still compiles on non-Windows targets, where opening a picker returns `PickerError::UnsupportedPlatform`.

The public API remains framework-independent so native backends for other operating systems can be added later without coupling consumers to a GUI toolkit.

## License

MIT
