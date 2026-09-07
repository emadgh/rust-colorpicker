use core::mem::{size_of, zeroed};
use std::sync::OnceLock;

use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::{
        Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUNDSMALL,
            DwmSetWindowAttribute,
        },
        Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACK_PEN, BeginPaint, BitBlt, COLOR_WINDOW,
            CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DEFAULT_GUI_FONT,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, Ellipse, EndPaint, FillRect, FrameRect,
            GetStockObject, GetSysColorBrush, InvalidateRect, NULL_BRUSH, PAINTSTRUCT, SRCCOPY,
            SelectObject, SetDIBitsToDevice, UpdateWindow, WHITE_PEN,
        },
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{EnableWindow, ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN},
        WindowsAndMessaging::{
            AdjustWindowRectEx, CS_DBLCLKS, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
            DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetClientRect, GetMessageW,
            GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW,
            GetWindowTextW, IDC_ARROW, IsDialogMessageW, IsWindow, LoadCursorW, MSG,
            RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SendMessageW, SetForegroundWindow,
            SetWindowLongPtrW, SetWindowTextW, ShowWindow, TranslateMessage, WM_CLOSE, WM_COMMAND,
            WM_CREATE, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
            WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD,
            WS_CLIPCHILDREN, WS_EX_DLGMODALFRAME, WS_EX_TOOLWINDOW, WS_POPUP, WS_SYSMENU,
            WS_TABSTOP, WS_VISIBLE,
        },
    },
};

use crate::{
    Color,
    hsv::{Hsv, rgb_to_hsv},
};

const CLASS_NAME: *const u16 = windows_sys::w!("RustColorPicker.Popup.v4");
static CLASS_REGISTRATION: OnceLock<Result<(), u32>> = OnceLock::new();

const CLIENT_WIDTH: i32 = 384;
const CLIENT_HEIGHT: i32 = 480;

const SV: Area = Area::new(16, 16, 314, 314);
const HUE: Area = Area::new(342, 16, 26, 314);
const ALPHA: Area = Area::new(16, 342, 352, 24);
const OLD_PREVIEW: Area = Area::new(16, 424, 148, 40);
const NEW_PREVIEW: Area = Area::new(220, 424, 148, 40);

const ID_HEX: u16 = 100;
const EN_CHANGE_CODE: u16 = 0x0300;
const WM_SETFONT_CODE: u32 = 0x0030;
const ES_CENTER_STYLE: u32 = 0x0001;
const ES_AUTOHSCROLL_STYLE: u32 = 0x0080;
const SS_RIGHT_STYLE: u32 = 0x0002;
const SS_CENTER_STYLE: u32 = 0x0001;

const BACKGROUND: Color = Color::rgb(250, 250, 251);
const SURFACE_BORDER: Color = Color::rgb(214, 216, 220);
const MARKER_DARK: Color = Color::rgb(32, 33, 36);
const CHECKER_LIGHT: Color = Color::rgb(246, 246, 247);
const CHECKER_DARK: Color = Color::rgb(216, 218, 221);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DragTarget {
    #[default]
    None,
    SaturationValue,
    Hue,
    Alpha,
}

struct PickerState {
    initial: Color,
    current: Color,
    hsv: Hsv,
    show_alpha: bool,
    accepted: bool,
    done: bool,
    drag: DragTarget,
    hex_edit: HWND,
    syncing_hex: bool,
    sv_cache_hue: f32,
    sv_pixels: Vec<u32>,
}

