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
    use windows::Win32::Foundation::{GlobalFree, HANDLE};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY, SW_HIDE, SW_SHOWNORMAL};

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
            let _ = AllowSetForegroundWindow(ASFW_ANY);
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

    /// Encodes a PowerShell script as Base64 UTF-16LE for `-EncodedCommand`.
    fn encode_ps_command(script: &str) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let utf16_bytes: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let mut out = String::with_capacity(utf16_bytes.len().div_ceil(3) * 4);
        for chunk in utf16_bytes.chunks(3) {
            let b0 = chunk[0];
            let b1 = *chunk.get(1).unwrap_or(&0);
            let b2 = *chunk.get(2).unwrap_or(&0);
            out.push(TABLE[(b0 >> 2) as usize] as char);
            out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            out.push(if chunk.len() > 1 {
                TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[(b2 & 0x3f) as usize] as char
            } else {
                '='
            });
        }
        out
    }

    pub fn execute_target(target: &str, mode: LaunchMode) -> Result<(), String> {
        match mode {
            LaunchMode::Normal => shell_execute("open", target, None, SW_SHOWNORMAL),
            LaunchMode::Elevated => shell_execute("runas", target, None, SW_SHOWNORMAL),
            LaunchMode::RevealInExplorer => {
                if target.contains('"') || target.contains('\0') {
                    return Err("Invalid characters in target path".to_string());
                }
                let args = format!("/select,\"{target}\"");
                shell_execute("open", "explorer.exe", Some(&args), SW_SHOWNORMAL)
            }
            LaunchMode::ShellHidden => {
                let encoded = encode_ps_command(target);
                let args = format!(
                    "-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand {encoded}"
                );
                shell_execute("open", "powershell.exe", Some(&args), SW_HIDE)
            }
            LaunchMode::ShellTerminal => {
                let encoded = encode_ps_command(target);
                let wt_args = format_wt_args(&encoded);
                if shell_execute("open", "wt.exe", Some(&wt_args), SW_SHOWNORMAL).is_ok() {
                    Ok(())
                } else {
                    let ps_args = format!("-NoExit -EncodedCommand {encoded}");
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
                    let _ = GlobalFree(hmem);
                    return Err("GlobalLock returned null".to_string());
                }
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr.cast::<u16>(), wide.len());
                let _ = GlobalUnlock(hmem);
                if let Err(e) = SetClipboardData(CF_UNICODETEXT_ID, HANDLE(hmem.0)) {
                    let _ = GlobalFree(hmem);
                    return Err(format!("SetClipboardData failed: {e}"));
                }
                Ok(())
            })();
            let _ = CloseClipboard();
            result
        }
    }

    pub fn system_power_action(action: &str) -> Result<(), String> {
        match action {
            "lock" => shell_execute("open", "rundll32.exe", Some("user32.dll,LockWorkStation"), SW_HIDE),
            "sleep" => unsafe {
                if !windows::Win32::System::Power::SetSuspendState(false, false, false).as_bool() {
                    return Err("SetSuspendState failed".to_string());
                }
                Ok(())
            },
            "restart" => shell_execute("open", "shutdown.exe", Some("/r /t 0"), SW_HIDE),
            "shutdown" => shell_execute("open", "shutdown.exe", Some("/s /t 0"), SW_HIDE),
            other => Err(format!("Unknown power action: {other}")),
        }
    }

    /// Formats Windows Terminal arguments with a `--` boundary and quoted `-EncodedCommand`.
    fn format_wt_args(encoded: &str) -> String {
        format!("-- powershell.exe -NoExit -EncodedCommand \"{encoded}\"")
    }

    #[allow(dead_code)]
    fn _unused(_p: PWSTR) {}

    #[cfg(test)]
    mod tests {
        use super::{encode_ps_command, format_wt_args};

        #[test]
        fn wt_args_include_double_dash_and_quoted_encoded_command() {
            let encoded = encode_ps_command("Get-ChildItem; --help");
            let args = format_wt_args(&encoded);
            assert!(args.starts_with("-- powershell.exe -NoExit -EncodedCommand \""));
            assert!(args.ends_with('"'));
            assert!(args.contains(&encoded));
        }

        #[test]
        fn reveal_in_explorer_rejects_quotes_and_nul() {
            use super::super::{execute_target, LaunchMode};
            let err_quote = execute_target(r#"C:\foo" /root,"\\evil\share"#, LaunchMode::RevealInExplorer);
            assert!(err_quote.is_err());
            let err_nul = execute_target("C:\\foo\0bar", LaunchMode::RevealInExplorer);
            assert!(err_nul.is_err());
        }
    }
}
