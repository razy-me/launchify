/// Execution mode for launching targets via Win32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    Normal,
    Elevated,
    RevealInExplorer,
    ShellHidden,
    ShellTerminal,
}

/// Executes a target path, URI, or shell command using the specified `LaunchMode`.
pub fn execute_target(target: &str, mode: LaunchMode) -> Result<(), String> {
    if target.trim().is_empty() {
        return Err("Cannot execute empty target".to_string());
    }
    #[cfg(windows)]
    {
        win_impl::execute_target(target, mode)
    }
    #[cfg(not(windows))]
    {
        let _ = (target, mode);
        Ok(())
    }
}

/// Copies UTF-8 text to the Windows clipboard (`CF_UNICODETEXT`).
pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        win_impl::copy_text_to_clipboard(text)
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        Ok(())
    }
}

/// Executes a built-in system power action (`lock`, `sleep`, `restart`, `shutdown`).
pub fn system_power_action(action: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        win_impl::system_power_action(action)
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        Ok(())
    }
}

#[cfg(windows)]
mod win_impl {
    use super::LaunchMode;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};

    const CF_UNICODETEXT_ID: u32 = 13;

    fn to_wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn shell_execute(
        verb: &str,
        file: &str,
        params: Option<&str>,
        show_cmd: windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD,
    ) -> Result<(), String> {
        let verb_w = to_wide_null(verb);
        let file_w = to_wide_null(file);
        let params_w = params.map(to_wide_null);

        let hinst = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb_w.as_ptr()),
                PCWSTR(file_w.as_ptr()),
                params_w
                    .as_ref()
                    .map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr())),
                PCWSTR::null(),
                show_cmd,
            )
        };
        // ShellExecuteW returns > 32 on success.
        if (hinst.0 as isize) <= 32 {
            return Err(format!("ShellExecuteW failed with code {}", hinst.0 as isize));
        }
        Ok(())
    }

    pub fn execute_target(target: &str, mode: LaunchMode) -> Result<(), String> {
        match mode {
            LaunchMode::Normal => shell_execute("open", target, None, SW_SHOWNORMAL),
            LaunchMode::Elevated => shell_execute("runas", target, None, SW_SHOWNORMAL),
            LaunchMode::RevealInExplorer => {
                let args = format!("/select,\"{target}\"");
                shell_execute("open", "explorer.exe", Some(&args), SW_SHOWNORMAL)
            }
            LaunchMode::ShellHidden => {
                let args = format!(
                    "-NoProfile -NonInteractive -WindowStyle Hidden -Command \"{target}\""
                );
                shell_execute("open", "powershell.exe", Some(&args), SW_HIDE)
            }
            LaunchMode::ShellTerminal => {
                let wt_args = format!("powershell.exe -NoExit -Command \"{target}\"");
                if shell_execute("open", "wt.exe", Some(&wt_args), SW_SHOWNORMAL).is_ok() {
                    Ok(())
                } else {
                    let ps_args = format!("-NoExit -Command \"{target}\"");
                    shell_execute("open", "powershell.exe", Some(&ps_args), SW_SHOWNORMAL)
                }
            }
        }
    }

    pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
        let wide = to_wide_null(text);
        let byte_len = wide.len() * std::mem::size_of::<u16>();

        unsafe {
            OpenClipboard(None).map_err(|e| format!("OpenClipboard failed: {e}"))?;
            let result = (|| {
                EmptyClipboard().map_err(|e| format!("EmptyClipboard failed: {e}"))?;
                let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len)
                    .map_err(|e| format!("GlobalAlloc failed: {e}"))?;
                let ptr = GlobalLock(hmem);
                if ptr.is_null() {
                    return Err("GlobalLock returned null".to_string());
                }
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr.cast::<u16>(), wide.len());
                let _ = GlobalUnlock(hmem);
                SetClipboardData(CF_UNICODETEXT_ID, HANDLE(hmem.0))
                    .map_err(|e| format!("SetClipboardData failed: {e}"))?;
                Ok(())
            })();
            let _ = CloseClipboard();
            result
        }
    }

    pub fn system_power_action(action: &str) -> Result<(), String> {
        match action {
            "lock" => shell_execute("open", "rundll32.exe", Some("user32.dll,LockWorkStation"), SW_HIDE),
            "sleep" => shell_execute(
                "open",
                "rundll32.exe",
                Some("powrprof.dll,SetSuspendState 0,1,0"),
                SW_HIDE,
            ),
            "restart" => shell_execute("open", "shutdown.exe", Some("/r /t 0"), SW_HIDE),
            "shutdown" => shell_execute("open", "shutdown.exe", Some("/s /t 0"), SW_HIDE),
            other => Err(format!("Unknown power action: {other}")),
        }
    }

    #[allow(dead_code)]
    fn _unused(_p: PWSTR) {}
}
