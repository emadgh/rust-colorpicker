//! Framework-independent native color picker building blocks.
//!
//! The crate intentionally separates the reusable color model and picker API
//! from GUI-framework integration. On Windows it uses the operating system's
//! Win32 common color dialog. Pure Win32 applications can additionally use the
//! native [`field::ColorField`] child control.

mod color;
mod dialog;

#[cfg(windows)]
pub mod field;

pub use color::{Color, ParseColorError};
pub use dialog::{ColorPicker, PickerError, pick_color};

#[cfg(windows)]
pub use dialog::pick_color_with_owner;

/// Native parent-window handle accepted by the Windows APIs in this crate.
#[cfg(windows)]
pub type NativeWindowHandle = windows_sys::Win32::Foundation::HWND;