impl PickerState {
    fn new(initial: Color, show_alpha: bool) -> Self {
        Self {
            initial,
            current: initial,
            hsv: Hsv::from_color(initial),
            show_alpha,
            accepted: false,
            done: false,
            drag: DragTarget::None,
            hex_edit: core::ptr::null_mut(),
            syncing_hex: false,
            sv_cache_hue: f32::NAN,
            sv_pixels: Vec::new(),
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
}

#[derive(Clone, Copy)]
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
}

pub(crate) fn show(owner: HWND, initial: Color, show_alpha: bool) -> Result<Option<Color>, u32> {
    register_class()?;

    let instance = unsafe { GetModuleHandleW(core::ptr::null()) };
    if instance.is_null() {
        return Err(unsafe { GetLastError() });
    }

    let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    let ex_style = WS_EX_DLGMODALFRAME | WS_EX_TOOLWINDOW;
    let mut outer = RECT {
        left: 0,
        top: 0,
        right: CLIENT_WIDTH,
        bottom: CLIENT_HEIGHT,
    };
    if unsafe { AdjustWindowRectEx(&mut outer, style, 0, ex_style) } == 0 {
        return Err(unsafe { GetLastError() });
    }

    let width = outer.right - outer.left;
    let height = outer.bottom - outer.top;
    let (x, y) = initial_position(owner, width, height);

    let mut state = Box::new(PickerState::new(initial, show_alpha));
    let state_ptr = state.as_mut() as *mut PickerState;

    let hwnd = unsafe {
        CreateWindowExW(
            ex_style,
            CLASS_NAME,
            windows_sys::w!("Color"),
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

    apply_window_appearance(hwnd);

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
            break;
        }
        if status == 0 {
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

fn apply_window_appearance(hwnd: HWND) {
    unsafe {
        let light_mode: i32 = 0;
        let caption_color = Color::rgb(248, 248, 250).to_colorref();
        let text_color = Color::rgb(35, 36, 40).to_colorref();
        let border_color = Color::rgb(219, 221, 225).to_colorref();
        let corner = DWMWCP_ROUNDSMALL;

        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            (&light_mode as *const i32).cast(),
            size_of::<i32>() as u32,
        );
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
            style: CS_DBLCLKS,
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

fn initial_position(owner: HWND, width: i32, height: i32) -> (i32, i32) {
    unsafe {
        if !owner.is_null() && IsWindow(owner) != 0 {
            let mut owner_rect = RECT::default();
            if GetWindowRect(owner, &mut owner_rect) != 0 {
                return (
                    owner_rect.left + ((owner_rect.right - owner_rect.left - width) / 2),
                    owner_rect.top + ((owner_rect.bottom - owner_rect.top - height) / 2),
                );
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
                unsafe { (*state).done = true };
                return -1;
            }
            0
        }
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
            } else if OLD_PREVIEW.contains(x, y) {
                unsafe {
                    (*state).set_rgb((*state).initial);
                    sync_hex(&mut *state);
                    InvalidateRect(hwnd, core::ptr::null(), 0);
                }
            } else if NEW_PREVIEW.contains(x, y) {
                unsafe { finish(hwnd, &mut *state, true) };
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
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
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

    let font = unsafe { GetStockObject(DEFAULT_GUI_FONT) };

    let hex_label = unsafe {
        create_child(
            hwnd,
            instance,
            windows_sys::w!("STATIC"),
            windows_sys::w!("Hex"),
            WS_CHILD | WS_VISIBLE | SS_RIGHT_STYLE,
            16,
            382,
            64,
            24,
            0,
        )?
    };

    state.hex_edit = unsafe {
        create_child(
            hwnd,
            instance,
            windows_sys::w!("EDIT"),
            core::ptr::null(),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_BORDER
                | ES_CENTER_STYLE
                | ES_AUTOHSCROLL_STYLE,
            88,
            378,
            280,
            30,
            ID_HEX,
        )?
    };

    let arrow = unsafe {
        create_child(
            hwnd,
            instance,
            windows_sys::w!("STATIC"),
            windows_sys::w!("→"),
            WS_CHILD | WS_VISIBLE | SS_CENTER_STYLE,
            174,
            432,
            36,
            24,
            0,
        )?
    };

    for child in [hex_label, state.hex_edit, arrow] {
        if !font.is_null() {
            unsafe { SendMessageW(child, WM_SETFONT_CODE, font as usize, 1) };
        }
    }

    unsafe { sync_hex(state) };
    Ok(())
}

#[allow(clippy::too_many_arguments)]
unsafe fn create_child(
    parent: HWND,
    instance: windows_sys::Win32::Foundation::HINSTANCE,
    class_name: *const u16,
    text: *const u16,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
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
            x,
            y,
            width,
            height,
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
    unsafe {
        ReleaseCapture();
        DestroyWindow(hwnd);
    }
}

fn hit_test(state: &PickerState, x: i32, y: i32) -> DragTarget {
    if SV.contains(x, y) {
        DragTarget::SaturationValue
    } else if HUE.contains(x, y) {
        DragTarget::Hue
    } else if state.show_alpha && ALPHA.contains(x, y) {
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
            state.hsv.s = normalized(x - SV.x, SV.width);
            state.hsv.v = 1.0 - normalized(y - SV.y, SV.height);
            state.rebuild_rgb();
        }
        DragTarget::Hue => {
            state.hsv.h = (1.0 - normalized(y - HUE.y, HUE.height)).rem_euclid(1.0);
            state.rebuild_rgb();
        }
        DragTarget::Alpha => {
            state.current.a = (normalized(x - ALPHA.x, ALPHA.width) * 255.0).round() as u8;
        }
        DragTarget::None => return,
    }

    unsafe {
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
    unsafe { InvalidateRect(hwnd, core::ptr::null(), 0) };
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

unsafe fn render_picker(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    client: &RECT,
    state: &mut PickerState,
) {
    unsafe {
        fill_solid(hdc, client, BACKGROUND);
        paint_sv(hdc, state);
        paint_hue(hdc);

        if state.show_alpha {
            paint_alpha(hdc, state.current);
        } else {
            fill_solid(hdc, &ALPHA.rect(), Color::rgb(241, 242, 244));
            frame_light(hdc, ALPHA);
        }

        paint_preview(hdc, OLD_PREVIEW, state.initial);
        paint_preview(hdc, NEW_PREVIEW, state.current);
        draw_markers(hdc, state);
    }
}

unsafe fn paint_sv(hdc: windows_sys::Win32::Graphics::Gdi::HDC, state: &mut PickerState) {
    let width = SV.width as usize;
    let height = SV.height as usize;
    let required = width * height;
    let hue_changed = !state.sv_cache_hue.is_finite()
        || (state.sv_cache_hue - state.hsv.h).abs() > 0.000_001;

    if state.sv_pixels.len() != required {
        state.sv_pixels.resize(required, 0);
        state.sv_cache_hue = f32::NAN;
    }

    if hue_changed || !state.sv_cache_hue.is_finite() {
        for y in 0..height {
            let value = 1.0 - normalized(y as i32, SV.height);
            for x in 0..width {
                let saturation = normalized(x as i32, SV.width);
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
        draw_bitmap(hdc, SV, &state.sv_pixels);
        frame_light(hdc, SV);
    }
}

unsafe fn paint_hue(hdc: windows_sys::Win32::Graphics::Gdi::HDC) {
    let width = HUE.width as usize;
    let height = HUE.height as usize;
    let mut pixels = vec![0u32; width * height];

    for y in 0..height {
        let hue = (1.0 - normalized(y as i32, HUE.height)).rem_euclid(1.0);
        let color = Hsv {
            h: hue,
            s: 1.0,
            v: 1.0,
        }
        .to_color(255);
        let pixel = dib_pixel(color);
        for x in 0..width {
            pixels[y * width + x] = pixel;
        }
    }

    unsafe {
        draw_bitmap(hdc, HUE, &pixels);
        frame_light(hdc, HUE);
    }
}

unsafe fn paint_alpha(hdc: windows_sys::Win32::Graphics::Gdi::HDC, current: Color) {
    let width = ALPHA.width as usize;
    let height = ALPHA.height as usize;
    let mut pixels = vec![0u32; width * height];

    for y in 0..height {
        for x in 0..width {
            let alpha = (normalized(x as i32, ALPHA.width) * 255.0).round() as u8;
            let foreground = Color::rgba(current.r, current.g, current.b, alpha);
            let background = checker_color(x, y);
            pixels[y * width + x] = dib_pixel(blend_over(foreground, background));
        }
    }

    unsafe {
        draw_bitmap(hdc, ALPHA, &pixels);
        frame_light(hdc, ALPHA);
    }
}

unsafe fn paint_preview(hdc: windows_sys::Win32::Graphics::Gdi::HDC, area: Area, color: Color) {
    let width = area.width as usize;
    let height = area.height as usize;
    let mut pixels = vec![0u32; width * height];

    for y in 0..height {
        for x in 0..width {
            let background = checker_color(x, y);
            pixels[y * width + x] = dib_pixel(blend_over(color, background));
        }
    }

    unsafe {
        draw_bitmap(hdc, area, &pixels);
        frame_light(hdc, area);
    }
}

fn checker_color(x: usize, y: usize) -> Color {
    if ((x / 8) + (y / 8)).is_multiple_of(2) {
        CHECKER_LIGHT
    } else {
        CHECKER_DARK
    }
}

unsafe fn draw_bitmap(hdc: windows_sys::Win32::Graphics::Gdi::HDC, area: Area, pixels: &[u32]) {
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

unsafe fn draw_markers(hdc: windows_sys::Win32::Graphics::Gdi::HDC, state: &PickerState) {
    let sx = SV.x + (state.hsv.s * (SV.width - 1) as f32).round() as i32;
    let sy = SV.y + ((1.0 - state.hsv.v) * (SV.height - 1) as f32).round() as i32;

    let old_brush = unsafe { SelectObject(hdc, GetStockObject(NULL_BRUSH)) };
    let old_pen = unsafe { SelectObject(hdc, GetStockObject(BLACK_PEN)) };
    unsafe { Ellipse(hdc, sx - 10, sy - 10, sx + 11, sy + 11) };
    unsafe { SelectObject(hdc, GetStockObject(WHITE_PEN)) };
    unsafe { Ellipse(hdc, sx - 9, sy - 9, sx + 10, sy + 10) };
    unsafe {
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
    }

    let hue_y = HUE.y + ((1.0 - state.hsv.h) * (HUE.height - 1) as f32).round() as i32;
    let hue_marker = RECT {
        left: HUE.x - 2,
        top: hue_y - 2,
        right: HUE.x + HUE.width + 2,
        bottom: hue_y + 3,
    };
    unsafe { frame_dark(hdc, &hue_marker) };
    let hue_inner = RECT {
        left: HUE.x - 1,
        top: hue_y - 1,
        right: HUE.x + HUE.width + 1,
        bottom: hue_y + 2,
    };
    unsafe { frame_white(hdc, &hue_inner) };

    if state.show_alpha {
        let alpha_x =
            ALPHA.x + ((state.current.a as f32 / 255.0) * (ALPHA.width - 1) as f32).round() as i32;
        let alpha_marker = RECT {
            left: alpha_x - 2,
            top: ALPHA.y - 2,
            right: alpha_x + 3,
            bottom: ALPHA.y + ALPHA.height + 2,
        };
        unsafe { frame_dark(hdc, &alpha_marker) };
        let alpha_inner = RECT {
            left: alpha_x - 1,
            top: ALPHA.y - 1,
            right: alpha_x + 2,
            bottom: ALPHA.y + ALPHA.height + 1,
        };
        unsafe { frame_white(hdc, &alpha_inner) };
    }
}

unsafe fn fill_solid(hdc: windows_sys::Win32::Graphics::Gdi::HDC, rect: &RECT, color: Color) {
    let brush = unsafe { CreateSolidBrush(color.to_colorref()) };
    if brush.is_null() {
        return;
    }
    unsafe {
        FillRect(hdc, rect, brush);
        DeleteObject(brush as _);
    }
}

unsafe fn frame_light(hdc: windows_sys::Win32::Graphics::Gdi::HDC, area: Area) {
    let brush = unsafe { CreateSolidBrush(SURFACE_BORDER.to_colorref()) };
    if brush.is_null() {
        return;
    }
    unsafe {
        FrameRect(hdc, &area.rect(), brush);
        DeleteObject(brush as _);
    }
}

unsafe fn frame_dark(hdc: windows_sys::Win32::Graphics::Gdi::HDC, rect: &RECT) {
    let brush = unsafe { CreateSolidBrush(MARKER_DARK.to_colorref()) };
    if brush.is_null() {
        return;
    }
    unsafe {
        FrameRect(hdc, rect, brush);
        DeleteObject(brush as _);
    }
}

unsafe fn frame_white(hdc: windows_sys::Win32::Graphics::Gdi::HDC, rect: &RECT) {
    let brush = unsafe { CreateSolidBrush(Color::WHITE.to_colorref()) };
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

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(core::iter::once(0)).collect()
}
