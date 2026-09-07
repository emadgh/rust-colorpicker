use core::mem::{size_of, zeroed};
use std::{ffi::c_void, sync::OnceLock};

use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::{
        Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUNDSMALL,
            DwmSetWindowAttribute,
        },
        Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACK_PEN, BeginPaint, BitBlt,
            CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW, CreateCompatibleBitmap,
            CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush, DEFAULT_CHARSET,
            DEFAULT_GUI_FONT, DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER, DT_LEFT, DT_RIGHT,
            DT_RTLREADING, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, Ellipse,
            EndPaint, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, FillRect, FrameRect, GetStockObject,
            GetSysColorBrush, HBRUSH, HDC, HFONT, InvalidateRect, NULL_BRUSH, OUT_DEFAULT_PRECIS,
            PAINTSTRUCT, PS_SOLID, RoundRect, SRCCOPY, SelectObject, SetBkColor, SetBkMode,
            SetDIBitsToDevice, SetTextColor, TRANSPARENT, UpdateWindow, WHITE_PEN,
        },
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{EnableWindow, ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN},
        WindowsAndMessaging::{
            AdjustWindowRectEx, CS_DBLCLKS, CS_DROPSHADOW, CW_USEDEFAULT, CreateWindowExW,
            DefWindowProcW, DestroyWindow, DispatchMessageW, GWL_EXSTYLE, GWLP_USERDATA,
            GetClientRect, GetMessageW, GetMonitorInfoW, GetSystemMetrics, GetWindowLongPtrW,
            GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IDC_ARROW, IsDialogMessageW,
            IsWindow, LoadCursorW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MSG, MonitorFromWindow,
            RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SendMessageW, SetForegroundWindow,
            SetWindowLongPtrW, SetWindowTextW, ShowWindow, TranslateMessage, WM_CLOSE, WM_COMMAND,
            WM_CREATE, WM_CTLCOLOREDIT, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_CAPTION, WS_CHILD,
            WS_CLIPCHILDREN, WS_EX_DLGMODALFRAME, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
            WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
        },
    },
};

use crate::{
    Color, ColorPicker, PickerChrome, PickerEvent,
    hsv::{Hsv, rgb_to_hsv},
};

const CLASS_NAME: *const u16 = windows_sys::w!("RustColorPicker.Popup.v5");
static CLASS_REGISTRATION: OnceLock<Result<(), u32>> = OnceLock::new();

const ID_HEX: u16 = 100;
const EN_CHANGE_CODE: u16 = 0x0300;
const WM_SETFONT_CODE: u32 = 0x0030;
const ES_CENTER_STYLE: u32 = 0x0001;
const ES_AUTOHSCROLL_STYLE: u32 = 0x0080;

#[derive(Clone, Copy)]
pub(crate) struct EventCallback {
    pub(crate) context: *mut c_void,
    pub(crate) invoke: unsafe fn(*mut c_void, PickerEvent),
}

