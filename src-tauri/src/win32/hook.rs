use crossbeam_channel::Sender;

/// Virtual-key code for Left Windows key.
pub const VK_LWIN_CODE: u16 = 0x5B;
/// Virtual-key code for Right Windows key.
pub const VK_RWIN_CODE: u16 = 0x5C;
/// Unassigned virtual-key code injected on solo Win release to suppress Start Menu.
pub const VK_MASK_DUMMY: u16 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDecision {
    PassThrough,
    TriggerSoloWin,
}

/// Pure state machine for distinguishing solo `VK_LWIN` tap from `Win+*` combos.
#[derive(Debug, Default)]
pub struct WinKeyStateMachine {
    lwin_down: bool,
    combo_used: bool,
}

impl WinKeyStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Processes a low-level keyboard event and returns whether to pass through
    /// or trigger the launcher on solo `VK_LWIN` release.
    pub fn on_key_event(&mut self, vk: u16, is_key_down: bool, is_injected: bool) -> HookDecision {
        if is_injected {
            return HookDecision::PassThrough;
        }

        if vk == VK_LWIN_CODE {
            if is_key_down {
                if !self.lwin_down {
                    self.lwin_down = true;
                    self.combo_used = false;
                }
                return HookDecision::PassThrough;
            }

            let was_solo = self.lwin_down && !self.combo_used;
            self.lwin_down = false;
            self.combo_used = false;
            return if was_solo {
                HookDecision::TriggerSoloWin
            } else {
                HookDecision::PassThrough
            };
        }

        if self.lwin_down && is_key_down && vk != VK_RWIN_CODE {
            self.combo_used = true;
        }

        HookDecision::PassThrough
    }
}

/// Handle to the background `WH_KEYBOARD_LL` hook thread.
/// Dropping this handle posts `WM_QUIT` and waits for clean unhook.
pub struct WinKeyHookHandle {
    #[cfg(windows)]
    thread_id: u32,
    #[cfg(windows)]
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl WinKeyHookHandle {
    /// Starts the low-level keyboard hook on a dedicated thread.
    pub fn start(trigger_tx: Sender<()>) -> Result<Self, String> {
        #[cfg(windows)]
        {
            win_impl::start_hook_thread(trigger_tx)
        }
        #[cfg(not(windows))]
        {
            let _ = trigger_tx;
            Ok(Self {})
        }
    }
}

impl Drop for WinKeyHookHandle {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            win_impl::stop_hook_thread(self.thread_id, self.join_handle.take());
        }
    }
}

#[cfg(windows)]
mod win_impl {
    use super::{HookDecision, WinKeyHookHandle, WinKeyStateMachine, VK_MASK_DUMMY};
    use crossbeam_channel::Sender;
    use std::cell::RefCell;
    use std::sync::mpsc;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    struct HookContext {
        machine: WinKeyStateMachine,
        trigger_tx: Sender<()>,
    }

    thread_local! {
        static HOOK_CTX: RefCell<Option<HookContext>> = const { RefCell::new(None) };
    }

    unsafe extern "system" fn ll_keyboard_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let msg = wparam.0 as u32;
            let is_key_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let is_key_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;

