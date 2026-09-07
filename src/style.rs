use crate::Color;

/// Top-level window chrome used by the Windows picker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickerChrome {
    /// A normal tool window with the native Windows caption and close button.
    #[default]
    Native,
    /// A frameless popup. Confirmation/cancellation are provided by the
    /// picker's own buttons and Enter/Escape handling.
    Borderless,
}

/// Colors used by the custom picker UI.
///
/// Applications can map their existing palette directly into this structure so
/// the picker looks like part of the host application rather than a separate
/// system dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickerTheme {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub text: Color,
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub checker_light: Color,
    pub checker_dark: Color,
}

impl PickerTheme {
    /// Neutral light palette. The values intentionally match GahYar's light
    /// surface hierarchy, while remaining generally suitable for Windows apps.
    #[must_use]
    pub const fn light() -> Self {
        Self {
            background: Color::rgb(231, 233, 236),
            surface: Color::rgb(250, 250, 250),
            surface_alt: Color::rgb(241, 243, 246),
            text: Color::rgb(27, 31, 36),
            muted: Color::rgb(91, 98, 107),
            border: Color::rgb(211, 214, 219),
            accent: Color::rgb(230, 181, 43),
            accent_text: Color::rgb(29, 25, 12),
            checker_light: Color::rgb(245, 246, 248),
            checker_dark: Color::rgb(220, 222, 226),
        }
    }

    /// Neutral dark palette. The values intentionally match GahYar's dark
    /// surface hierarchy.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
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
        }
    }

    #[must_use]
    pub fn is_dark(self) -> bool {
        // Fast perceived-luminance approximation. DWM only needs a binary
        // light/dark decision for caption controls.
        let weighted = self.background.r as u32 * 299
            + self.background.g as u32 * 587
            + self.background.b as u32 * 114;
        weighted < 128_000
    }
}

impl Default for PickerTheme {
    fn default() -> Self {
        Self::light()
    }
}

/// Text displayed by the picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerLabels {
    pub title: String,
    pub hex: String,
    pub confirm: String,
    pub cancel: String,
}

impl PickerLabels {
    #[must_use]
    pub fn english() -> Self {
        Self {
            title: "Color".to_owned(),
            hex: "Hex".to_owned(),
            confirm: "OK".to_owned(),
            cancel: "Cancel".to_owned(),
        }
    }

    #[must_use]
    pub fn persian() -> Self {
        Self {
            title: "انتخاب رنگ".to_owned(),
            hex: "هگز".to_owned(),
            confirm: "تأیید".to_owned(),
            cancel: "انصراف".to_owned(),
        }
    }
}

impl Default for PickerLabels {
    fn default() -> Self {
        Self::english()
    }
}

/// Font family and logical pixel sizes at 96 DPI.
///
/// The picker scales these values by [`crate::ColorPicker::dpi`]. A host that
/// loads a private Windows font resource can pass that family name here (for
/// example GahYar passes `Vazirmatn`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerFont {
    pub family: String,
    pub regular_size: i32,
    pub button_size: i32,
}

impl PickerFont {
    #[must_use]
    pub fn new(family: impl Into<String>, regular_size: i32, button_size: i32) -> Self {
        Self {
            family: family.into(),
            regular_size: regular_size.max(8),
            button_size: button_size.max(8),
        }
    }
}

impl Default for PickerFont {
    fn default() -> Self {
        Self::new("Segoe UI", 14, 14)
    }
}
