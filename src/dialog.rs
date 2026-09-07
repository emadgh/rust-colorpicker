use core::fmt;

use crate::{Color, PickerChrome, PickerFont, PickerLabels, PickerTheme};

/// Realtime events emitted by `pick_live` / `pick_with_owner_live`.
///
/// A host can apply [`PickerEvent::Preview`] immediately to its in-memory UI.
/// If the user cancels, presses Escape, or closes the picker without
/// confirmation, [`PickerEvent::Cancelled`] carries the original color so the
/// host can restore it. [`PickerEvent::Accepted`] is the final committed color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerEvent {
    Preview(Color),
    Accepted(Color),
    Cancelled(Color),
}

impl PickerEvent {
    #[must_use]
    pub const fn color(self) -> Color {
        match self {
            Self::Preview(color) | Self::Accepted(color) | Self::Cancelled(color) => color,
        }
    }
}

/// Stateful custom native color picker.
///
/// On Windows this opens the crate's own Win32/GDI popup rather than the
/// operating system's legacy `ChooseColorW` dialog.
#[derive(Clone, Debug)]
pub struct ColorPicker {
    pub(crate) show_alpha: bool,
    pub(crate) dpi: u32,
    pub(crate) theme: PickerTheme,
    pub(crate) chrome: PickerChrome,
    pub(crate) labels: PickerLabels,
    pub(crate) font: PickerFont,
    pub(crate) rtl: bool,
    pub(crate) corner_radius: i32,
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self {
            show_alpha: true,
            dpi: 96,
            theme: PickerTheme::default(),
            chrome: PickerChrome::default(),
            labels: PickerLabels::default(),
            font: PickerFont::default(),
            rtl: false,
            corner_radius: 10,
        }
    }
}

