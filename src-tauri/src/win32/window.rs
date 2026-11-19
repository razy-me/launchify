/// Physical-pixel monitor work area rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl MonitorRect {
    pub fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }
}

/// Computes the physical-pixel `(x, y)` top-left position for the launcher window:
/// horizontally centered on `work_area` and vertically anchored at 28% from the top.
pub fn compute_launcher_position(
    work_area: MonitorRect,
    logical_width: i32,
    dpi_scale: f64,
) -> (i32, i32) {
    let scale = if dpi_scale.is_finite() && dpi_scale > 0.0 {
        dpi_scale
    } else {
        1.0
    };
    let phys_width = ((logical_width as f64) * scale).round() as i32;
    let x = work_area.left + ((work_area.width() - phys_width) / 2).max(0);
    let y = work_area.top + ((work_area.height() as f64) * 0.28).round() as i32;
    (x, y.max(work_area.top))
}

/// Applies `WS_EX_TOOLWINDOW` so the launcher never appears in the taskbar or Alt+Tab.
pub fn apply_toolwindow_style(hwnd: isize) -> Result<(), String> {
    #[cfg(windows)]
    {
        win_impl::apply_toolwindow_style(hwnd)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        Ok(())
    }
}

/// Saves the currently focused foreground window handle (if any).
pub fn save_foreground_window() -> Option<isize> {
    #[cfg(windows)]
    {
        win_impl::save_foreground_window()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Restores focus to a previously saved foreground window handle.
pub fn restore_foreground_window(hwnd: isize) -> Result<(), String> {
    #[cfg(windows)]
    {
        win_impl::restore_foreground_window(hwnd)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        Ok(())
    }
}

/// Positions the launcher on the cursor's active monitor at 28% vertical anchor,
/// shows it, and acquires clean keyboard focus.
pub fn show_and_focus_launcher(hwnd: isize, logical_width: i32) -> Result<(), String> {
    #[cfg(windows)]
    {
        win_impl::show_and_focus_launcher(hwnd, logical_width)
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, logical_width);
        Ok(())
    }
}

/// Hides the launcher window with `SW_HIDE` to preserve warm state.
pub fn hide_launcher_window(hwnd: isize) {
    #[cfg(windows)]
    {
        win_impl::hide_launcher_window(hwnd);
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
    }
}

#[cfg(windows)]
mod win_impl {
    use super::{compute_launcher_position, MonitorRect};
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW,
        GetWindowThreadProcessId, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, GWL_EXSTYLE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW,
        WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    fn as_hwnd(raw: isize) -> HWND {
        HWND(raw as *mut std::ffi::c_void)
    }

    pub fn apply_toolwindow_style(hwnd_raw: isize) -> Result<(), String> {
        let hwnd = as_hwnd(hwnd_raw);
        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let new_style = (ex_style | (WS_EX_TOOLWINDOW.0 as isize)) & !(WS_EX_APPWINDOW.0 as isize);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        }
        Ok(())
    }

    pub fn save_foreground_window() -> Option<isize> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            None
        } else {
            Some(hwnd.0 as isize)
        }
    }

    pub fn restore_foreground_window(hwnd_raw: isize) -> Result<(), String> {
        if hwnd_raw == 0 {
            return Ok(());
        }
        let hwnd = as_hwnd(hwnd_raw);
        unsafe {
            let fg = GetForegroundWindow();
            let fg_tid = if !fg.0.is_null() {
                GetWindowThreadProcessId(fg, None)
            } else {
                0
            };
            let cur_tid = GetCurrentThreadId();
            let attached = fg_tid != 0 && fg_tid != cur_tid && AttachThreadInput(cur_tid, fg_tid, true).as_bool();
            let _ = SetForegroundWindow(hwnd);
            if attached {
                let _ = AttachThreadInput(cur_tid, fg_tid, false);
            }
        }
        Ok(())
    }

    pub fn show_and_focus_launcher(hwnd_raw: isize, logical_width: i32) -> Result<(), String> {
        let hwnd = as_hwnd(hwnd_raw);
        unsafe {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let work = MonitorRect {
                    left: info.rcWork.left,
                    top: info.rcWork.top,
                    right: info.rcWork.right,
                    bottom: info.rcWork.bottom,
                };
                let dpi = GetDpiForWindow(hwnd);
                let scale = if dpi > 0 { dpi as f64 / 96.0 } else { 1.0 };
                let (x, y) = compute_launcher_position(work, logical_width, scale);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW,
                );
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }

            let fg = GetForegroundWindow();
            let fg_tid = if !fg.0.is_null() {
                GetWindowThreadProcessId(fg, None)
            } else {
                0
            };
            let cur_tid = GetCurrentThreadId();
            let attached = fg_tid != 0 && fg_tid != cur_tid && AttachThreadInput(cur_tid, fg_tid, true).as_bool();
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            if attached {
                let _ = AttachThreadInput(cur_tid, fg_tid, false);
            }
        }
        Ok(())
    }

    pub fn hide_launcher_window(hwnd_raw: isize) {
        let hwnd = as_hwnd(hwnd_raw);
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_horizontally_and_anchors_at_28_percent_at_1x_dpi() {
        let work = MonitorRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let (x, y) = compute_launcher_position(work, 680, 1.0);
        assert_eq!(x, (1920 - 680) / 2);
        assert_eq!(y, (1040.0 * 0.28_f64).round() as i32);
    }

    #[test]
    fn accounts_for_multi_monitor_offset_and_high_dpi() {
        let work = MonitorRect {
            left: 1920,
            top: 100,
            right: 4480, // 2560px wide
            bottom: 1500, // 1400px tall
        };
        // 1.5x DPI -> 680 * 1.5 = 1020px physical width
        let (x, y) = compute_launcher_position(work, 680, 1.5);
        assert_eq!(x, 1920 + (2560 - 1020) / 2);
        assert_eq!(y, 100 + (1400.0 * 0.28_f64).round() as i32);
    }
}
