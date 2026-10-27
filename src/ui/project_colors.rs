//! Project-wide repository colors, independent of filtering and menu ordering.
// Palette and generation adapted from JetBrains RepositoryColorsApi.kt and
// ColorUtil/MixedColorProducer at Rebased 4aec352 (Apache-2.0).
// Copyright 2000-2025 JetBrains s.r.o. and contributors.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct RepositoryColors(BTreeMap<PathBuf, u32>);
impl RepositoryColors {
    pub fn new<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Self {
        let mut paths: Vec<_> = paths.into_iter().collect();
        // VcsLogColorManagerFactory sorts by root name. A full-path tie break
        // keeps equal basenames deterministic across discovery order changes.
        paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()).then_with(|| a.cmp(b)));
        paths.dedup();
        let count = paths.len();
        Self(
            paths
                .into_iter()
                .enumerate()
                .map(|(i, path)| (path.to_owned(), color(i, count)))
                .collect(),
        )
    }
    pub fn get(&self, path: &Path) -> Option<u32> {
        self.0.get(path).copied()
    }
}

const DARK: [u32; 14] = [
    0x865762, 0x436946, 0x5b53af, 0x825845, 0x389895, 0x826a41, 0x88458a, 0x947259, 0x178e63,
    0x583d7a, 0x227985, 0x875887, 0x8c7b58, 0x5a5d6b,
];
fn color(index: usize, count: usize) -> u32 {
    if index < DARK.len() {
        return DARK[index];
    }
    let rounds = count / DARK.len();
    let balance = (index / DARK.len()) as f64 / rounds as f64;
    let tones = ((balance - 0.5).abs() * 2. * rounds as f64 + 1.) as usize;
    let first = DARK[index % DARK.len()];
    let second = DARK[(index + 1) % DARK.len()];
    let channels = [16, 8, 0].map(|shift| {
        let a = ((first >> shift) & 255) as f64;
        let b = ((second >> shift) & 255) as f64;
        (a + (balance * (b - a) + 0.5).floor()) as f32
    });
    let max = channels.into_iter().fold(0_f32, f32::max);
    if max == 0. {
        return 0;
    }
    // Changing HSV brightness preserves channel ratios; use the same f32
    // multiplication/clamping as ColorUtil.brighter without a color-space cache.
    let mut brightness = max / 255.;
    for _ in 0..2 * tones {
        brightness = (brightness * 1.1_f32).min(1.);
    }
    channels.into_iter().fold(0, |rgb, channel| {
        (rgb << 8) | (channel / max * brightness * 255. + 0.5) as u32
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_survives_discovery_order_and_filtered_lookup() {
        let parent = Path::new("/project");
        let child = Path::new("/project/child");
        let a = RepositoryColors::new([parent, child]);
        let b = RepositoryColors::new([child, parent, child]);
        assert_eq!(a.get(child), Some(0x865762));
        assert_eq!(a.get(parent), Some(0x436946));
        assert_eq!(a.get(parent), b.get(parent));
        assert_eq!(a.get(child), b.get(child));
        assert_eq!(a.get(Path::new("/unknown")), None);
    }
    #[test]
    fn extra_roots_do_not_repeat_the_palette() {
        assert_ne!(color(14, 15), color(0, 15));
        for count in [15, 28, 29, 4096] {
            for index in 0..count {
                assert!(color(index, count) <= 0xffffff);
            }
        }
    }
}
