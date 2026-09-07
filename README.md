# rust-colorpicker

A lightweight custom native color picker for Rust applications on Windows.

This crate does **not** wrap the legacy Windows `ChooseColorW` dialog. The picker UI is implemented by the crate itself with Win32/GDI and native Windows input controls, with no egui, iced, Slint, Qt, WebView, or other GUI framework dependency.

## Picker UI

The Windows picker provides:

- large 2D Saturation / Value surface
- vertical Hue strip
- optional Alpha strip
- draggable selection indicators
- editable `#RRGGBB` / `#RRGGBBAA` Hex field
- old-color and new-color previews
- explicit Confirm and Cancel buttons
- Enter to confirm and Escape to cancel
- native-caption or fully borderless popup modes
- DPI / application-scale input
- configurable host palette
- configurable font family and sizes
- English/Persian labels and RTL support
- realtime preview events with transactional rollback on cancel
- owner-window modality, topmost inheritance, and monitor work-area clamping
- double-buffered GDI rendering and cached gradients

## Architecture

The package is split into reusable layers:

- **Color core** — `Color`, hex parsing/formatting, RGBA handling, and HSV conversion internals.
- **Picker API** — framework-independent `ColorPicker` configuration and result API.
- **Appearance API** — `PickerTheme`, `PickerFont`, `PickerLabels`, and `PickerChrome`.
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

Alpha editing is enabled by default. For an RGB-only host:

```rust
let mut picker = rust_colorpicker::ColorPicker::new();
picker.set_show_alpha(false);
```

When alpha editing is disabled, the alpha row is removed entirely and the incoming alpha value is preserved.

## Match the host application

The picker accepts the effective DPI/scale, colors, fonts, labels, direction, and window chrome from the host application.

```rust
use rust_colorpicker::{
    ColorPicker, PickerChrome, PickerFont, PickerLabels, PickerTheme,
};

let mut picker = ColorPicker::new();
picker.set_show_alpha(false);
picker.set_scale_percent(110); // alternatively: picker.set_dpi(106)
picker.set_chrome(PickerChrome::Borderless);
picker.set_theme(PickerTheme::dark());
picker.set_font(PickerFont::new("Vazirmatn", 15, 16));
picker.set_labels(PickerLabels::persian());
picker.set_rtl(true);
picker.set_corner_radius(10);
```

`PickerTheme` is intentionally a plain public struct. Applications with their own theme system should map their active palette into it rather than forcing one of the built-in light/dark presets.

```rust
use rust_colorpicker::{Color, PickerTheme};

let theme = PickerTheme {
    background: Color::rgb(30, 31, 33),
    surface: Color::rgb(43, 45, 48),
    surface_alt: Color::rgb(53, 56, 60),
    text: Color::rgb(240, 241, 243),
    muted: Color::rgb(176, 183, 192),
    border: Color::rgb(70, 70, 70),
    accent: Color::rgb(248, 211, 88),
    accent_text: Color::rgb(24, 24, 24),
    checker_light: Color::rgb(70, 72, 76),
    checker_dark: Color::rgb(52, 54, 58),
};
```

## Realtime / transactional mode

`pick_live` and `pick_with_owner_live` emit changes while the user drags or edits Hex.

```rust
use rust_colorpicker::{Color, ColorPicker, PickerEvent};

# fn repaint_with(_: Color) {}
# fn persist(_: Color) {}
# fn example(hwnd: rust_colorpicker::NativeWindowHandle) -> Result<(), rust_colorpicker::PickerError> {
let original = Color::rgb(30, 120, 220);
let mut picker = ColorPicker::new();
picker.set_show_alpha(false);

let selected = picker.pick_with_owner_live(hwnd, original, |event| {
    match event {
        PickerEvent::Preview(color) => {
            // Apply to in-memory UI only. Do not persist yet.
            repaint_with(color);
        }
        PickerEvent::Accepted(color) => {
            repaint_with(color);
        }
        PickerEvent::Cancelled(original) => {
            // Escape, Cancel, close-button, or any non-confirmed close rolls back.
            repaint_with(original);
        }
    }
})?;

if let Some(color) = selected {
    persist(color);
}
# Ok(())
# }
```

Realtime mode is transactional by design:

- `Preview(color)` — current tentative selection
- `Accepted(color)` — explicit confirmation
- `Cancelled(original)` — original input color, emitted for every non-confirmed exit

This lets a host update its UI immediately without accidentally persisting an unconfirmed color.

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

If the owner is topmost, the picker inherits that behavior. The picker is also clamped to the owner's monitor work area, including monitors with negative desktop coordinates.

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

## Windows demo artifact

CI builds `examples/picker.rs` as a release-mode Windows executable and uploads it as the `rust-colorpicker-demo-windows` workflow artifact. This is intended for visual and interaction testing without publishing a release.

## Platform behavior

Version 0.3 is Windows-native first. The core crate still compiles on non-Windows targets, where opening a picker returns `PickerError::UnsupportedPlatform`.

The public API remains framework-independent so native backends for other operating systems can be added later without coupling consumers to a GUI toolkit.

## License

MIT
