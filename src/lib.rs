//! Framework-independent native color picker building blocks.
//!
//! The crate provides its own custom Win32/GDI color picker on Windows. It does
//! not wrap the legacy `ChooseColorW` common dialog and does not depend on a GUI
//! framework such as egui, iced, Slint, Qt, or a WebView.

mod color;
mod dialog;

#[cfg(any(windows, test))]
mod hsv;

#[cfg(windows)]
#[allow(clippy::field_reassign_with_default)]
mod win32_picker;

#[cfg(windows)]
pub mod field;

pub use color::{Color, ParseColorError};
pub use dialog::{ColorPicker, PickerError, pick_color};

#[cfg(windows)]
pub use dialog::pick_color_with_owner;

/// Native parent-window handle accepted by the Windows APIs in this crate.
#[cfg(windows)]
pub type NativeWindowHandle = windows_sys::Win32::Foundation::HWND;
