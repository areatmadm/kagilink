//! Own-window synthetic input self-test (feasibility only, synthetic text only).
//!
//! It creates ONE small window with an edit box, makes sure that window is the foreground window
//! and holds the keyboard focus, sends a fixed harmless test string with SendInput (Unicode), and
//! reads the text back from the edit box. If the window is not foreground at the moment of sending,
//! nothing is sent (so the text can never land in another program). It does not touch other
//! windows, elevated windows, credential dialogs, the secure desktop or Remote Desktop, and it
//! never handles a real secret. SendInput's return value only says events were queued; the
//! read-back from our own edit box is the evidence of delivery to our own window.
//! Opt-in: it does nothing unless started with `--send-to-own-window`.

#[cfg(not(windows))]
fn main() {
    println!("kagilink-input-probe: Windows only.");
}

#[cfg(windows)]
fn main() {
    if !std::env::args().any(|a| a == "--send-to-own-window") {
        println!("usage: kagilink-input-probe --send-to-own-window");
        println!("Sends a fixed harmless test string into a window this program creates itself.");
        std::process::exit(64);
    }
    std::process::exit(selftest::run());
}

#[cfg(windows)]
mod selftest {
    use std::time::{Duration, Instant};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
        GetWindowTextW, PeekMessageW, RegisterClassW, SetForegroundWindow, ShowWindow,
        TranslateMessage, CW_USEDEFAULT, ES_AUTOHSCROLL, MSG, PM_REMOVE, SW_SHOW, WNDCLASSW,
        WS_CHILD, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };

    const TEST_TEXT: &str = "kagilink-input-selftest-0123";
    const WAIT_SECONDS: u64 = 20;

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        // SAFETY: forwards every message to the default procedure unchanged.
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    fn pump(ms: u64) {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            // SAFETY: standard message loop on the thread that created the windows.
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn unicode_events(text: &str) -> Vec<INPUT> {
        let mut v = Vec::new();
        for unit in text.encode_utf16() {
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

    pub fn run() -> i32 {
        // SAFETY: creates two windows owned by this thread, uses them, and destroys them.
        unsafe {
            let hinst = match GetModuleHandleW(PCWSTR::null()) {
                Ok(h) => h,
                Err(e) => {
                    println!("RESULT: ERROR (module handle: {e})");
                    return 1;
                }
            };
            let class = w!("KagiLinkInputSelfTest");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinst.into(),
                lpszClassName: class,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                println!("RESULT: ERROR (window class)");
                return 1;
            }
            let hwnd = match CreateWindowExW(
                Default::default(),
                class,
                w!("KagiLink input self-test (test text only)"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                480,
                120,
                None,
                None,
                Some(hinst.into()),
                None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    println!("RESULT: ERROR (window: {e})");
                    return 1;
                }
            };
            let edit = match CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                w!(""),
                WS_CHILD
                    | WS_VISIBLE
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                10,
                10,
                440,
                26,
                Some(hwnd),
                None,
                Some(hinst.into()),
                None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    let _ = DestroyWindow(hwnd);
                    println!("RESULT: ERROR (edit control: {e})");
                    return 1;
                }
            };
            let _ = ShowWindow(hwnd, SW_SHOW);
            // Windows does not let a background program take the foreground by itself (and this
            // probe does not try to work around that). Ask for a click and wait for it.
            let _ = SetForegroundWindow(hwnd);
            println!(
                "Click the small test window within {WAIT_SECONDS} seconds to make it active ..."
            );
            let deadline = Instant::now() + Duration::from_secs(WAIT_SECONDS);
            while GetForegroundWindow() != hwnd && Instant::now() < deadline {
                pump(100);
            }
            if GetForegroundWindow() != hwnd {
                let _ = DestroyWindow(hwnd);
                println!(
                    "RESULT: INCONCLUSIVE (the test window never became the foreground window, so                      NOTHING was sent)"
                );
                return 2;
            }
            let _ = SetFocus(Some(edit));
            pump(300);
            if GetForegroundWindow() != hwnd {
                let _ = DestroyWindow(hwnd);
                println!("RESULT: INCONCLUSIVE (focus left the test window; NOTHING was sent)");
                return 2;
            }

            let events = unicode_events(TEST_TEXT);
            let queued = SendInput(&events, std::mem::size_of::<INPUT>() as i32);
            pump(600);

            let mut buf = [0u16; 128];
            let n = GetWindowTextW(edit, &mut buf);
            let text = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
            let _ = DestroyWindow(hwnd);

            println!("events queued by SendInput : {queued} of {}", events.len());
            println!(
                "edit box now contains      : {} characters",
                text.chars().count()
            );
            if queued as usize == events.len() && text == TEST_TEXT {
                println!("RESULT: DELIVERED to our own window (normal integrity, same desktop).");
                println!("This says nothing about other windows, elevated windows, credential");
                println!("dialogs, the secure desktop or Remote Desktop.");
                0
            } else if queued as usize != events.len() {
                println!(
                    "RESULT: BLOCKED or partial (SendInput queued fewer events than requested)."
                );
                1
            } else {
                println!("RESULT: NOT DELIVERED (events queued but the edit box text differs).");
                1
            }
        }
    }
}