impl ColorPicker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Controls whether the picker exposes an alpha slider and `#RRGGBBAA`
    /// hex values. Defaults to `true`.
    ///
    /// When disabled, the alpha row is removed rather than leaving an empty
    /// disabled bar, and the incoming alpha component is preserved.
    pub fn set_show_alpha(&mut self, show_alpha: bool) {
        self.show_alpha = show_alpha;
    }

    #[must_use]
    pub const fn show_alpha(&self) -> bool {
        self.show_alpha
    }

    /// Sets the effective DPI used to scale the custom picker UI.
    ///
    /// `96` is 100%. This is intentionally an explicit input rather than a
    /// forced system-DPI query: applications with their own UI scaling can pass
    /// the same effective DPI and keep the picker visually identical.
    pub fn set_dpi(&mut self, dpi: u32) {
        self.dpi = dpi.clamp(48, 480);
    }

    #[must_use]
    pub const fn dpi(&self) -> u32 {
        self.dpi
    }

    /// Convenience API for applications that store scale as a percentage.
    /// For example, GahYar can pass its `ui_scale` value directly.
    pub fn set_scale_percent(&mut self, percent: u32) {
        let percent = percent.clamp(50, 500);
        self.set_dpi((96 * percent + 50) / 100);
    }

    #[must_use]
    pub fn scale_percent(&self) -> u32 {
        (self.dpi * 100 + 48) / 96
    }

    pub fn set_theme(&mut self, theme: PickerTheme) {
        self.theme = theme;
    }

    #[must_use]
    pub const fn theme(&self) -> PickerTheme {
        self.theme
    }

    pub fn set_chrome(&mut self, chrome: PickerChrome) {
        self.chrome = chrome;
    }

    #[must_use]
    pub const fn chrome(&self) -> PickerChrome {
        self.chrome
    }

    pub fn set_labels(&mut self, labels: PickerLabels) {
        self.labels = labels;
    }

    #[must_use]
    pub fn labels(&self) -> &PickerLabels {
        &self.labels
    }

    pub fn set_font(&mut self, font: PickerFont) {
        self.font = font;
    }

    #[must_use]
    pub fn font(&self) -> &PickerFont {
        &self.font
    }

    /// Enables RTL text/layout details for hosts such as Persian applications.
    pub fn set_rtl(&mut self, rtl: bool) {
        self.rtl = rtl;
    }

    #[must_use]
    pub const fn rtl(&self) -> bool {
        self.rtl
    }

    /// Logical button/input corner radius at 96 DPI.
    pub fn set_corner_radius(&mut self, radius: i32) {
        self.corner_radius = radius.clamp(0, 32);
    }

    #[must_use]
    pub const fn corner_radius(&self) -> i32 {
        self.corner_radius
    }

    /// Opens the custom native picker without an owner window.
    ///
    /// Returns `Ok(None)` when the user cancels.
    pub fn pick(&mut self, initial: Color) -> Result<Option<Color>, PickerError> {
        #[cfg(windows)]
        {
            crate::win32_picker::show(core::ptr::null_mut(), initial, self, None)
                .map_err(PickerError::Windows)
        }

        #[cfg(not(windows))]
        {
            let _ = initial;
            Err(PickerError::UnsupportedPlatform)
        }
    }

    /// Opens the picker without an owner and emits realtime preview/commit/
    /// rollback events while the modal loop is running.
    pub fn pick_live<F>(
        &mut self,
        initial: Color,
        mut on_event: F,
    ) -> Result<Option<Color>, PickerError>
    where
        F: FnMut(PickerEvent),
    {
        #[cfg(windows)]
        {
            self.pick_live_impl(core::ptr::null_mut(), initial, &mut on_event)
        }

        #[cfg(not(windows))]
        {
            let _ = (initial, &mut on_event);
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
        crate::win32_picker::show(owner, initial, self, None).map_err(PickerError::Windows)
    }

    /// Owned-window variant with realtime events.
    ///
    /// `Preview(color)` may be applied to host UI immediately. The picker is
    /// transactional: every non-confirmed exit emits `Cancelled(initial)` so a
    /// host that applied previews can restore its original state.
    #[cfg(windows)]
    pub fn pick_with_owner_live<F>(
        &mut self,
        owner: windows_sys::Win32::Foundation::HWND,
        initial: Color,
        mut on_event: F,
    ) -> Result<Option<Color>, PickerError>
    where
        F: FnMut(PickerEvent),
    {
        self.pick_live_impl(owner, initial, &mut on_event)
    }

    #[cfg(windows)]
    fn pick_live_impl<F>(
        &self,
        owner: windows_sys::Win32::Foundation::HWND,
        initial: Color,
        on_event: &mut F,
    ) -> Result<Option<Color>, PickerError>
    where
        F: FnMut(PickerEvent),
    {
        use std::{
            ffi::c_void,
            panic::{AssertUnwindSafe, catch_unwind},
        };

        struct CallbackContext<F> {
            callback: *mut F,
            panicked: bool,
        }

        unsafe fn invoke<F>(context: *mut c_void, event: PickerEvent)
        where
            F: FnMut(PickerEvent),
        {
            let context = unsafe { &mut *(context as *mut CallbackContext<F>) };
            if context.panicked {
                return;
            }
            let callback = unsafe { &mut *context.callback };
            if catch_unwind(AssertUnwindSafe(|| callback(event))).is_err() {
                context.panicked = true;
            }
        }

        let mut context = CallbackContext {
            callback: on_event as *mut F,
            panicked: false,
        };
        let bridge = crate::win32_picker::EventCallback {
            context: (&mut context as *mut CallbackContext<F>).cast(),
            invoke: invoke::<F>,
        };
        let result = crate::win32_picker::show(owner, initial, self, Some(bridge))
            .map_err(PickerError::Windows);
        if context.panicked {
            Err(PickerError::CallbackPanicked)
        } else {
            result
        }
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
    CallbackPanicked,
}

impl fmt::Display for PickerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                f.write_str("no native color-picker backend is available on this platform")
            }
            Self::Windows(code) => write!(f, "Win32 color-picker error {code}"),
            Self::CallbackPanicked => f.write_str("color-picker realtime callback panicked"),
        }
    }
}

impl std::error::Error for PickerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_percent_round_trip_is_close() {
        let mut picker = ColorPicker::new();
        for scale in [80, 90, 100, 110, 125, 150, 200] {
            picker.set_scale_percent(scale);
            assert!(picker.scale_percent().abs_diff(scale) <= 1);
        }
    }

    #[test]
    fn configurable_state_clamps_safely() {
        let mut picker = ColorPicker::new();
        picker.set_dpi(1);
        picker.set_corner_radius(999);
        assert_eq!(picker.dpi(), 48);
        assert_eq!(picker.corner_radius(), 32);
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_platform_is_explicit() {
        assert_eq!(
            ColorPicker::new().pick(Color::BLACK),
            Err(PickerError::UnsupportedPlatform)
        );
    }
}
