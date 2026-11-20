#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_kb: u64,
}

#[cfg(windows)]
pub fn list_processes() -> Result<Vec<ProcessInfo>, String> {
    use std::mem::size_of;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ};

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| format!("CreateToolhelp32Snapshot failed: {e}"))?;

        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut processes = Vec::new();
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                let pid = entry.th32ProcessID;

                let memory_kb = if pid > 0 {
                    if let Ok(proc_handle) =
                        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, false, pid)
                    {
                        let mut counters = PROCESS_MEMORY_COUNTERS {
                            cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                            ..Default::default()
                        };
                        let kb = if GetProcessMemoryInfo(
                            proc_handle,
                            &mut counters,
                            size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                        )
                        .is_ok()
                        {
                            (counters.WorkingSetSize / 1024) as u64
                        } else {
                            0
                        };
                        let _ = CloseHandle(proc_handle);
                        kb
                    } else {
                        0
                    }
                } else {
                    0
                };

                processes.push(ProcessInfo {
                    pid,
                    name,
                    memory_kb,
                });

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
        Ok(processes)
    }
}

#[cfg(not(windows))]
pub fn list_processes() -> Result<Vec<ProcessInfo>, String> {
    Ok(Vec::new())
}

#[cfg(windows)]
pub fn kill_process(pid: u32) -> Result<(), String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
            .map_err(|e| format!("OpenProcess({pid}) failed: {e}"))?;
        let res = TerminateProcess(handle, 1)
            .map_err(|e| format!("TerminateProcess({pid}) failed: {e}"));
        let _ = CloseHandle(handle);
        res
    }
}

#[cfg(not(windows))]
pub fn kill_process(_pid: u32) -> Result<(), String> {
    Err("kill_process is only supported on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_info_fields_and_clone() {
        let info = ProcessInfo {
            pid: 1234,
            name: "chrome.exe".to_string(),
            memory_kb: 204800,
        };
        let cloned = info.clone();
        assert_eq!(cloned.pid, 1234);
        assert_eq!(cloned.name, "chrome.exe");
        assert_eq!(cloned.memory_kb, 204800);
    }
}
