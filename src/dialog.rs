use core::fmt;

use crate::Color;

/// Stateful native color picker.
///
/// Keeping an instance alive preserves the 16 custom color swatches between
/// invocations, matching the Win32 common-dialog contract.
#[derive(Clone, Debug)]
pub struct ColorPicker {
    custom_colors: [Color; 16],
    full_open: bool,
    any_color: bool,
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self {
            custom_colors: [Color::BLACK; 16],
            full_open: true,
            any_color: true,
        }
    }
}

impl ColorPicker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn custom_colors(&self) -> &[Color; 16] {
        &self.custom_colors
    }

    pub fn set_custom_colors(&mut self, colors: [Color; 16]) {
        self.custom_colors = colors;
    }

    /// Controls whether the native dialog opens with the custom-color controls
    /// already expanded. Defaults to `true`.
    pub fn set_full_open(&mut self, full_open: bool) {
        self.full_open = full_open;
    }

    #[must_use]
    pub const fn full_open(&self) -> bool {
        self.full_open
    }

    /// Controls the Win32 `CC_ANYCOLOR` behavior. Defaults to `true`.
    pub fn set_any_color(&mut self, any_color: bool) {
        self.any_color = any_color;
    }

    #[must_use]
    pub const fn any_color(&self) -> bool {
        self.any_color
    }

    /// Opens the operating-system color picker without an owner window.
    ///
    /// Returns `Ok(None)` when the user cancels. On Windows the native common
    /// color dialog edits RGB; `initial.a` is preserved in the returned color.
    pub fn pick(&mut self, initial: Color) -> Result<Option<Color>, PickerError> {
        #[cfg(windows)]
        {
            return self.pick_with_owner(core::ptr::null_mut(), initial);
        }

        #[cfg(not(windows))]
        {
            let _ = initial;
            Err(PickerError::UnsupportedPlatform)
        }
    }

    /// Opens the native Win32 color dialog owned by `owner`.
    ///
    /// A null owner is allowed by Win32. Passing a real top-level HWND is
    /// preferred so modality, focus restoration and taskbar behavior are
    /// correct for the host application.
    #[cfg(windows)]
    pub fn pick_with_owner(
        &mut self,
        owner: windows_sys::Win32::Foundation::HWND,
        initial: Color,
    ) -> Result<Option<Color>, PickerError> {
        use core::mem::{size_of, zeroed};
        use windows_sys::Win32::UI::Controls::Dialogs::{
            CC_ANYCOLOR, CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW,
            CommDlgExtendedError,
        };

        let mut native_custom = self.custom_colors.map(Color::to_colorref);
        let mut dialog: CHOOSECOLORW = unsafe { zeroed() };
        dialog.lStructSize = size_of::<CHOOSECOLORW>() as u32;
        dialog.hwndOwner = owner;
        dialog.rgbResult = initial.to_colorref();
        dialog.lpCustColors = native_custom.as_mut_ptr();
        dialog.Flags = CC_RGBINIT
            | if self.full_open { CC_FULLOPEN } else { 0 }
            | if self.any_color { CC_ANYCOLOR } else { 0 };

        let accepted = unsafe { ChooseColorW(&mut dialog) } != 0;

        self.custom_colors = native_custom.map(Color::from_colorref);

        if accepted {
            Ok(Some(Color::from_colorref_with_alpha(
                dialog.rgbResult,
                initial.a,
            )))
        } else {
            let error = unsafe { CommDlgExtendedError() };
            if error == 0 {
                Ok(None)
            } else {
                Err(PickerError::CommonDialog(error))
            }
        }
    }
}

/// One-shot convenience API.
///
/// Use [`ColorPicker`] directly when custom swatches should persist between
/// calls.
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
    CommonDialog(u32),
}

impl fmt::Display for PickerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                f.write_str("no native color-picker backend is available on this platform")
            }
            Self::CommonDialog(code) => write!(f, "Win32 common-dialog error 0x{code:08X}"),
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
