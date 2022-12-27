// Disable while debugging
// #![windows_subsystem = "windows"]
use std::{ffi::OsStr, sync::Mutex, thread::JoinHandle};
use std::{iter::once, sync::mpsc::Sender};
use std::{mem, sync::mpsc::Receiver};
use std::{os::windows::prelude::OsStrExt, sync::mpsc::channel};
use std::{ptr, time::Duration};
use windows::Win32::Foundation::{
    GetLastError, SetLastError, BOOL, HWND, LPARAM, LRESULT, POINT, PWSTR, RECT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, ClipCursor, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetClipCursor, GetCursorPos, GetMessageW, PostMessageW, PostQuitMessage, RegisterClassW,
    SetCursorPos, SetWindowsHookExW, /* ShowWindow, */ TranslateMessage, CW_USEDEFAULT, HMENU,
    KBDLLHOOKSTRUCT, /* SW_SHOW, */ WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP, WM_CLOSE, WM_DESTROY,
    WM_KEYUP, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_NULL, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{VK_F13, VK_PAUSE},
    Shell::{Shell_NotifyIconW, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW},
};
const TRAY_ICON_MESSAGE: u32 = WM_APP + 1;
const KEY_1: u32 = VK_F13 as u32;
const KEY_2: u32 = VK_PAUSE as u32;

static WAIT_SENDER: Mutex<Option<Sender<(i32, i32)>>> = Mutex::new(None);

struct Monitor {
    width: i32,
    height: i32,
}
const MONITOR_0: Monitor = Monitor {
    width: 2560,
    height: 1440,
};
const MONITOR_1: Monitor = Monitor {
    width: 1080,
    height: 1920,
};

fn win32_string(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(once(0)).collect()
}

