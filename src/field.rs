//! Native Win32 color-field control.
//!
//! This module is deliberately Windows-specific. Framework-based applications
//! should normally render their own field/button and call [`crate::ColorPicker`]
//! on click. Pure Win32 applications can use [`ColorField`] directly.

use core::fmt;
use std::{marker::PhantomData, rc::Rc, sync::OnceLock};

use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, COLOR_WINDOWFRAME, CreateSolidBrush, DeleteObject, DrawFocusRect, EndPaint,
        FillRect, FrameRect, GetSysColorBrush, InvalidateRect, PAINTSTRUCT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{GetFocus, SetFocus, VK_RETURN, VK_SPACE},
        WindowsAndMessaging::{
            CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
            GetClientRect, GetDlgCtrlID, GetParent, GetWindowLongPtrW, IDC_ARROW, IsWindow,
            LoadCursorW, RegisterClassW, SendMessageW, SetWindowLongPtrW, WM_COMMAND,
            WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONUP, WM_NCDESTROY, WM_PAINT,
            WM_SETFOCUS, WNDCLASSW, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
        },
    },
};

use crate::{Color, ColorPicker, PickerError};

/// Notification code placed in the high word of `WM_COMMAND` after the user
/// accepts a different color.
pub const COLOR_FIELD_CHANGED: u16 = 0x0100;

const CLASS_NAME: *const u16 = windows_sys::w!("RustColorPicker.ColorField");
static CLASS_REGISTRATION: OnceLock<Result<(), u32>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorFieldBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl ColorFieldBounds {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Owned native child control that displays a color swatch and opens this
/// crate's custom Win32/GDI picker when clicked or activated with Enter/Space.
///
/// The control sends `WM_COMMAND` to its parent after a changed selection. The
/// low word is the control ID and the high word is [`COLOR_FIELD_CHANGED`].
///
/// `ColorField` is intentionally neither `Send` nor `Sync`: Win32 HWND controls
/// must be used on the UI thread that created them.
pub struct ColorField {
    hwnd: HWND,
    _ui_thread_only: PhantomData<Rc<()>>,
}

impl ColorField {
    /// Creates a native child color field.
    ///
    /// # Safety
    ///
    /// `parent` must be a valid HWND owned by the calling UI thread and must
    /// remain valid for at least as long as this child control is alive.
    pub unsafe fn create(
        parent: HWND,
        control_id: u16,
        bounds: ColorFieldBounds,
        initial: Color,
    ) -> Result<Self, FieldError> {
        if parent.is_null() || unsafe { IsWindow(parent) } == 0 {
            return Err(FieldError::InvalidParent);
        }

        register_class()?;

        let instance = unsafe { GetModuleHandleW(core::ptr::null()) };
        if instance.is_null() {
            return Err(FieldError::Windows(unsafe { GetLastError() }));
        }

        let hwnd = unsafe {
            CreateWindowExW(
                0,
                CLASS_NAME,
                core::ptr::null(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                parent,
                control_id as usize as _,
                instance,
                core::ptr::null(),
            )
        };

        if hwnd.is_null() {
            return Err(FieldError::Windows(unsafe { GetLastError() }));
        }

        let state = Box::new(FieldState {
            color: initial,
            picker: ColorPicker::new(),
        });
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
            InvalidateRect(hwnd, core::ptr::null(), 0);
        }

        Ok(Self {
            hwnd,
            _ui_thread_only: PhantomData,
        })
    }

    #[must_use]
    pub const fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn color(&self) -> Result<Color, FieldError> {
        let state = state_ptr(self.hwnd)?;
        Ok(unsafe { (*state).color })
    }

    pub fn set_color(&self, color: Color) -> Result<(), FieldError> {
        let state = state_ptr(self.hwnd)?;
        unsafe {
            (*state).color = color;
            InvalidateRect(self.hwnd, core::ptr::null(), 0);
        }
        Ok(())
    }

    /// Opens the picker programmatically. A changed accepted color also emits
    /// the same parent notification as a mouse/keyboard activation.
    pub fn open_picker(&self) -> Result<Option<Color>, FieldError> {
        activate(self.hwnd)
    }
}

impl Drop for ColorField {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_null() && IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
    }
}

struct FieldState {
    color: Color,
    picker: ColorPicker,
}

