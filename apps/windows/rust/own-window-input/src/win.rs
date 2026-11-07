//! Win32 side of the own-window sink. All `unsafe` in this crate lives here.

use crate::{prepare, OpenError, Outcome};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::FlashWindowEx;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetWindowTextLengthW, GetWindowTextW, PeekMessageW, RegisterClassW, SetForegroundWindow,
    SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage, CW_USEDEFAULT, ES_AUTOHSCROLL,
    ES_PASSWORD, FLASHWINFO, FLASHW_ALL, FLASHW_TIMERNOFG, HWND_TOPMOST, MSG, PM_REMOVE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOW, WINDOW_STYLE, WNDCLASSW, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use zeroize::Zeroizing;

/// How long a delivery waits for the user to have the test window active before giving up.
const FOREGROUND_GRACE: Duration = Duration::from_secs(20);
/// Time allowed for queued input to reach the edit box before it is read back.
const SETTLE: Duration = Duration::from_millis(700);

enum Cmd {
    Deliver(Zeroizing<Vec<u16>>, Sender<Outcome>),
    Close,
}

/// A window owned by this program with one edit box. Created on its own thread so BLE work on the
/// caller's thread never freezes it.
pub struct OwnWindowSink {
    cmds: Sender<Cmd>,
    thread: Option<JoinHandle<()>>,
}

impl OwnWindowSink {
    /// Open the test window. With `show_text` false the box masks what is typed (default for demos);
    /// the sink still verifies the real content by reading the box back inside this process.
    pub fn open(show_text: bool) -> Result<Self, OpenError> {
        let (cmds, rx) = channel();
        let (ready_tx, ready_rx) = channel();
        let thread = std::thread::Builder::new()
            .name("kagilink-own-window".into())
            .spawn(move || window_thread(show_text, ready_tx, rx))
            .map_err(|e| OpenError::Window(format!("thread: {e}")))?;
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                cmds,
                thread: Some(thread),
            }),
            Ok(Err(step)) => {
                let _ = thread.join();
                Err(OpenError::Window(step))
            }
            Err(_) => Err(OpenError::Window("window thread did not start".into())),
        }
    }

    /// Type a received value into the window if (and only if) it is the foreground window with focus.
    /// Blocks until the outcome is known. The value is validated first and wiped after use.
    pub fn deliver(&self, plaintext: &[u8]) -> Outcome {
        let text = match prepare(plaintext) {
            Ok(t) => t,
            Err(o) => return o,
        };
        let (reply_tx, reply_rx) = channel();
        if self.cmds.send(Cmd::Deliver(text, reply_tx)).is_err() {
            return Outcome::WindowGone;
        }
        reply_rx
            .recv_timeout(FOREGROUND_GRACE + SETTLE + Duration::from_secs(5))
            .unwrap_or(Outcome::WindowGone)
    }
}

impl Drop for OwnWindowSink {
    fn drop(&mut self) {
        let _ = self.cmds.send(Cmd::Close);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: forwards every message to the default procedure unchanged.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn pump(ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    loop {
        // SAFETY: standard message loop on the thread that created the windows.
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if Instant::now() >= end {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_text(hwnd: HWND, s: &str) {
    let w = wide(s);
    // SAFETY: `w` is a NUL-terminated UTF-16 buffer that outlives the call; hwnd belongs to this thread.
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(w.as_ptr()));
    }
}

fn unicode_events(text: &[u16]) -> Vec<INPUT> {
    let mut v = Vec::with_capacity(text.len() * 2);
    for &unit in text {
        for up in [false, true] {
            v.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: unit,
                        dwFlags: KEYEVENTF_UNICODE
                            | if up {
                                KEYEVENTF_KEYUP
                            } else {
                                Default::default()
                            },
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });
        }
    }
    v
}

fn window_thread(show_text: bool, ready: Sender<Result<(), String>>, cmds: Receiver<Cmd>) {
    // SAFETY: creates windows owned by this thread, uses them only here, and destroys them before returning.
    unsafe {
        let hinst = match GetModuleHandleW(PCWSTR::null()) {
            Ok(h) => h,
            Err(e) => {
                let _ = ready.send(Err(format!("module handle: {e}")));
                return;
            }
        };
        let class = w!("KagiLinkOwnWindowInput");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            lpszClassName: class,
            ..Default::default()
        };
        // A second sink in the same process re-registers the class; that failing is harmless.
        let _ = RegisterClassW(&wc);
        let hwnd = match CreateWindowExW(
            Default::default(),
            class,
            w!("KagiLink test window (synthetic data only)"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            150,
            None,
            None,
            Some(hinst.into()),
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                let _ = ready.send(Err(format!("window: {e}")));
                return;
            }
        };
        let label = CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("Click the box below, then press Send on the phone."),
            WS_CHILD | WS_VISIBLE,
            10,
            10,
            480,
            20,
            Some(hwnd),
            None,
            Some(hinst.into()),
            None,
        )
        .unwrap_or_default();
        let mut style = WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32);
        if !show_text {
            style |= WINDOW_STYLE(ES_PASSWORD as u32);
        }
        let edit = match CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            w!(""),
            style,
            10,
            40,
            480,
            26,
            Some(hwnd),
            None,
            Some(hinst.into()),
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                let _ = DestroyWindow(hwnd);
                let _ = ready.send(Err(format!("edit control: {e}")));
                return;
            }
        };
        let _ = ShowWindow(hwnd, SW_SHOW);
        // Keep the test window above other windows so it is not lost behind them (no activation).
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        // Windows may refuse to bring a background program forward; this does not work around that.
        let _ = SetForegroundWindow(hwnd);
        let _ = ready.send(Ok(()));

        loop {
            pump(0);
            match cmds.recv_timeout(Duration::from_millis(10)) {
                Ok(Cmd::Deliver(text, reply)) => {
                    let outcome = deliver_one(hwnd, edit, label, &text);
                    let _ = reply.send(outcome);
                }
                Ok(Cmd::Close) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        set_text(edit, "");
        let _ = DestroyWindow(hwnd);
    }
}

