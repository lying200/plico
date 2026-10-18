use plico::{
    git::Cancel,
    three_way::{self, Tint},
};
fn check(texts: [&str; 3]) -> Vec<three_way::Row> {
    let rows = three_way::align(texts, &Cancel::default()).unwrap();
    for col in 0..3 {
        let original = texts[col].split_inclusive('\n').collect::<Vec<_>>();
        let indices = rows.iter().filter_map(|r| r.lines[col]).collect::<Vec<_>>();
        assert_eq!(
            indices,
            (0..original.len()).collect::<Vec<_>>(),
            "every source line must appear exactly once in its column"
        );
        assert_eq!(
            indices.into_iter().map(|i| original[i]).collect::<String>(),
            texts[col]
        );
    }
    rows
}
#[test]
fn identical_and_empty_sources_have_no_changes() {
    assert!(check(["", "", ""]).is_empty());
    assert!(
        check(["a\nb\n", "a\nb\n", "a\nb\n"])
            .iter()
            .all(|r| r.tints == [None; 3])
    );
}
#[test]
fn one_parent_change_colors_only_parent_and_result() {
    let rows = check(["a\nold\nz\n", "a\nnew\nz\n", "a\nnew\nz\n"]);
    assert_eq!(
        rows[1].tints,
        [Some(Tint::Modified), Some(Tint::Modified), None]
    );
}
#[test]
fn result_additions_and_deletions_invert_conflict_editor_colors() {
    let added = check(["a\nz\n", "a\ninsert\nz\n", "a\nz\n"]);
    assert_eq!(added[1].lines, [None, Some(1), None]);
    assert_eq!(added[1].tints[1], Some(Tint::Inserted));
    let removed = check(["a\nremove\nz\n", "a\nz\n", "a\nremove\nz\n"]);
    assert_eq!(removed[1].lines, [Some(1), None, Some(1)]);
    assert_eq!(removed[1].tints[0], Some(Tint::Deleted));
}
#[test]
fn divergent_parents_conflict_but_equal_parent_changes_are_modifications() {
    assert_eq!(
        check(["left\n", "result\n", "right\n"])[0].tints,
        [Some(Tint::Conflict); 3]
    );
    assert_eq!(
        check(["same\n", "result\n", "same\n"])[0].tints,
        [Some(Tint::Modified); 3]
    );
}
#[test]
fn fragments_pad_shorter_sides_without_fabricating_source_lines() {
    let r = check([
        "head\nleft1\nleft2\ntail\n",
        "head\nresult\ntail\n",
        "head\nright\ntail\n",
    ]);
    assert_eq!(r[2].lines, [Some(2), None, None]);
    assert_eq!(r[3].lines, [Some(3), Some(2), Some(2)]);
    check(["中\r\n\tlast", "中\r\n\tedit\nlast", "中\nlast"]);
}
#[test]
fn all_small_sequences_preserve_source_order_across_repeated_lines() {
    let samples = [
        "",
        "a\n",
        "b\n",
        "a\nb\n",
        "b\na\n",
        "a\na\n",
        "a\nb\na\n",
        "b\na\nb\n",
    ];
    for a in samples {
        for b in samples {
            for c in samples {
                check([a, b, c]);
            }
        }
    }
}
#[test]
fn cancellation_and_row_limits_do_not_silently_truncate() {
    let token = Cancel::default();
    token.cancel();
    assert_eq!(
        three_way::align(["a", "a", "a"], &token).unwrap_err(),
        "Cancelled"
    );
    let huge = "x\n".repeat(three_way::MAX_ROWS + 1);
    assert!(
        three_way::align([&huge, &huge, &huge], &Cancel::default())
            .unwrap_err()
            .contains("budget")
    );
}

fn emphasized(texts: [&str; 3]) -> [Vec<String>; 3] {
    let rows = check(texts);
    let inner = three_way::inner_ranges(texts, &rows, &Cancel::default()).unwrap();
    std::array::from_fn(|col| {
        let lines = texts[col].split_inclusive('\n').collect::<Vec<_>>();
        let lines = &lines;
        inner[col]
            .iter()
            .flat_map(|(&line, ranges)| {
                ranges
                    .iter()
                    .map(move |r| lines[line][r.clone()].to_owned())
            })
            .collect()
    })
}
#[test]
fn inner_words_union_both_parent_comparisons_in_result() {
    let texts = [
        "let alpha = 1; let beta = 2;\n",
        "let alpha = 10; let beta = 20;\n",
        "let alpha = 10; let beta = 2;\n",
    ];
    let e = emphasized(texts);
    assert_eq!(e[0], ["1", "2"]);
    assert_eq!(e[1], ["10", "20"]);
    assert_eq!(e[2], ["2"]);
}
#[test]
fn inner_compares_entire_fragment_across_inserted_lines() {
    let e = emphasized([
        "let value = 1;\n",
        "// added\nlet value = 2;\n",
        "// added\nlet value = 2;\n",
    ]);
    assert_eq!(e[0], ["1"]);
    assert!(e[1].iter().any(|v| v == "2"));
    assert!(
        !e[1].iter().any(|v| v.contains("let value")),
        "unchanged words stay unaccented despite shifted rows"
    );
    assert!(e[2].is_empty());
}
#[test]
fn inner_source_offsets_preserve_unicode_tabs_and_crlf() {
    let e = emphasized([
        "\tlet 名字 = \"旧值\";\r\n",
        "\tlet 名字 = \"新值\";\r\n",
        "\tlet 名字 = \"新值\";\r\n",
    ]);
    assert_eq!(e[0].concat(), "旧");
    assert_eq!(e[1].concat(), "新");
    assert!(e[2].is_empty());
    assert!(
        emphasized(["", "only result\n", ""])
            .iter()
            .all(Vec::is_empty)
    );
}
#[test]
fn inner_skips_oversized_blocks_without_affecting_rows_and_checks_cancel() {
    let a = format!("{}x\n", "a".repeat(33000));
    let b = format!("{}y\n", "a".repeat(33000));
    assert!(emphasized([&a, &b, &a]).iter().all(Vec::is_empty));
    let token = Cancel::default();
    token.cancel();
    assert!(three_way::inner_ranges(["", "", ""], &[], &token).is_err());
}