fn register_class() -> Result<(), FieldError> {
    let result = CLASS_REGISTRATION.get_or_init(|| unsafe {
        let instance = GetModuleHandleW(core::ptr::null());
        if instance.is_null() {
            return Err(GetLastError());
        }

        let cursor = LoadCursorW(core::ptr::null_mut(), IDC_ARROW);
        if cursor.is_null() {
            return Err(GetLastError());
        }

        let class = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: cursor,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };

        if RegisterClassW(&class) == 0 {
            Err(GetLastError())
        } else {
            Ok(())
        }
    });

    match *result {
        Ok(()) => Ok(()),
        Err(code) => Err(FieldError::Windows(code)),
    }
}

fn state_ptr(hwnd: HWND) -> Result<*mut FieldState, FieldError> {
    unsafe {
        if hwnd.is_null() || IsWindow(hwnd) == 0 {
            return Err(FieldError::Destroyed);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut FieldState;
        if ptr.is_null() {
            Err(FieldError::Destroyed)
        } else {
            Ok(ptr)
        }
    }
}

fn activate(hwnd: HWND) -> Result<Option<Color>, FieldError> {
    let state = state_ptr(hwnd)?;

    // Copy state out before opening the modal custom popup. The picker runs a
    // nested message loop, so holding a Rust reference into window state across
    // that call would make re-entrant messages unsound.
    let (initial, mut picker) = unsafe { ((*state).color, (*state).picker.clone()) };
    let owner = unsafe { GetParent(hwnd) };
    let selected = picker
        .pick_with_owner(owner, initial)
        .map_err(FieldError::Picker)?;

    // Re-resolve after the modal popup because the host could have destroyed
    // the control while the nested message loop was active.
    let state = state_ptr(hwnd)?;
    unsafe {
        (*state).picker = picker;
    }

    let Some(color) = selected else {
        return Ok(None);
    };

    if color != initial {
        unsafe {
            (*state).color = color;
            InvalidateRect(hwnd, core::ptr::null(), 0);
        }
        notify_changed(hwnd);
    }

    Ok(Some(color))
}

fn notify_changed(hwnd: HWND) {
    unsafe {
        let parent = GetParent(hwnd);
        if parent.is_null() {
            return;
        }
        let id = GetDlgCtrlID(hwnd) as u16;
        let wparam = id as usize | ((COLOR_FIELD_CHANGED as usize) << 16);
        SendMessageW(parent, WM_COMMAND, wparam, hwnd as LPARAM);
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            unsafe { paint(hwnd) };
            0
        }
        WM_ERASEBKGND => 1,
        WM_SETFOCUS | WM_KILLFOCUS => {
            unsafe { InvalidateRect(hwnd, core::ptr::null(), 0) };
            0
        }
        WM_LBUTTONUP => {
            unsafe { SetFocus(hwnd) };
            let _ = activate(hwnd);
            0
        }
        WM_KEYDOWN if wparam == VK_SPACE as usize || wparam == VK_RETURN as usize => {
            let _ = activate(hwnd);
            0
        }
        WM_NCDESTROY => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut FieldState };
            if !ptr.is_null() {
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut paint: PAINTSTRUCT = unsafe { core::mem::zeroed() };
    let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
    if hdc.is_null() {
        return;
    }

    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect) };

    let frame = unsafe { GetSysColorBrush(COLOR_WINDOWFRAME) };
    if !frame.is_null() {
        unsafe { FrameRect(hdc, &rect, frame) };
    }

    rect.left += 2;
    rect.top += 2;
    rect.right -= 2;
    rect.bottom -= 2;

    if let Ok(state) = state_ptr(hwnd) {
        let color = unsafe { (*state).color };
        let brush = unsafe { CreateSolidBrush(color.to_colorref()) };
        if !brush.is_null() {
            unsafe {
                FillRect(hdc, &rect, brush);
                DeleteObject(brush as _);
            }
        }
    }

    if unsafe { GetFocus() } == hwnd {
        rect.left += 3;
        rect.top += 3;
        rect.right -= 3;
        rect.bottom -= 3;
        unsafe { DrawFocusRect(hdc, &rect) };
    }

    unsafe { EndPaint(hwnd, &paint) };
}

#[derive(Debug)]
pub enum FieldError {
    InvalidParent,
    Destroyed,
    Windows(u32),
    Picker(PickerError),
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParent => f.write_str("parent HWND is invalid"),
            Self::Destroyed => f.write_str("color field HWND has been destroyed"),
            Self::Windows(code) => write!(f, "Win32 error {code}"),
            Self::Picker(error) => write!(f, "color picker failed: {error}"),
        }
    }
}

impl std::error::Error for FieldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Picker(error) => Some(error),
            _ => None,
        }
    }
}
