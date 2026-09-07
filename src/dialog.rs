use core::fmt;

use crate::Color;

/// Stateful custom native color picker.
///
/// On Windows this opens the crate's own Win32/GDI popup rather than the
/// operating system's legacy `ChooseColorW` dialog.
#[derive(Clone, Debug)]
pub struct ColorPicker {
    show_alpha: bool,
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self { show_alpha: true }
    }
}

impl ColorPicker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Controls whether the picker exposes an alpha slider and `#RRGGBBAA`
    /// hex values. Defaults to `true`.
    pub fn set_show_alpha(&mut self, show_alpha: bool) {
        self.show_alpha = show_alpha;
    }

    #[must_use]
    pub const fn show_alpha(&self) -> bool {
        self.show_alpha
    }

    /// Opens the custom native picker without an owner window.
    ///
    /// Returns `Ok(None)` when the user cancels.
    pub fn pick(&mut self, initial: Color) -> Result<Option<Color>, PickerError> {
        #[cfg(windows)]
        {
            crate::win32_picker::show(core::ptr::null_mut(), initial, self.show_alpha)
                .map_err(PickerError::Windows)
        }

        #[cfg(not(windows))]
        {
            let _ = initial;
            Err(PickerError::UnsupportedPlatform)
        }
    }

    /// Opens the custom picker owned by `owner`.
    ///
    /// Passing the application's top-level HWND gives correct modality and
    /// focus restoration while the popup is open.
    #[cfg(windows)]
    pub fn pick_with_owner(
        &mut self,
        owner: windows_sys::Win32::Foundation::HWND,
        initial: Color,
    ) -> Result<Option<Color>, PickerError> {
        crate::win32_picker::show(owner, initial, self.show_alpha).map_err(PickerError::Windows)
    }
}

/// One-shot convenience API.
pub fn pick_color(initial: Color) -> Result<Option<Color>, PickerError> {
    ColorPicker::default().pick(initial)
}

/// One-shot Win32 convenience API with a parent HWND.
#[cfg(windows)]
pub fn pick_color_with_owner(
    owner: windows_sys::Win32::Foundation::HWND,
    initial: Color,
) -> Result<Option<Color>, PickerError> {
    ColorPicker::default().pick_with_owner(owner, initial)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerError {
    UnsupportedPlatform,
    Windows(u32),
}

impl fmt::Display for PickerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                f.write_str("no native color-picker backend is available on this platform")
            }
            Self::Windows(code) => write!(f, "Win32 color-picker error {code}"),
        }
    }
}

impl std::error::Error for PickerError {}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_is_explicit() {
        assert_eq!(
            ColorPicker::new().pick(Color::BLACK),
            Err(PickerError::UnsupportedPlatform)
        );
    }
}