/// SAFETY: must be called on the thread that created `hwnd`, `edit` and `label`.
unsafe fn deliver_one(hwnd: HWND, edit: HWND, label: HWND, text: &[u16]) -> Outcome {
    // SAFETY: Win32 calls on windows owned by this thread; no pointer outlives its call.
    unsafe {
        if GetForegroundWindow() != hwnd {
            // A value is waiting: make the window easy to find and say what to do. Flashing and a
            // topmost z-order do not take the foreground; only the user's click does.
            set_text(
                label,
                "A VALUE ARRIVED: CLICK THE BOX BELOW NOW (waiting 20 s).",
            );
            let fi = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd,
                dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
                uCount: 0,
                dwTimeout: 0,
            };
            let _ = FlashWindowEx(&fi);
        }
        let deadline = Instant::now() + FOREGROUND_GRACE;
        while GetForegroundWindow() != hwnd && Instant::now() < deadline {
            pump(50);
        }
        if GetForegroundWindow() != hwnd {
            set_text(
                label,
                "Not typed: this window was not active. Click the box and send again.",
            );
            return Outcome::NotForeground;
        }
        let _ = SetFocus(Some(edit));
        pump(100);
        if GetForegroundWindow() != hwnd || GetFocus() != edit {
            set_text(label, "Not typed: the box did not have the keyboard focus.");
            return Outcome::NotForeground;
        }
        set_text(edit, "");
        let events = unicode_events(text);
        let queued = SendInput(&events, std::mem::size_of::<INPUT>() as i32);
        pump(SETTLE.as_millis() as u64);
        if queued as usize != events.len() {
            set_text(edit, "");
            set_text(label, "Blocked: Windows did not accept all input events.");
            return Outcome::Blocked;
        }
        let len = GetWindowTextLengthW(edit).max(0) as usize;
        let mut buf = Zeroizing::new(vec![0u16; len + 2]);
        let n = GetWindowTextW(edit, &mut buf[..]).max(0) as usize;
        if buf[..n] == text[..] {
            set_text(
                label,
                &format!(
                    "Delivered {} characters (read back from this box).",
                    text.len()
                ),
            );
            Outcome::Delivered { chars: n }
        } else {
            set_text(edit, "");
            set_text(
                label,
                "Mismatch: the box did not contain the sent value; cleared.",
            );
            Outcome::Mismatch
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_is_either_verified_or_refused_and_never_blind() {
        // A headless CI session may or may not give the new window the foreground. Either result is
        // acceptable; what must never happen is Blocked/Mismatch for an ordinary value, or a false
        // "Delivered". The text is only ever typed while our own window is foreground (checked in code).
        let sink = OwnWindowSink::open(false).expect("window opens");
        match sink.deliver(b"synthetic-test-value") {
            Outcome::Delivered { chars } => assert_eq!(chars, 20),
            Outcome::NotForeground => {}
            other => panic!("unexpected outcome {other:?}"),
        }
    }

    #[test]
    fn invalid_values_are_refused_before_any_window_work() {
        let sink = OwnWindowSink::open(false).expect("window opens");
        assert_eq!(sink.deliver(b"a\nb"), Outcome::NotText);
        assert_eq!(sink.deliver(&[0xff]), Outcome::NotText);
    }
}