unsafe extern "system" fn window_process(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // println!("{msg}, {w:x}, {l:x}", w = wParam.0, l = lParam.0);
    match msg {
        WM_CLOSE => {
            println!("Close Called");
            DestroyWindow(hwnd);
        }
        WM_DESTROY => {
            println!("Destroy Called");
            PostQuitMessage(0);
        }
        TRAY_ICON_MESSAGE => {
            if lparam.0 as u32 & 0xFFFF == WM_LBUTTONDOWN {
                println!("Icon Clicked");
                PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        _ => {}
    };
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn create_hidden_window() -> HWND {
    let name = "Mouse Fix Window";
    let title = "Mouse Fix";
    let hinstance = GetModuleHandleW(PWSTR(ptr::null_mut()));

    // Create "class" for window, using WNDCLASSW struct (different from Window our struct)
    let wnd_class = WNDCLASSW {
        lpfnWndProc: Some(window_process),
        hInstance: hinstance,
        lpszClassName: PWSTR(win32_string(name).as_mut_ptr()),
        ..Default::default()
    };

    RegisterClassW(&wnd_class);

    CreateWindowExW(
        0,
        PWSTR(win32_string(name).as_mut_ptr()),
        PWSTR(win32_string(title).as_mut_ptr()),
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        HWND(0),
        HMENU(0),
        hinstance,
        ptr::null_mut(),
    )
}

unsafe fn switch_screens() {
    let mut mouse_pos = mem::zeroed();
    GetCursorPos(&mut mouse_pos);
    ClipCursor(ptr::null());
    let new_x;
    let new_y;
    if mouse_pos.x >= MONITOR_0.width {
        let x_perc = (mouse_pos.x as f32 - MONITOR_0.width as f32) / (MONITOR_1.width as f32 - 1.);
        let y_perc = mouse_pos.y as f32 / (MONITOR_1.height as f32 - 1.);
        new_x = (x_perc * (MONITOR_0.width as f32 - 0.5)) as i32;
        new_y = (y_perc * (MONITOR_0.height as f32 - 0.5)) as i32;
    } else {
        let x_perc = mouse_pos.x as f32 / (MONITOR_0.width as f32 - 1.);
        let y_perc = mouse_pos.y as f32 / (MONITOR_0.height as f32 - 1.);
        new_x = (x_perc * (MONITOR_1.width as f32 - 0.5)) as i32 + MONITOR_0.width;
        new_y = (y_perc * (MONITOR_1.height as f32 - 0.5)) as i32;
    }
    if let Some(sender) = &*WAIT_SENDER.lock().unwrap() {
        sender.send((new_x, new_y)).unwrap();
    }
}

fn wait_move(recv: Receiver<(i32, i32)>) {
    unsafe {
        while let Ok((x, y)) = recv.recv() {
            // Necessary delay for some reason, don't remove
            std::thread::sleep(Duration::from_millis(10));
            SetCursorPos(x, y);
            set_clips(POINT { x, y });
        }
    }
}

unsafe fn set_clips(point: POINT) {
    let mut rect = mem::zeroed();
    GetClipCursor(&mut rect);
    if point.x >= MONITOR_0.width {
        if rect.left != MONITOR_0.width + 1
            || rect.top != 0
            || rect.right != MONITOR_0.width + MONITOR_1.width
            || rect.bottom != MONITOR_0.height + MONITOR_1.height
        {
            ClipCursor(&RECT {
                left: MONITOR_0.width,
                top: 0,
                right: MONITOR_0.width + MONITOR_1.width,
                bottom: MONITOR_0.height + MONITOR_1.height,
            });
        }
    } else if rect.left != 0
        || rect.top != 0
        || rect.right != MONITOR_0.width
        || rect.bottom != MONITOR_0.height
    {
        ClipCursor(&RECT {
            left: 0,
            top: 0,
            right: MONITOR_0.width,
            bottom: MONITOR_0.height,
        });
    }
}

unsafe extern "system" fn keyboard_callback(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode >= 0 {
        let mut point = mem::zeroed();
        GetCursorPos(&mut point);
        set_clips(point);
        let keyboard_struct = *mem::transmute::<_, &KBDLLHOOKSTRUCT>(lparam);
        if wparam.0 == WM_KEYUP as usize && keyboard_struct.vkCode == KEY_1 {
            switch_screens();
        }
        if wparam.0 == WM_KEYUP as usize && keyboard_struct.vkCode == KEY_2 {
            switch_screens();
        }
    }
    CallNextHookEx(None, ncode, wparam, lparam)
}

unsafe extern "system" fn mouse_callback(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode >= 0 {
        if wparam.0 != WM_MOUSEMOVE as usize {
            let mut point = mem::zeroed();
            GetCursorPos(&mut point);
            // println!("{point:?}");
            set_clips(point);
        }
    }
    CallNextHookEx(None, ncode, wparam, lparam)
}

fn set_sized_str(val: &str, arr: &mut [u16]) {
    let mut val = win32_string(val);
    val.resize(arr.len(), 0);
    arr.copy_from_slice(val.as_slice());
}

fn main() {
    let (send, recv) = channel();
    *WAIT_SENDER.lock().unwrap() = Some(send);
    let _ = std::thread::spawn(move || wait_move(recv));

    unsafe {
        let hwnd = create_hidden_window();

        let data = NOTIFYICONDATAW {
            cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
            uFlags: NIF_TIP | NIF_MESSAGE,
            uCallbackMessage: TRAY_ICON_MESSAGE,
            hWnd: hwnd,
            szTip: {
                let mut sz_tip = [0; 128];
                set_sized_str("Mouse Fix", &mut sz_tip);
                sz_tip
            },
            ..Default::default()
        };

        let _keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_callback), None, 0);
        let _mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_callback), None, 0);

        SetLastError(10);
        let mut x = 0;
        while !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
            let error = GetLastError();
            println!("{error}, 0x{error:x}");
            println!("Failed");
            x += 1;
            std::thread::sleep(std::time::Duration::from_secs(10));
            if x == 10 {
                PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                break;
            }
        }

        let mut msg = mem::zeroed();
        loop {
            let b = GetMessageW(&mut msg, hwnd, 0, 0);
            // println!("{msg:?}");
            if b != BOOL(1) {
                break;
            }
            TranslateMessage(&msg);
            match msg.message {
                WM_NULL => {
                    PostQuitMessage(-1);
                    println!("Null!");
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    println!("Quit!");
                }
                _ => (),
            };
            DispatchMessageW(&msg);
        }

        if !Shell_NotifyIconW(NIM_DELETE, &data).as_bool() {
            let error = GetLastError();
            println!("0x{error:X}, 0x{error:X}");
            println!("Failed");
        }

        ClipCursor(ptr::null());
    }
}