            if is_key_down || is_key_up {
                let is_injected = (kb.flags & LLKHF_INJECTED) != Default::default();
                let decision = HOOK_CTX.with(|cell| {
                    let mut borrow = cell.borrow_mut();
                    if let Some(ctx) = borrow.as_mut() {
                        let d = ctx.machine.on_key_event(
                            kb.vkCode as u16,
                            is_key_down,
                            is_injected,
                        );
                        if d == HookDecision::TriggerSoloWin {
                            let _ = ctx.trigger_tx.try_send(());
                        }
                        d
                    } else {
                        HookDecision::PassThrough
                    }
                });

                if decision == HookDecision::TriggerSoloWin {
                    inject_mask_key();
                }
            }
        }

        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }

    /// Injects a harmless unassigned virtual key so Windows suppresses the Start Menu
    /// on solo `VK_LWIN` release without affecting `Win+*` combos.
    unsafe fn inject_mask_key() {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(VK_MASK_DUMMY),
                        wScan: 0,
                        dwFlags: Default::default(),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(VK_MASK_DUMMY),
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }

    pub fn start_hook_thread(trigger_tx: Sender<()>) -> Result<WinKeyHookHandle, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();

        let join_handle = std::thread::Builder::new()
            .name("launchify-win-hook".to_string())
            .spawn(move || unsafe {
                let tid = GetCurrentThreadId();
                HOOK_CTX.with(|cell| {
                    *cell.borrow_mut() = Some(HookContext {
                        machine: WinKeyStateMachine::new(),
                        trigger_tx,
                    });
                });

                let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), None, 0) {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!("SetWindowsHookExW failed: {e}")));
                        return;
                    }
                };

                let _ = ready_tx.send(Ok(tid));

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                let _ = UnhookWindowsHookEx(hook);
                HOOK_CTX.with(|cell| {
                    *cell.borrow_mut() = None;
                });
            })
            .map_err(|e| format!("Failed to spawn hook thread: {e}"))?;

        let thread_id = ready_rx
            .recv()
            .map_err(|e| format!("Hook thread failed to report readiness: {e}"))??;

        Ok(WinKeyHookHandle {
            thread_id,
            join_handle: Some(join_handle),
        })
    }

    pub fn stop_hook_thread(
        thread_id: u32,
        join_handle: Option<std::thread::JoinHandle<()>>,
    ) {
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(handle) = join_handle {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_lwin_tap_triggers_launcher() {
        let mut sm = WinKeyStateMachine::new();
        assert_eq!(
            sm.on_key_event(VK_LWIN_CODE, true, false),
            HookDecision::PassThrough
        );
        assert_eq!(
            sm.on_key_event(VK_LWIN_CODE, false, false),
            HookDecision::TriggerSoloWin
        );
    }

    #[test]
    fn win_e_combo_passes_through_without_trigger() {
        let mut sm = WinKeyStateMachine::new();
        assert_eq!(
            sm.on_key_event(VK_LWIN_CODE, true, false),
            HookDecision::PassThrough
        );
        assert_eq!(
            sm.on_key_event(b'E' as u16, true, false),
            HookDecision::PassThrough
        );
        assert_eq!(
            sm.on_key_event(b'E' as u16, false, false),
            HookDecision::PassThrough
        );
        assert_eq!(
            sm.on_key_event(VK_LWIN_CODE, false, false),
            HookDecision::PassThrough
        );
    }

    #[test]
    fn win_l_and_win_d_and_win_shift_s_pass_through() {
        let mut sm = WinKeyStateMachine::new();
        for key in [b'L' as u16, b'D' as u16] {
            sm.on_key_event(VK_LWIN_CODE, true, false);
            sm.on_key_event(key, true, false);
            sm.on_key_event(key, false, false);
            assert_eq!(
                sm.on_key_event(VK_LWIN_CODE, false, false),
                HookDecision::PassThrough
            );
        }

        // Win + Shift + S
        sm.on_key_event(VK_LWIN_CODE, true, false);
        sm.on_key_event(0xA0, true, false); // VK_LSHIFT
        sm.on_key_event(b'S' as u16, true, false);
        sm.on_key_event(b'S' as u16, false, false);
        sm.on_key_event(0xA0, false, false);
        assert_eq!(
            sm.on_key_event(VK_LWIN_CODE, false, false),
            HookDecision::PassThrough
        );
    }

    #[test]
    fn injected_events_are_ignored() {
        let mut sm = WinKeyStateMachine::new();
        assert_eq!(
            sm.on_key_event(VK_LWIN_CODE, true, true),
            HookDecision::PassThrough
        );
        assert_eq!(
            sm.on_key_event(VK_LWIN_CODE, false, true),
            HookDecision::PassThrough
        );
    }
}
