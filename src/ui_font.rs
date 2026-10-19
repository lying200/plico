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

use gpui_kit::{App, SharedString, px};
use gpui_kit::component::{ActiveTheme, Theme};

#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub ui_family: String,
    pub ui_size: u16,
    pub editor_family: String,
    pub editor_size: u16,
}
impl Default for Preferences {
    fn default() -> Self { Self { ui_family: family().into(), ui_size: 13, editor_family: "JetBrains Mono".into(), editor_size: 13 } }
}
impl Preferences {
    pub fn current(cx: &App) -> Self {
        Self { ui_family: cx.theme().font_family.to_string(), ui_size: f32::from(cx.theme().font_size) as u16,
            editor_family: cx.theme().mono_font_family.to_string(), editor_size: f32::from(cx.theme().mono_font_size) as u16 }
    }
    pub fn apply(&self, cx: &mut App) {
        Theme::update(cx, |theme| {
            theme.font_family = self.ui_family.clone().into(); theme.font_size = px(self.ui_size as f32);
            theme.mono_font_family = self.editor_family.clone().into(); theme.mono_font_size = px(self.editor_size as f32);
        });
        cx.refresh_windows();
    }
    fn decode(text: &str) -> Option<Self> {
        let values: Vec<String> = serde_json::from_str(text).ok()?;
        if values.len()!=4 { return None; }
        let prefs=Self { ui_family: values[0].clone(), ui_size: values[1].parse().ok()?, editor_family: values[2].clone(), editor_size: values[3].parse().ok()? };
        if [&prefs.ui_family,&prefs.editor_family].iter().any(|s| s.trim().is_empty() || s.len()>256 || s.chars().any(char::is_control))
            || !(9..=32).contains(&prefs.ui_size) || !(9..=40).contains(&prefs.editor_size) { return None; }
        Some(prefs)
    }
    pub fn load() -> Self {
        use std::io::Read;
        let mut text=String::new();
        path().and_then(|p| std::fs::File::open(p).ok()).and_then(|f| f.take(4096).read_to_string(&mut text).ok()).and_then(|_| Self::decode(&text)).unwrap_or_default()
    }
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path)=path() else {return Err(std::io::Error::other("User settings directory is unavailable"));};
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, serde_json::to_string(&[self.ui_family.clone(),self.ui_size.to_string(),self.editor_family.clone(),self.editor_size.to_string()])?)
    }
}
fn path() -> Option<std::path::PathBuf> {
    #[cfg(windows)] let base=std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
    #[cfg(not(windows))] let base=std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(||std::env::var_os("HOME").map(|h|std::path::PathBuf::from(h).join(".config")));
    base.map(|b|b.join("GitReview").join("fonts.json"))
}
pub fn editor_family(cx: &App) -> SharedString { cx.theme().mono_font_family.clone() }
pub fn editor_size(cx: &App) -> gpui_kit::Pixels { cx.theme().mono_font_size }

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn separate_fonts_and_validate_saved_values() {
        let p=Preferences::decode(r#"["Segoe UI","14","Consolas","18"]"#).unwrap();
        assert_eq!(p.ui_size,14); assert_eq!(p.editor_size,18);
        for text in [r#"["","14","Consolas","18"]"#,r#"["Segoe UI","0","Consolas","18"]"#,r#"["Segoe UI","14","Consolas","999"]"#,"garbage"] { assert!(Preferences::decode(text).is_none()); }
    }
}

#[cfg(test)] mod diff_palette_tests {
    use gpui_kit::component::diff::{DiffFile, DiffSide};
    #[test] fn replacement_has_blue_inline_emphasis_and_utf8_boundaries() {
        let file=DiffFile::from_editor_sources(Some(("test.rs","let 中文 = old;\n")),Some(("test.rs","let 中文 = new;\n"))).unwrap();
        for side in [DiffSide::Original,DiffSide::Modified] {
            let highlights=file.editor_change_highlights(side);
            assert!(highlights.len()>1);
            assert_eq!(highlights[1].1.background_color,Some(gpui_kit::rgb(0x385570).into()));
            assert!(highlights[0].1.background_color.unwrap().a<1.);
            for (range,_,_) in highlights { assert!(file.source(side).get(range).is_some()); }
        }
        let file=DiffFile::from_editor_sources(Some(("a.txt","before\n")),Some(("a.txt","before\nadded\n"))).unwrap();
        assert!(file.editor_change_highlights(DiffSide::Original).is_empty());
        assert_eq!(file.editor_change_highlights(DiffSide::Modified)[0].1.background_color,Some(gpui_kit::rgb(0x294436).into()));
    }
}
