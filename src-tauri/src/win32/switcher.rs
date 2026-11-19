/// Metadata for a visible top-level window returned by the `@` window switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub exe_path: Option<String>,
}

/// Enumerates visible, non-cloaked, non-tool top-level windows.
pub fn enumerate_visible_windows() -> Vec<WindowInfo> {
    #[cfg(windows)]
    {
        win_impl::enumerate_visible_windows()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Focuses a window by `HWND`, restoring it first if minimized.
pub fn focus_window(hwnd: isize) -> Result<(), String> {
    #[cfg(windows)]
    {
        win_impl::focus_window(hwnd)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        Ok(())
    }
}

#[cfg(windows)]
mod win_impl {
    use super::WindowInfo;
    use std::path::Path;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, EnumWindows, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow,
        GWL_EXSTYLE, SW_RESTORE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> windows::Win32::Foundation::BOOL {
        if !IsWindowVisible(hwnd).as_bool() {
            return true.into();
        }

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if (ex_style & WS_EX_TOOLWINDOW.0) != 0 && (ex_style & WS_EX_APPWINDOW.0) == 0 {
            return true.into();
        }

        let mut cloaked: u32 = 0;
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        );
        if hr.is_err() || cloaked != 0 {
            return true.into();
        }

        let title_len = GetWindowTextLengthW(hwnd);
        if title_len <= 0 {
            return true.into();
        }

        let mut buf = vec![0u16; (title_len + 1) as usize];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied <= 0 {
            return true.into();
        }
        let title = String::from_utf16_lossy(&buf[..copied as usize]);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let (process_name, exe_path) = if pid != 0 {
            query_process_info(pid)
        } else {
            (String::new(), None)
        };

        let list = &mut *(lparam.0 as *mut Vec<WindowInfo>);
        list.push(WindowInfo {
            hwnd: hwnd.0 as isize,
            title,
            process_name,
            exe_path,
        });

        true.into()
    }

    unsafe fn query_process_info(pid: u32) -> (String, Option<String>) {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return (String::new(), None);
        };

        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(handle);

        if !ok || len == 0 {
            return (String::new(), None);
        }

        let full = String::from_utf16_lossy(&buf[..len as usize]);
        let name = Path::new(&full)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        (name, Some(full))
    }

    pub fn enumerate_visible_windows() -> Vec<WindowInfo> {
        let mut list: Vec<WindowInfo> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(enum_proc),
                LPARAM((&mut list as *mut Vec<WindowInfo>) as isize),
            );
        }
        list
    }

    pub fn focus_window(hwnd_raw: isize) -> Result<(), String> {
        if hwnd_raw == 0 {
            return Err("Invalid window handle (0)".to_string());
        }
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let cur_thread = GetCurrentThreadId();
            let target_thread = GetWindowThreadProcessId(hwnd, None);
            let attach = target_thread != 0 && target_thread != cur_thread;
            if attach {
                let _ = AttachThreadInput(cur_thread, target_thread, true);
            }
            let _ = BringWindowToTop(hwnd);
            let ok = SetForegroundWindow(hwnd).as_bool();
            if attach {
                let _ = AttachThreadInput(cur_thread, target_thread, false);
            }
            if !ok {
                return Err(format!("SetForegroundWindow failed for hwnd {hwnd_raw}"));
            }
        }
        Ok(())
    }
}
