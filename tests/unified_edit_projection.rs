#![cfg(feature = "desktop-tests")]
use gpui_kit::component::diff::{DiffFile, UnifiedEditProjection};
fn projection(old: &str, new: &str) -> UnifiedEditProjection {
    UnifiedEditProjection::new(&DiffFile::from_sources(Some(("a", old)), Some(("a", new))).unwrap())
}
#[test]
fn deleted_lines_are_readonly_and_do_not_enter_saved_working_text() {
    let source = "共同\n新\n尾";
    let p = projection("删除\n共同\n旧\n尾", source);
    assert_eq!(p.text(), "删除\n共同\n旧\n新\n尾");
    for deleted in p.deleted_ranges() {
        assert!(p.working_range(deleted.clone()).is_none());
        assert!(p.working_range(deleted.start..deleted.start).is_none());
        assert!(
            p.working_range(deleted.start + 1..deleted.start + 1)
                .is_none()
        );
    }
    let start = p.text().find("新").unwrap();
    let mapped = p.working_range(start..start + "新".len()).unwrap();
    let mut saved = source.to_owned();
    saved.replace_range(mapped, "new\nsecond");
    assert_eq!(saved, "共同\nnew\nsecond\n尾");
    assert!(
        p.working_range(p.text().find("共同").unwrap()..start + 3)
            .is_none()
    );
    assert!(p.working_range(start + 1..start + 1).is_none());
}
#[test]
fn empty_documents_and_missing_final_newline_remain_exact() {
    for (old, new) in [
        ("", ""),
        ("old", ""),
        ("", "new"),
        ("old", "new"),
        ("a\nb\n", "a\n"),
        ("a\n", "a"),
        ("a", "a\n"),
    ] {
        let p = projection(old, new);
        let eof = p.display_offset(new.len()).unwrap();
        assert_eq!(p.working_range(eof..eof), Some(new.len()..new.len()));
        let mut saved = new.to_owned();
        saved.replace_range(p.working_range(eof..eof).unwrap(), "X");
        assert_eq!(saved, format!("{new}X"));
        assert_eq!(p.display_offset(new.len() + 1), None);
        assert_eq!(
            p.working_range(p.text().len() + 1..p.text().len() + 1),
            None
        );
    }
    assert_eq!(projection("old", "new").text(), "old\nnew");
}
#[test]
fn every_accepted_selection_matches_exact_working_bytes() {
    let samples = [
        "",
        "a",
        "a\n",
        "a\nb",
        "b\na",
        "😀\n中\n",
        "a\n中\nb\n",
        "\n\n",
    ];
    for old in samples {
        for new in samples {
            let p = projection(old, new);
            for source in (0..=new.len()).filter(|&i| new.is_char_boundary(i)) {
                let display = p.display_offset(source).unwrap();
                assert_eq!(
                    p.working_range(display..display),
                    Some(source..source),
                    "{old:?} / {new:?}"
                );
            }
            for start in 0..=p.text().len() {
                for end in start..=p.text().len() {
                    if let Some(mapped) = p.working_range(start..end) {
                        assert_eq!(&p.text()[start..end], &new[mapped], "{old:?} / {new:?}");
                        assert!(
                            p.deleted_ranges()
                                .iter()
                                .all(|r| start == end || end <= r.start || start >= r.end)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn consecutive_edits_shift_later_guards_and_source_positions_without_rediff() {
    let mut source = "共同\nnew\ntail".to_owned();
    let mut p = projection("共同\nremoved\ntail", &source);
    let deleted_before = p
        .deleted_ranges()
        .iter()
        .map(|r| p.text()[r.clone()].to_owned())
        .collect::<Vec<_>>();
    // Type, paste new lines/UTF-8, replace, delete and append. No new DiffFile
    // is constructed between these operations, as with rapid keyboard input.
    for (needle, replacement) in [
        ("共同", "head\n😀"),
        ("new", "新\nsecond"),
        ("second", ""),
        ("tail", "last"),
        ("last", "last\n"),
    ] {
        let start = p.text().find(needle).unwrap();
        let range = start..start + needle.len();
        let expected = p.working_range(range.clone()).unwrap();
        assert_eq!(p.apply_edit(range, replacement), Some(expected.clone()));
        source.replace_range(expected, replacement);
        for offset in (0..=source.len()).filter(|&i| source.is_char_boundary(i)) {
            let display = p.display_offset(offset).unwrap();
            assert_eq!(p.working_range(display..display), Some(offset..offset));
        }
        let deleted_after = p
            .deleted_ranges()
            .iter()
            .map(|r| p.text()[r.clone()].to_owned())
            .collect::<Vec<_>>();
        assert_eq!(deleted_after, deleted_before);
    }
    assert_eq!(source, "head\n😀\n新\n\nlast\n");
    let text = p.text().to_owned();
    let guards = p.deleted_ranges().to_vec();
    assert!(p.apply_edit(guards[0].clone(), "bad").is_none());
    assert_eq!(p.text(), text);
    assert_eq!(p.deleted_ranges(), guards);
}

#[test]
fn deleting_entire_writable_region_keeps_an_editable_empty_anchor() {
    let mut p = projection("old", "new");
    let start = p.text().find("new").unwrap();
    assert_eq!(p.apply_edit(start..start + 3, ""), Some(0..3));
    let empty = p.display_offset(0).unwrap();
    assert_eq!(p.apply_edit(empty..empty, "再输入"), Some(0..0));
    assert_eq!(p.text(), "old\n再输入");
    assert_eq!(
        p.working_range(empty..p.text().len()),
        Some(0.."再输入".len())
    );
}

#[test]
fn mixed_edit_sequences_preserve_roundtrips_and_readonly_bytes() {
    let samples = [
        "",
        "a",
        "a\n",
        "a\nb",
        "b\na",
        "😀\n中\n",
        "a\n中\nb\n",
        "\n\n",
    ];
    let replacements = ["", "X", "中", "\n", "😀\nnext"];
    let mut seed = 0x39a7_u64;
    for old in samples {
        for initial in samples {
            let mut source = initial.to_owned();
            let mut p = projection(old, &source);
            let guards = p
                .deleted_ranges()
                .iter()
                .map(|r| p.text()[r.clone()].to_owned())
                .collect::<Vec<_>>();
            for step in 0..24 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let boundaries = (0..=p.text().len())
                    .filter(|&i| p.text().is_char_boundary(i))
                    .collect::<Vec<_>>();
                let a = boundaries[(seed as usize) % boundaries.len()];
                let b = boundaries[((seed >> 32) as usize) % boundaries.len()];
                let range = a.min(b)..a.max(b);
                let expected = p.working_range(range.clone());
                let previous = p.text().to_owned();
                let replacement = replacements[step % replacements.len()];
                assert_eq!(p.apply_edit(range.clone(), replacement), expected);
                if let Some(mapped) = expected {
                    source.replace_range(mapped, replacement);
                    let mut display = previous;
                    display.replace_range(range, replacement);
                    assert_eq!(p.text(), display);
                } else {
                    assert_eq!(p.text(), previous);
                }
                assert_eq!(
                    p.deleted_ranges()
                        .iter()
                        .map(|r| p.text()[r.clone()].to_owned())
                        .collect::<Vec<_>>(),
                    guards
                );
                for offset in (0..=source.len()).filter(|&i| source.is_char_boundary(i)) {
                    let display = p.display_offset(offset).unwrap();
                    assert_eq!(
                        p.working_range(display..display),
                        Some(offset..offset),
                        "{old:?}/{initial:?} step {step}"
                    );
                }
            }
        }
    }
}