impl EventCallback {
    unsafe fn emit(self, event: PickerEvent) {
        unsafe { (self.invoke)(self.context, event) };
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DragTarget {
    #[default]
    None,
    SaturationValue,
    Hue,
    Alpha,
}

#[derive(Clone, Copy, Debug)]
struct Area {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Area {
    const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    fn rect(self) -> RECT {
        RECT {
            left: self.x,
            top: self.y,
            right: self.x + self.width,
            bottom: self.y + self.height,
        }
    }

    fn inset(self, amount: i32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - amount * 2).max(1),
            height: (self.height - amount * 2).max(1),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Layout {
    width: i32,
    height: i32,
    sv: Area,
    hue: Area,
    alpha: Option<Area>,
    hex_label: Area,
    hex_frame: Area,
    hex_edit: Area,
    old_preview: Area,
    new_preview: Area,
    arrow: Area,
    cancel: Area,
    confirm: Area,
}

impl Layout {
    fn new(dpi: u32, show_alpha: bool, rtl: bool) -> Self {
        let s = |value| scale(value, dpi);
        let width = s(400);
        let sv = Area::new(s(16), s(16), s(330), s(330));
        let hue = Area::new(s(358), s(16), s(26), s(330));

        let alpha = show_alpha.then(|| Area::new(s(16), s(358), s(368), s(24)));
        let hex_y = if show_alpha { s(398) } else { s(362) };

        let (hex_label, hex_frame) = if rtl {
            (
                Area::new(s(326), hex_y, s(58), s(34)),
                Area::new(s(16), hex_y, s(302), s(34)),
            )
        } else {
            (
                Area::new(s(16), hex_y, s(58), s(34)),
                Area::new(s(82), hex_y, s(302), s(34)),
            )
        };
        let hex_edit = Area {
            x: hex_frame.x + s(7),
            y: hex_frame.y + s(5),
            width: hex_frame.width - s(14),
            height: hex_frame.height - s(10),
        };

        let preview_y = hex_y + s(50);
        let old_preview = Area::new(s(16), preview_y, s(156), s(42));
        let arrow = Area::new(s(182), preview_y, s(36), s(42));
        let new_preview = Area::new(s(228), preview_y, s(156), s(42));

        let button_y = preview_y + s(58);
        let cancel = Area::new(s(176), button_y, s(96), s(36));
        let confirm = Area::new(s(284), button_y, s(100), s(36));
        let height = button_y + s(52);

        Self {
            width,
            height,
            sv,
            hue,
            alpha,
            hex_label,
            hex_frame,
            hex_edit,
            old_preview,
            new_preview,
            arrow,
            cancel,
            confirm,
        }
    }
}

struct PickerState {
    initial: Color,
    current: Color,
    hsv: Hsv,
    show_alpha: bool,
    dpi: u32,
    theme: crate::PickerTheme,
    chrome: PickerChrome,
    labels: crate::PickerLabels,
    font_spec: crate::PickerFont,
    rtl: bool,
    corner_radius: i32,
    layout: Layout,
    accepted: bool,
    done: bool,
    drag: DragTarget,
    hex_edit: HWND,
    syncing_hex: bool,
    font_regular: HFONT,
    font_button: HFONT,
    edit_brush: HBRUSH,
    sv_cache_hue: f32,
    sv_pixels: Vec<u32>,
    hue_pixels: Vec<u32>,
    callback: Option<EventCallback>,
    last_preview: Color,
    final_event_sent: bool,
}

impl PickerState {
    fn new(initial: Color, picker: &ColorPicker, callback: Option<EventCallback>) -> Self {
        Self {
            initial,
            current: initial,
            hsv: Hsv::from_color(initial),
            show_alpha: picker.show_alpha,
            dpi: picker.dpi,
            theme: picker.theme,
            chrome: picker.chrome,
            labels: picker.labels.clone(),
            font_spec: picker.font.clone(),
            rtl: picker.rtl,
            corner_radius: picker.corner_radius,
            layout: Layout::new(picker.dpi, picker.show_alpha, picker.rtl),
            accepted: false,
            done: false,
            drag: DragTarget::None,
            hex_edit: core::ptr::null_mut(),
            syncing_hex: false,
            font_regular: core::ptr::null_mut(),
            font_button: core::ptr::null_mut(),
            edit_brush: core::ptr::null_mut(),
            sv_cache_hue: f32::NAN,
            sv_pixels: Vec::new(),
            hue_pixels: Vec::new(),
            callback,
            last_preview: initial,
            final_event_sent: false,
        }
    }

    fn set_rgb(&mut self, color: Color) {
        let converted = rgb_to_hsv(color);
        if converted.s > 0.0001 && converted.v > 0.0001 {
            self.hsv.h = converted.h;
        }
        self.hsv.s = converted.s;
        self.hsv.v = converted.v;
        self.current = color;
    }

    fn rebuild_rgb(&mut self) {
        self.current = self.hsv.to_color(self.current.a);
    }

    unsafe fn emit_preview(&mut self) {
        if self.current == self.last_preview {
            return;
        }
        self.last_preview = self.current;
        if let Some(callback) = self.callback {
            unsafe { callback.emit(PickerEvent::Preview(self.current)) };
        }
    }

    unsafe fn emit_final(&mut self, accepted: bool) {
        if self.final_event_sent {
            return;
        }
        self.final_event_sent = true;
        if let Some(callback) = self.callback {
            let event = if accepted {
                PickerEvent::Accepted(self.current)
            } else {
                PickerEvent::Cancelled(self.initial)
            };
            unsafe { callback.emit(event) };
        }
    }
}

impl Drop for PickerState {
    fn drop(&mut self) {
        unsafe {
            if !self.font_regular.is_null() {
                DeleteObject(self.font_regular as _);
            }
            if !self.font_button.is_null() {
                DeleteObject(self.font_button as _);
            }
            if !self.edit_brush.is_null() {
                DeleteObject(self.edit_brush as _);
            }
        }
    }
}

pub(crate) fn show(
    owner: HWND,
    initial: Color,
    picker: &ColorPicker,
    callback: Option<EventCallback>,
) -> Result<Option<Color>, u32> {
    register_class()?;

    let instance = unsafe { GetModuleHandleW(core::ptr::null()) };
    if instance.is_null() {
        return Err(unsafe { GetLastError() });
    }

    let mut state = Box::new(PickerState::new(initial, picker, callback));
    let style = match state.chrome {
        PickerChrome::Native => WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
        PickerChrome::Borderless => WS_POPUP | WS_CLIPCHILDREN,
    };
    let mut ex_style = WS_EX_TOOLWINDOW;
    if state.chrome == PickerChrome::Native {
        ex_style |= WS_EX_DLGMODALFRAME;
    }
    if owner_is_topmost(owner) {
        ex_style |= WS_EX_TOPMOST;
    }

    let mut outer = RECT {
        left: 0,
        top: 0,
        right: state.layout.width,
        bottom: state.layout.height,
    };
    if unsafe { AdjustWindowRectEx(&mut outer, style, 0, ex_style) } == 0 {
        return Err(unsafe { GetLastError() });
    }

    let width = outer.right - outer.left;
    let height = outer.bottom - outer.top;
    let (x, y) = initial_position(owner, width, height, state.dpi);
    let title = wide(&state.labels.title);
    let state_ptr = state.as_mut() as *mut PickerState;

    let hwnd = unsafe {
        CreateWindowExW(
            ex_style,
            CLASS_NAME,
            title.as_ptr(),
            style,
            x,
            y,
            width,
            height,
            owner,
            core::ptr::null_mut(),
            instance,
            state_ptr.cast(),
        )
    };
    if hwnd.is_null() {
        return Err(unsafe { GetLastError() });
    }

    apply_window_appearance(hwnd, &state);

    let owner_enabled = !owner.is_null() && unsafe { IsWindow(owner) } != 0;
    if owner_enabled {
        unsafe { EnableWindow(owner, 0) };
    }

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        SetForegroundWindow(hwnd);
    }

    let mut message: MSG = unsafe { zeroed() };
    let mut loop_error = None;

    while !state.done {
        let status = unsafe { GetMessageW(&mut message, core::ptr::null_mut(), 0, 0) };
        if status == -1 {
            loop_error = Some(unsafe { GetLastError() });
            unsafe { state.emit_final(false) };
            state.done = true;
            break;
        }
        if status == 0 {
            unsafe { state.emit_final(false) };
            state.done = true;
            break;
        }

        if message.message == WM_KEYDOWN {
            if message.wParam == VK_ESCAPE as usize {
                unsafe { finish(hwnd, &mut state, false) };
                continue;
            }
            if message.wParam == VK_RETURN as usize {
                unsafe { finish(hwnd, &mut state, true) };
                continue;
            }
        }

        if unsafe { IsDialogMessageW(hwnd, &message) } == 0 {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    if unsafe { IsWindow(hwnd) } != 0 {
        unsafe { DestroyWindow(hwnd) };
    }

    if owner_enabled && unsafe { IsWindow(owner) } != 0 {
        unsafe {
            EnableWindow(owner, 1);
            SetForegroundWindow(owner);
        }
    }

    if let Some(error) = loop_error {
        return Err(error);
    }

    if state.accepted {
        Ok(Some(state.current))
    } else {
        Ok(None)
    }
}

fn owner_is_topmost(owner: HWND) -> bool {
    !owner.is_null()
        && unsafe { IsWindow(owner) } != 0
        && (unsafe { GetWindowLongPtrW(owner, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST) != 0
}

fn apply_window_appearance(hwnd: HWND, state: &PickerState) {
    unsafe {
        let dark_mode: i32 = i32::from(state.theme.is_dark());
        let caption_color = state.theme.background.to_colorref();
        let text_color = state.theme.text.to_colorref();
        let border_color = state.theme.border.to_colorref();
        let corner = DWMWCP_ROUNDSMALL;

        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            (&dark_mode as *const i32).cast(),
            size_of::<i32>() as u32,
        );
        if state.chrome == PickerChrome::Native {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR as u32,
                (&caption_color as *const u32).cast(),
                size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TEXT_COLOR as u32,
                (&text_color as *const u32).cast(),
                size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                (&border_color as *const u32).cast(),
                size_of::<u32>() as u32,
            );
        }
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&corner as *const i32).cast(),
            size_of::<i32>() as u32,
        );
    }
}

fn register_class() -> Result<(), u32> {
    *CLASS_REGISTRATION.get_or_init(|| unsafe {
        let instance = GetModuleHandleW(core::ptr::null());
        if instance.is_null() {
            return Err(GetLastError());
        }

        let cursor = LoadCursorW(core::ptr::null_mut(), IDC_ARROW);
        if cursor.is_null() {
            return Err(GetLastError());
        }

        let class = WNDCLASSW {
            style: CS_DBLCLKS | CS_DROPSHADOW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: cursor,
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };

        if RegisterClassW(&class) == 0 {
            let error = GetLastError();
            if error == 1410 { Ok(()) } else { Err(error) }
        } else {
            Ok(())
        }
    })
}

fn initial_position(owner: HWND, width: i32, height: i32, dpi: u32) -> (i32, i32) {
    unsafe {
        if !owner.is_null() && IsWindow(owner) != 0 {
            let mut owner_rect = RECT::default();
            if GetWindowRect(owner, &mut owner_rect) != 0 {
                let mut x = owner_rect.left + (owner_rect.right - owner_rect.left - width) / 2;
                let mut y = owner_rect.top + (owner_rect.bottom - owner_rect.top - height) / 2;
                let monitor = MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST);
                if !monitor.is_null() {
                    let mut info: MONITORINFO = zeroed();
                    info.cbSize = size_of::<MONITORINFO>() as u32;
                    if GetMonitorInfoW(monitor, &mut info) != 0 {
                        let gap = scale(8, dpi);
                        x = clamp_window_axis(x, width, info.rcWork.left + gap, info.rcWork.right - gap);
                        y = clamp_window_axis(y, height, info.rcWork.top + gap, info.rcWork.bottom - gap);
                    }
                }
                return (x, y);
            }
        }

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        if screen_w > 0 && screen_h > 0 {
            ((screen_w - width) / 2, (screen_h - height) / 2)
        } else {
            (CW_USEDEFAULT, CW_USEDEFAULT)
        }
    }
}

fn clamp_window_axis(value: i32, size: i32, minimum: i32, maximum: i32) -> i32 {
    if maximum <= minimum || size >= maximum - minimum {
        minimum
    } else {
        value.clamp(minimum, maximum - size)
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
        if create.is_null() {
            return 0;
        }
        let state = unsafe { (*create).lpCreateParams as *mut PickerState };
        if state.is_null() {
            return 0;
        }
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
        return 1;
    }

    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PickerState };

    match message {
        WM_CREATE if !state.is_null() => {
            if unsafe { create_children(hwnd, &mut *state) }.is_err() {
                unsafe {
                    (*state).emit_final(false);
                    (*state).done = true;
                }
                return -1;
            }
            0
        }
        WM_CTLCOLOREDIT if !state.is_null() => unsafe {
            let state = &*state;
            if lparam as HWND == state.hex_edit && !state.edit_brush.is_null() {
                let hdc = wparam as HDC;
                SetTextColor(hdc, state.theme.text.to_colorref());
                SetBkColor(hdc, state.theme.surface.to_colorref());
                state.edit_brush as LRESULT
            } else {
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
        },
        WM_PAINT if !state.is_null() => {
            unsafe { paint(hwnd, &mut *state) };
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN if !state.is_null() => {
            let (x, y) = point_from_lparam(lparam);
            let target = hit_test(unsafe { &*state }, x, y);
            if target != DragTarget::None {
                unsafe {
                    (*state).drag = target;
                    SetCapture(hwnd);
                    update_from_point(hwnd, &mut *state, target, x, y);
                }
            }
            0
        }
        WM_MOUSEMOVE if !state.is_null() => {
            let target = unsafe { (*state).drag };
            if target != DragTarget::None {
                let (x, y) = point_from_lparam(lparam);
                unsafe { update_from_point(hwnd, &mut *state, target, x, y) };
            }
            0
        }
        WM_LBUTTONUP if !state.is_null() => {
            let (x, y) = point_from_lparam(lparam);
            let was_dragging = unsafe { (*state).drag != DragTarget::None };
            if was_dragging {
                unsafe {
                    (*state).drag = DragTarget::None;
                    ReleaseCapture();
                }
            } else if unsafe { (*state).layout.old_preview.contains(x, y) } {
                unsafe {
                    (*state).set_rgb((*state).initial);
                    (*state).emit_preview();
                    sync_hex(&mut *state);
                    InvalidateRect(hwnd, core::ptr::null(), 0);
                }
            } else if unsafe { (*state).layout.confirm.contains(x, y) } {
                unsafe { finish(hwnd, &mut *state, true) };
            } else if unsafe { (*state).layout.cancel.contains(x, y) } {
                unsafe { finish(hwnd, &mut *state, false) };
            }
            0
        }
        WM_COMMAND if !state.is_null() => {
            let id = (wparam & 0xFFFF) as u16;
            let notification = ((wparam >> 16) & 0xFFFF) as u16;
            if id == ID_HEX && notification == EN_CHANGE_CODE {
                unsafe { handle_hex_change(hwnd, &mut *state) };
                0
            } else {
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
        }
        WM_KEYDOWN if !state.is_null() && wparam == VK_ESCAPE as usize => {
            unsafe { finish(hwnd, &mut *state, false) };
            0
        }
        WM_KEYDOWN if !state.is_null() && wparam == VK_RETURN as usize => {
            unsafe { finish(hwnd, &mut *state, true) };
            0
        }
        WM_CLOSE if !state.is_null() => {
            unsafe { finish(hwnd, &mut *state, false) };
            0
        }
        WM_NCDESTROY => {
            if !state.is_null() {
                unsafe {
                    if !(*state).done {
                        (*state).accepted = false;
                        (*state).emit_final(false);
                        (*state).done = true;
                    }
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn create_children(hwnd: HWND, state: &mut PickerState) -> Result<(), u32> {
    let instance = unsafe { GetModuleHandleW(core::ptr::null()) };
    if instance.is_null() {
        return Err(unsafe { GetLastError() });
    }

    state.font_regular = unsafe {
        create_font(
            &state.font_spec.family,
            state.font_spec.regular_size,
            FW_NORMAL as i32,
            state.dpi,
        )
    };
    state.font_button = unsafe {
        create_font(
            &state.font_spec.family,
            state.font_spec.button_size,
            FW_SEMIBOLD as i32,
            state.dpi,
        )
    };
    state.edit_brush = unsafe { CreateSolidBrush(state.theme.surface.to_colorref()) };

    state.hex_edit = unsafe {
        create_child(
            hwnd,
            instance,
            windows_sys::w!("EDIT"),
            core::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_CENTER_STYLE | ES_AUTOHSCROLL_STYLE,
            state.layout.hex_edit,
            ID_HEX,
        )?
    };

    let font = if state.font_regular.is_null() {
        unsafe { GetStockObject(DEFAULT_GUI_FONT) }
    } else {
        state.font_regular as _
    };
    if !font.is_null() {
        unsafe { SendMessageW(state.hex_edit, WM_SETFONT_CODE, font as usize, 1) };
    }

    unsafe { sync_hex(state) };
    Ok(())
}

unsafe fn create_font(family: &str, logical_size: i32, weight: i32, dpi: u32) -> HFONT {
    let family = wide(family);
    unsafe {
        CreateFontW(
            -scale(logical_size.max(8), dpi),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32 | FF_DONTCARE as u32,
            family.as_ptr(),
        )
    }
}

unsafe fn create_child(
    parent: HWND,
    instance: windows_sys::Win32::Foundation::HINSTANCE,
    class_name: *const u16,
    text: *const u16,
    style: u32,
    area: Area,
    id: u16,
) -> Result<HWND, u32> {
    let menu = if id == 0 {
        core::ptr::null_mut()
    } else {
        id as usize as _
    };

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name,
            text,
            style,
            area.x,
            area.y,
            area.width,
            area.height,
            parent,
            menu,
            instance,
            core::ptr::null(),
        )
    };

    if hwnd.is_null() {
        Err(unsafe { GetLastError() })
    } else {
        Ok(hwnd)
    }
}

unsafe fn finish(hwnd: HWND, state: &mut PickerState, accepted: bool) {
    if state.done {
        return;
    }
    state.accepted = accepted;
    state.done = true;
    state.drag = DragTarget::None;
    unsafe {
        state.emit_final(accepted);
        ReleaseCapture();
        DestroyWindow(hwnd);
    }
}

fn hit_test(state: &PickerState, x: i32, y: i32) -> DragTarget {
    if state.layout.sv.contains(x, y) {
        DragTarget::SaturationValue
    } else if state.layout.hue.contains(x, y) {
        DragTarget::Hue
    } else if state
        .layout
        .alpha
        .is_some_and(|area| area.contains(x, y))
    {
        DragTarget::Alpha
    } else {
        DragTarget::None
    }
}

unsafe fn update_from_point(
    hwnd: HWND,
    state: &mut PickerState,
    target: DragTarget,
    x: i32,
    y: i32,
) {
    match target {
        DragTarget::SaturationValue => {
            let area = state.layout.sv;
            state.hsv.s = normalized(x - area.x, area.width);
            state.hsv.v = 1.0 - normalized(y - area.y, area.height);
            state.rebuild_rgb();
        }
        DragTarget::Hue => {
            let area = state.layout.hue;
            state.hsv.h = (1.0 - normalized(y - area.y, area.height)).rem_euclid(1.0);
            state.rebuild_rgb();
        }
        DragTarget::Alpha => {
            let Some(area) = state.layout.alpha else {
                return;
            };
            state.current.a = (normalized(x - area.x, area.width) * 255.0).round() as u8;
        }
        DragTarget::None => return,
    }

    unsafe {
        state.emit_preview();
        sync_hex(state);
        InvalidateRect(hwnd, core::ptr::null(), 0);
    }
}

fn normalized(value: i32, length: i32) -> f32 {
    if length <= 1 {
        return 0.0;
    }
    (value as f32 / (length - 1) as f32).clamp(0.0, 1.0)
}

unsafe fn handle_hex_change(hwnd: HWND, state: &mut PickerState) {
    if state.syncing_hex || state.hex_edit.is_null() {
        return;
    }

    let length = unsafe { GetWindowTextLengthW(state.hex_edit) };
    if length <= 0 || length > 16 {
        return;
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let copied =
        unsafe { GetWindowTextW(state.hex_edit, buffer.as_mut_ptr(), buffer.len() as i32) };
    if copied <= 0 {
        return;
    }

    let text = String::from_utf16_lossy(&buffer[..copied as usize]);
    let Ok(mut color) = Color::parse_hex(text.trim()) else {
        return;
    };

    if !state.show_alpha {
        color.a = state.current.a;
    }

    state.set_rgb(color);
    unsafe {
        state.emit_preview();
        InvalidateRect(hwnd, core::ptr::null(), 0);
    }
}

unsafe fn sync_hex(state: &mut PickerState) {
    if state.hex_edit.is_null() {
        return;
    }

    let text = if state.show_alpha {
        state.current.to_hex_rgba()
    } else {
        state.current.to_hex_rgb()
    };
    let wide = wide(&text);

    state.syncing_hex = true;
    unsafe { SetWindowTextW(state.hex_edit, wide.as_ptr()) };
    state.syncing_hex = false;
}

unsafe fn paint(hwnd: HWND, state: &mut PickerState) {
    let mut paint: PAINTSTRUCT = unsafe { zeroed() };
    let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
    if hdc.is_null() {
        return;
    }

    let mut client = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client) };
    let width = client.right - client.left;
    let height = client.bottom - client.top;

    let memory_dc = unsafe { CreateCompatibleDC(hdc) };
    if memory_dc.is_null() {
        unsafe {
            render_picker(hdc, &client, state);
            EndPaint(hwnd, &paint);
        }
        return;
    }

    let bitmap = unsafe { CreateCompatibleBitmap(hdc, width, height) };
    if bitmap.is_null() {
        unsafe {
            DeleteDC(memory_dc);
            render_picker(hdc, &client, state);
            EndPaint(hwnd, &paint);
        }
        return;
    }

    let old_bitmap = unsafe { SelectObject(memory_dc, bitmap as _) };

    unsafe {
        render_picker(memory_dc, &client, state);
        BitBlt(hdc, 0, 0, width, height, memory_dc, 0, 0, SRCCOPY);
        SelectObject(memory_dc, old_bitmap);
        DeleteObject(bitmap as _);
        DeleteDC(memory_dc);
        EndPaint(hwnd, &paint);
    }
}

unsafe fn render_picker(hdc: HDC, client: &RECT, state: &mut PickerState) {
    unsafe {
        fill_solid(hdc, client, state.theme.background);
        if state.chrome == PickerChrome::Borderless {
            frame_rect(hdc, client, state.theme.border);
        }

        paint_sv(hdc, state);
        paint_hue(hdc, state);
        if state.show_alpha {
            paint_alpha(hdc, state);
        }

        draw_round_box(
            hdc,
            state.layout.hex_frame,
            state.theme.surface,
            state.theme.border,
            scale(state.corner_radius, state.dpi),
        );
        draw_text(
            hdc,
            &state.labels.hex,
            state.layout.hex_label,
            state.theme.muted,
            regular_font(state),
            if state.rtl {
                DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_RTLREADING
            } else {
                DT_LEFT | DT_VCENTER | DT_SINGLELINE
            },
        );

        paint_preview(hdc, state, state.layout.old_preview, state.initial);
        paint_preview(hdc, state, state.layout.new_preview, state.current);
        draw_text(
            hdc,
            "→",
            state.layout.arrow,
            state.theme.muted,
            regular_font(state),
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        draw_button(
            hdc,
            state.layout.cancel,
            state.theme.surface_alt,
            state.theme.border,
            state.theme.text,
            &state.labels.cancel,
            button_font(state),
            state.rtl,
            scale(state.corner_radius, state.dpi),
        );
        draw_button(
            hdc,
            state.layout.confirm,
            state.theme.accent,
            state.theme.accent,
            state.theme.accent_text,
            &state.labels.confirm,
            button_font(state),
            state.rtl,
            scale(state.corner_radius, state.dpi),
        );
        draw_markers(hdc, state);
    }
}

fn regular_font(state: &PickerState) -> HFONT {
    if state.font_regular.is_null() {
        unsafe { GetStockObject(DEFAULT_GUI_FONT) as HFONT }
    } else {
        state.font_regular
    }
}

fn button_font(state: &PickerState) -> HFONT {
    if state.font_button.is_null() {
        regular_font(state)
    } else {
        state.font_button
    }
}

unsafe fn paint_sv(hdc: HDC, state: &mut PickerState) {
    let area = state.layout.sv;
    let width = area.width as usize;
    let height = area.height as usize;
    let required = width * height;
    let hue_changed =
        !state.sv_cache_hue.is_finite() || (state.sv_cache_hue - state.hsv.h).abs() > 0.000_001;

    if state.sv_pixels.len() != required {
        state.sv_pixels.resize(required, 0);
        state.sv_cache_hue = f32::NAN;
    }

    if hue_changed || !state.sv_cache_hue.is_finite() {
        for y in 0..height {
            let value = 1.0 - normalized(y as i32, area.height);
            for x in 0..width {
                let saturation = normalized(x as i32, area.width);
                let color = Hsv {
                    h: state.hsv.h,
                    s: saturation,
                    v: value,
                }
                .to_color(255);
                state.sv_pixels[y * width + x] = dib_pixel(color);
            }
        }
        state.sv_cache_hue = state.hsv.h;
    }

    unsafe {
        draw_bitmap(hdc, area, &state.sv_pixels);
        frame_area(hdc, area, state.theme.border);
    }
}

unsafe fn paint_hue(hdc: HDC, state: &mut PickerState) {
    let area = state.layout.hue;
    let width = area.width as usize;
    let height = area.height as usize;
    let required = width * height;
    if state.hue_pixels.len() != required {
        state.hue_pixels.resize(required, 0);
        for y in 0..height {
            let hue = (1.0 - normalized(y as i32, area.height)).rem_euclid(1.0);
            let color = Hsv {
                h: hue,
                s: 1.0,
                v: 1.0,
            }
            .to_color(255);
            let pixel = dib_pixel(color);
            for x in 0..width {
                state.hue_pixels[y * width + x] = pixel;
            }
        }
    }

    unsafe {
        draw_bitmap(hdc, area, &state.hue_pixels);
        frame_area(hdc, area, state.theme.border);
    }
}

unsafe fn paint_alpha(hdc: HDC, state: &PickerState) {
    let Some(area) = state.layout.alpha else {
        return;
    };
    let width = area.width as usize;
    let height = area.height as usize;
    let mut pixels = vec![0u32; width * height];
    let tile = scale(8, state.dpi).max(2) as usize;

    for y in 0..height {
        for x in 0..width {
            let alpha = (normalized(x as i32, area.width) * 255.0).round() as u8;
            let foreground = Color::rgba(state.current.r, state.current.g, state.current.b, alpha);
            let background = checker_color(state, x, y, tile);
            pixels[y * width + x] = dib_pixel(blend_over(foreground, background));
        }
    }

    unsafe {
        draw_bitmap(hdc, area, &pixels);
        frame_area(hdc, area, state.theme.border);
    }
}

unsafe fn paint_preview(hdc: HDC, state: &PickerState, area: Area, color: Color) {
    let width = area.width as usize;
    let height = area.height as usize;
    let mut pixels = vec![0u32; width * height];
    let tile = scale(8, state.dpi).max(2) as usize;

    for y in 0..height {
        for x in 0..width {
            let background = checker_color(state, x, y, tile);
            pixels[y * width + x] = dib_pixel(blend_over(color, background));
        }
    }

    unsafe {
        draw_bitmap(hdc, area, &pixels);
        frame_area(hdc, area, state.theme.border);
    }
}

fn checker_color(state: &PickerState, x: usize, y: usize, tile: usize) -> Color {
    if ((x / tile) + (y / tile)) % 2 == 0 {
        state.theme.checker_light
    } else {
        state.theme.checker_dark
    }
}

unsafe fn draw_bitmap(hdc: HDC, area: Area, pixels: &[u32]) {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: area.width,
            biHeight: -area.height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };

    unsafe {
        SetDIBitsToDevice(
            hdc,
            area.x,
            area.y,
            area.width as u32,
            area.height as u32,
            0,
            0,
            0,
            area.height as u32,
            pixels.as_ptr().cast(),
            &info,
            DIB_RGB_COLORS,
        );
    }
}

unsafe fn draw_markers(hdc: HDC, state: &PickerState) {
    let sv = state.layout.sv;
    let radius = scale(10, state.dpi).max(5);
    let sx = sv.x + (state.hsv.s * (sv.width - 1) as f32).round() as i32;
    let sy = sv.y + ((1.0 - state.hsv.v) * (sv.height - 1) as f32).round() as i32;

    let old_brush = unsafe { SelectObject(hdc, GetStockObject(NULL_BRUSH)) };
    let old_pen = unsafe { SelectObject(hdc, GetStockObject(BLACK_PEN)) };
    unsafe { Ellipse(hdc, sx - radius, sy - radius, sx + radius + 1, sy + radius + 1) };
    unsafe { SelectObject(hdc, GetStockObject(WHITE_PEN)) };
    unsafe {
        Ellipse(
            hdc,
            sx - radius + 1,
            sy - radius + 1,
            sx + radius,
            sy + radius,
        )
    };
    unsafe {
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
    }

    let hue = state.layout.hue;
    let hue_y = hue.y + ((1.0 - state.hsv.h) * (hue.height - 1) as f32).round() as i32;
    let thickness = scale(2, state.dpi).max(1);
    let hue_marker = RECT {
        left: hue.x - thickness,
        top: hue_y - thickness,
        right: hue.x + hue.width + thickness,
        bottom: hue_y + thickness + 1,
    };
    unsafe { frame_rect(hdc, &hue_marker, state.theme.text) };

    if let Some(alpha) = state.layout.alpha {
        let alpha_x = alpha.x
            + ((state.current.a as f32 / 255.0) * (alpha.width - 1) as f32).round() as i32;
        let alpha_marker = RECT {
            left: alpha_x - thickness,
            top: alpha.y - thickness,
            right: alpha_x + thickness + 1,
            bottom: alpha.y + alpha.height + thickness,
        };
        unsafe { frame_rect(hdc, &alpha_marker, state.theme.text) };
    }
}

unsafe fn draw_button(
    hdc: HDC,
    area: Area,
    fill: Color,
    border: Color,
    text_color: Color,
    text: &str,
    font: HFONT,
    rtl: bool,
    radius: i32,
) {
    unsafe {
        draw_round_box(hdc, area, fill, border, radius);
        draw_text(
            hdc,
            text,
            area,
            text_color,
            font,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | if rtl { DT_RTLREADING } else { 0 },
        );
    }
}

unsafe fn draw_round_box(hdc: HDC, area: Area, fill: Color, border: Color, radius: i32) {
    if radius <= 1 {
        unsafe {
            fill_solid(hdc, &area.rect(), fill);
            frame_area(hdc, area, border);
        }
        return;
    }

    let brush = unsafe { CreateSolidBrush(fill.to_colorref()) };
    let pen = unsafe { CreatePen(PS_SOLID, 1, border.to_colorref()) };
    if brush.is_null() || pen.is_null() {
        if !brush.is_null() {
            unsafe { DeleteObject(brush as _) };
        }
        if !pen.is_null() {
            unsafe { DeleteObject(pen as _) };
        }
        return;
    }
    let old_brush = unsafe { SelectObject(hdc, brush as _) };
    let old_pen = unsafe { SelectObject(hdc, pen as _) };
    unsafe {
        RoundRect(
            hdc,
            area.x,
            area.y,
            area.x + area.width,
            area.y + area.height,
            radius * 2,
            radius * 2,
        );
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        DeleteObject(brush as _);
        DeleteObject(pen as _);
    }
}

unsafe fn draw_text(
    hdc: HDC,
    text: &str,
    area: Area,
    color: Color,
    font: HFONT,
    format: u32,
) {
    let mut content = wide(text);
    let mut rect = area.rect();
    let old_font = if font.is_null() {
        core::ptr::null_mut()
    } else {
        unsafe { SelectObject(hdc, font as _) }
    };
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, color.to_colorref());
        DrawTextW(hdc, content.as_mut_ptr(), -1, &mut rect, format);
        if !old_font.is_null() {
            SelectObject(hdc, old_font);
        }
    }
}

unsafe fn fill_solid(hdc: HDC, rect: &RECT, color: Color) {
    let brush = unsafe { CreateSolidBrush(color.to_colorref()) };
    if brush.is_null() {
        return;
    }
    unsafe {
        FillRect(hdc, rect, brush);
        DeleteObject(brush as _);
    }
}

unsafe fn frame_area(hdc: HDC, area: Area, color: Color) {
    unsafe { frame_rect(hdc, &area.rect(), color) };
}

unsafe fn frame_rect(hdc: HDC, rect: &RECT, color: Color) {
    let brush = unsafe { CreateSolidBrush(color.to_colorref()) };
    if brush.is_null() {
        return;
    }
    unsafe {
        FrameRect(hdc, rect, brush);
        DeleteObject(brush as _);
    }
}

fn blend_over(foreground: Color, background: Color) -> Color {
    let a = foreground.a as u32;
    let inv = 255 - a;
    Color::rgb(
        ((foreground.r as u32 * a + background.r as u32 * inv + 127) / 255) as u8,
        ((foreground.g as u32 * a + background.g as u32 * inv + 127) / 255) as u8,
        ((foreground.b as u32 * a + background.b as u32 * inv + 127) / 255) as u8,
    )
}

fn dib_pixel(color: Color) -> u32 {
    color.b as u32 | ((color.g as u32) << 8) | ((color.r as u32) << 16)
}

fn point_from_lparam(lparam: LPARAM) -> (i32, i32) {
    let raw = lparam as u32;
    (raw as i16 as i32, (raw >> 16) as i16 as i32)
}

fn scale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi as i64 + 48) / 96) as i32
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(core::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_scale_and_remove_alpha_cleanly() {
        for dpi in [77, 86, 96, 106, 120, 144, 192] {
            let with_alpha = Layout::new(dpi, true, false);
            let rgb_only = Layout::new(dpi, false, false);
            assert!(with_alpha.alpha.is_some());
            assert!(rgb_only.alpha.is_none());
            assert!(rgb_only.height < with_alpha.height);
            assert!(with_alpha.confirm.x + with_alpha.confirm.width <= with_alpha.width);
            assert!(with_alpha.sv.width > 0 && with_alpha.hue.width > 0);
        }
    }

    #[test]
    fn rtl_moves_hex_label_to_the_right() {
        let ltr = Layout::new(96, false, false);
        let rtl = Layout::new(96, false, true);
        assert!(ltr.hex_label.x < ltr.hex_frame.x);
        assert!(rtl.hex_label.x > rtl.hex_frame.x);
    }

    #[test]
    fn clamp_keeps_windows_inside_work_area() {
        assert_eq!(clamp_window_axis(-100, 200, 10, 1000), 10);
        assert_eq!(clamp_window_axis(950, 200, 10, 1000), 800);
        assert_eq!(clamp_window_axis(200, 200, 10, 1000), 200);
    }
}
