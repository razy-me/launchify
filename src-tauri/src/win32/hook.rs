use crossbeam_channel::Sender;

/// Virtual-key code for Left Windows key (re-export compatibility).
pub const VK_LWIN_CODE: u16 = 0x5B;
/// Virtual-key code for Left Alt key (`VK_LMENU`).
pub const VK_TRIGGER_CODE: u16 = 0xA4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDecision {
    PassThrough,
    Trigger,
}

/// Pure state machine for detecting a solo tap of the Left Alt key (`VK_LMENU`)
/// while leaving `Alt+*` shortcuts untouched.
#[derive(Debug, Default)]
pub struct WinKeyStateMachine {
    trigger_down: bool,
    other_key_pressed: bool,
}

impl WinKeyStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Processes a low-level keyboard event and returns whether to pass through
    /// or trigger the launcher toggle on a solo `VK_LMENU` tap.
    pub fn on_key_event(&mut self, vk: u16, is_key_down: bool, is_injected: bool) -> HookDecision {
        if is_injected {
            return HookDecision::PassThrough;
        }

        if vk == VK_TRIGGER_CODE {
            if is_key_down {
                if !self.trigger_down {
                    self.trigger_down = true;
                    self.other_key_pressed = false;
                }
                return HookDecision::PassThrough;
            } else {
                let should_trigger = self.trigger_down && !self.other_key_pressed;
                self.trigger_down = false;
                self.other_key_pressed = false;
                return if should_trigger {
                    HookDecision::Trigger
                } else {
                    HookDecision::PassThrough
                };
            }
        }

        if self.trigger_down && is_key_down {
            self.other_key_pressed = true;
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
    use super::{HookDecision, WinKeyHookHandle, WinKeyStateMachine};
    use crossbeam_channel::Sender;
    use std::cell::RefCell;
    use std::sync::mpsc;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
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
                let vk = kb.vkCode as u16;
                HOOK_CTX.with(|cell| {
                    let mut borrow = cell.borrow_mut();
                    if let Some(ctx) = borrow.as_mut() {
                        let d = ctx.machine.on_key_event(vk, is_key_down, is_injected);
                        if d == HookDecision::Trigger {
                            let _ = ctx.trigger_tx.try_send(());
                        }
                    }
                });
            }
        }

        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }

    pub fn start_hook_thread(trigger_tx: Sender<()>) -> Result<WinKeyHookHandle, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();

        let join_handle = std::thread::Builder::new()
            .name("launchify-kbd-hook".into())
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
    fn solo_lalt_tap_triggers_launcher() {
        let mut sm = WinKeyStateMachine::new();
        assert_eq!(sm.on_key_event(VK_TRIGGER_CODE, true, false), HookDecision::PassThrough);
        assert_eq!(sm.on_key_event(VK_TRIGGER_CODE, true, false), HookDecision::PassThrough);
        assert_eq!(sm.on_key_event(VK_TRIGGER_CODE, false, false), HookDecision::Trigger);
    }

    #[test]
    fn lalt_combo_passes_through_without_trigger() {
        let mut sm = WinKeyStateMachine::new();
        assert_eq!(sm.on_key_event(VK_TRIGGER_CODE, true, false), HookDecision::PassThrough);
        assert_eq!(sm.on_key_event(b'C' as u16, true, false), HookDecision::PassThrough);
        assert_eq!(sm.on_key_event(b'C' as u16, false, false), HookDecision::PassThrough);
        assert_eq!(sm.on_key_event(VK_TRIGGER_CODE, false, false), HookDecision::PassThrough);
    }

    #[test]
    fn injected_events_are_ignored() {
        let mut sm = WinKeyStateMachine::new();
        assert_eq!(sm.on_key_event(VK_TRIGGER_CODE, true, true), HookDecision::PassThrough);
        assert_eq!(sm.on_key_event(VK_TRIGGER_CODE, false, true), HookDecision::PassThrough);
    }
}
