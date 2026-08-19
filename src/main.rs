#![windows_subsystem = "windows"]
//! La Meuh — gestionnaire de mises à jour Windows (winget), en un clic.
//! Réécriture Rust de l'original en C++/Win32 (voir /home/user/La_meuh).

mod app;
mod process;
mod resources;
mod winget;

use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{UpdateWindow, COLOR_BTNFACE};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use app::{wndproc, AppState};
use resources::IDI_ICON1;

fn main() {
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);

        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("LaMeuhWindowClass");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hIcon: LoadIconW(hinstance, resource_id(IDI_ICON1)).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(
                (COLOR_BTNFACE.0 + 1) as isize as *mut _,
            ),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }

        let state = Box::new(AppState::new());
        let state_ptr = Box::into_raw(state);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("La Meuh - Mises à jour"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            450,
            320,
            None,
            None,
            hinstance,
            Some(state_ptr as *const std::ffi::c_void),
        );

        let hwnd: HWND = match hwnd {
            Ok(h) => h,
            Err(_) => {
                drop(Box::from_raw(state_ptr));
                return;
            }
        };

        let _ = ShowWindow(hwnd, SW_SHOWDEFAULT);
        let _ = UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe fn resource_id(id: u16) -> windows::core::PCWSTR {
    windows::core::PCWSTR(id as usize as *const u16)
}
