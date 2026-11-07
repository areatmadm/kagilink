//! Own-window input sink (D029 position, owner-approved scope 2026-11-07: "phone Send reacts in an input box").
//!
//! The receiver hands a received SYNTHETIC value to this sink. The sink types it into ONE window that
//! this program created itself, and only if that window is the foreground window with the keyboard
//! focus at that moment; otherwise nothing is typed (so the text can never land in another program).
//! Delivery is proven by reading our own edit box back (SendInput's return value only says events were
//! queued). It never touches other windows, elevated windows, credential dialogs, the secure desktop or
//! Remote Desktop (Phase 5 found those unsupported). It does not make a real-secret transfer safe: D012
//! (real secrets stay disabled) is unchanged, and the edit box is ordinary window memory that this
//! program cannot erase beyond clearing the text and destroying the window.

use zeroize::Zeroizing;

/// Longest value accepted (UTF-16 code units are sent one by one, so keep it small).
pub const MAX_CHARS: usize = 128;

/// What happened to one delivery. Never contains the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Typed and read back identically from our own edit box.
    Delivered { chars: usize },
    /// The test window was not the foreground window with focus, so NOTHING was typed.
    NotForeground,
    /// Windows queued fewer input events than requested (input blocked); the box was cleared.
    Blocked,
    /// Events were queued but the box did not contain exactly the value; the box was cleared.
    Mismatch,
    /// The value is not usable text (not UTF-8, empty, or contains control characters).
    NotText,
    /// The value is longer than [`MAX_CHARS`].
    TooLong,
    /// The window thread is gone.
    WindowGone,
}

/// Validate and convert a received value to UTF-16. Control characters (Enter, Tab, Escape ...) are
/// refused so a value can never act as a key command in the window.
pub fn prepare(plaintext: &[u8]) -> Result<Zeroizing<Vec<u16>>, Outcome> {
    let text = std::str::from_utf8(plaintext).map_err(|_| Outcome::NotText)?;
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(Outcome::NotText);
    }
    if text.chars().count() > MAX_CHARS {
        return Err(Outcome::TooLong);
    }
    Ok(Zeroizing::new(text.encode_utf16().collect()))
}

/// Why the window could not be opened.
#[derive(Debug, PartialEq, Eq)]
pub enum OpenError {
    /// Not a Windows build.
    Unsupported,
    /// A Win32 step failed; the text names the step only.
    Window(String),
}

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::OwnWindowSink;

/// Non-Windows stand-in so dependants compile everywhere; it can never be opened.
#[cfg(not(windows))]
pub struct OwnWindowSink;

#[cfg(not(windows))]
impl OwnWindowSink {
    /// Always fails on non-Windows platforms.
    pub fn open(_show_text: bool) -> Result<Self, OpenError> {
        Err(OpenError::Unsupported)
    }
    /// Never delivers.
    pub fn deliver(&self, _plaintext: &[u8]) -> Outcome {
        Outcome::WindowGone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_text() {
        let v = prepare(b"synthetic-value-0123").unwrap();
        assert_eq!(v.len(), 20);
    }

    #[test]
    fn accepts_non_ascii_text() {
        let v = prepare("가나다-ünï".as_bytes()).unwrap();
        assert_eq!(String::from_utf16(&v).unwrap(), "가나다-ünï");
    }

    #[test]
    fn refuses_empty_invalid_utf8_and_controls() {
        assert_eq!(prepare(b"").unwrap_err(), Outcome::NotText);
        assert_eq!(prepare(&[0xff, 0xfe, 0x41]).unwrap_err(), Outcome::NotText);
        for bad in ["a\nb", "a\rb", "a\tb", "a\u{1b}b", "a\u{0}b", "a\u{7f}b"] {
            assert_eq!(
                prepare(bad.as_bytes()).unwrap_err(),
                Outcome::NotText,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn length_limit_is_in_characters() {
        let max = "x".repeat(MAX_CHARS);
        assert!(prepare(max.as_bytes()).is_ok());
        let over = "x".repeat(MAX_CHARS + 1);
        assert_eq!(prepare(over.as_bytes()).unwrap_err(), Outcome::TooLong);
        let korean = "가".repeat(MAX_CHARS);
        assert!(prepare(korean.as_bytes()).is_ok());
    }
}
