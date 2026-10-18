//! Rebased JBUIScale.computeSystemFontData uses win.messagebox.font on Windows.
//! Resolve once; rendering and explicit text measurement must use the same face.
pub fn family() -> &'static str {
    #[cfg(windows)]
    {
        static FAMILY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        FAMILY.get_or_init(|| windows_message_font().unwrap_or_else(|| ".SystemUIFont".into()))
    }
    #[cfg(not(windows))]
    { "Inter" }
}

#[cfg(windows)]
fn windows_message_font() -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SystemParametersInfoW,
    };
    let mut metrics = NONCLIENTMETRICSW::default();
    metrics.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
    // SAFETY: cbSize describes a live, writable NONCLIENTMETRICSW; this GET
    // operation only fills the supplied buffer and does not retain its pointer.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, metrics.cbSize,
            (&mut metrics as *mut NONCLIENTMETRICSW).cast(), 0)
    };
    if ok == 0 { return None; }
    let face = metrics.lfMessageFont.lfFaceName;
    let end = face.iter().position(|c| *c == 0).unwrap_or(face.len());
    String::from_utf16(&face[..end]).ok().filter(|name| !name.trim().is_empty())
}
