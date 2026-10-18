//! Rebased 4aec352 NameUtil builder / MatcherWithFallback /
//! GitBranchesMatcherWrapper. JetBrains and contributors, Apache-2.0.
use crate::{
    keyboard_layout::Layout, pinyin_matcher::PinyinMatcher, search_unicode,
    utf16_matcher::Utf16Matcher,
};
use std::ops::Range;

pub struct Search {
    pattern: String,
    main: Utf16Matcher,
    layout: Option<Utf16Matcher>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Match {
    /// UTF-16 offsets, to be converted at the UI text-rendering boundary.
    pub fragments: Vec<Range<usize>>,
    pub degree: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Folder(crate::branch_popup::FolderKey),
    Action(String),
    Reference(crate::branch_popup::Section, String),
}

pub struct PopupResult {
    pub actions: Vec<String>,
    pub selected: Option<Selection>,
}

/// Convert matcher UTF-16 intervals to GPUI UTF-8 byte intervals. A match
/// touching either surrogate covers its whole scalar, never an invalid UTF-8
/// boundary. Input ranges must be ordered, as returned by Search::find.
pub fn byte_ranges(text: &str, ranges: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut result: Vec<Range<usize>> = Vec::new();
    let mut utf16 = 0;
    let mut range_index = 0;
    for (byte,c) in text.char_indices() {
        let end = utf16+c.len_utf16();
        while ranges.get(range_index).is_some_and(|range| range.end<=utf16) { range_index+=1; }
        if ranges.get(range_index).is_some_and(|range| range.start<end && range.end>utf16) {
            let byte_end=byte+c.len_utf8();
            if let Some(last)=result.last_mut().filter(|r| r.end==byte) { last.end=byte_end; }
            else { result.push(byte..byte_end); }
        }
        utf16=end;
    }
    result
}

/// PopupBase nullize(true) and trailing-whitespace normalization before the
/// step's matcher construction. Keep this separate from Search::new semantics.
pub fn popup_pattern(input: &str) -> Option<&str> {
    let trimmed = input.trim_end_matches(|c: char| search_unicode::whitespace(c as u32));
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Applies the single-repository popup's filtering and preferred-selection
/// policy. Actions carry stable IDs and already-localized visible text. The
/// caller provides the saved-preferred/current ref for the unfiltered case.
/// Expansion is additive here, matching applySearchPattern; explicit collapse
/// remains owned by the tree event handler, rather than a search-only snapshot.
pub fn apply_to_popup(
    search: Option<&Search>,
    groups: &mut Vec<crate::branch_popup::Group<'_>>,
    actions: &[(&str, &str)],
    filter_actions: bool,
    default_ref: Option<&str>,
    expansion: &mut crate::branch_popup::Expansion,
    directory_grouping: bool,
) -> PopupResult {
    use crate::branch_popup::Section;
    let reference = if let Some(search) = search {
        search
            .filter_groups(groups)
            .map(|(section, name)| Selection::Reference(section, name.into()))
    } else {
        default_ref.and_then(|name| {
            groups.iter().find_map(|group| {
                group
                    .references
                    .iter()
                    .find(|r| r.full_name == name)
                    .map(|r| Selection::Reference(group.section, r.full_name.into()))
            })
        })
    };
    let mut action_match: Option<(&str, i32)> = None;
    let visible_actions = actions
        .iter()
        .filter_map(|&(id, text)| {
            if let Some(search) = search.filter(|_| filter_actions) {
                let found = search.find(text)?;
                if action_match.is_none_or(|(_, degree)| found.degree > degree) {
                    action_match = Some((id, found.degree));
                }
            }
            Some(id.to_owned())
        })
        .collect::<Vec<_>>();
    let selected = action_match
        .map(|(id, _)| Selection::Action(id.into()))
        .or(reference)
        .or_else(|| {
            visible_actions
                .first()
                .map(|id| Selection::Action(id.clone()))
        })
        .or_else(|| {
            groups.iter().find_map(|g| {
                g.references
                    .first()
                    .map(|r| Selection::Reference(g.section, r.full_name.into()))
            })
        });
    for group in groups.iter() {
        for reference in &group.references {
            let preferred = matches!(&selected,Some(Selection::Reference(section,name)) if *section==group.section && name==reference.full_name);
            if reference.current || group.section == Section::Recent || preferred {
                expansion.reveal(group.section, reference.name, directory_grouping);
            }
        }
    }
    PopupResult {
        actions: visible_actions,
        selected,
    }
}

impl Search {
    /// Filter already ordered groups in place, preserving their display order.
    /// Preferred ref uses group precedence, then score; equal scores keep the
    /// first sorted item. Actions are handled separately by the popup owner.
    pub fn filter_groups<'a>(
        &self,
        groups: &mut Vec<crate::branch_popup::Group<'a>>,
    ) -> Option<(crate::branch_popup::Section, &'a str)> {
        use crate::branch_popup::Section;
        let recent: std::collections::HashSet<_> = groups
            .iter()
            .filter(|g| g.section == Section::Recent)
            .flat_map(|g| g.references.iter().map(|r| r.full_name))
            .collect();
        let mut preferred = None;
        for section in [
            Section::Recent,
            Section::Local,
            Section::Remote,
            Section::Tags,
        ] {
            let Some(group) = groups.iter_mut().find(|g| g.section == section) else {
                continue;
            };
            let mut best: Option<(&'a str, i32)> = None;
            group.references.retain(|reference| {
                if section == Section::Local && recent.contains(reference.full_name) {
                    return false;
                }
                let Some(found) = self.find(reference.name) else {
                    return false;
                };
                if best.is_none_or(|(_, degree)| found.degree > degree) {
                    best = Some((reference.full_name, found.degree));
                }
                true
            });
            if preferred.is_none() {
                preferred = best.map(|(name, _)| (section, name));
            }
        }
        groups.retain(|g| !g.references.is_empty());
        preferred
    }

    /// None and exactly '/' disable filtering. Empty/blank strings still create
    /// a matcher, as setSearchPattern does; callers decide when to pass None.
    pub fn new(input: Option<&str>, layout: &Layout, linux: bool) -> Option<Self> {
        let input = input?;
        if input == "/" {
            return None;
        }
        let trimmed = input.trim_matches(|c: char| search_unicode::whitespace(c as u32));
        let pattern = format!("*{trimmed}");
        let main = Utf16Matcher::new(&pattern);
        let alternate = layout.alternate(&pattern, linux, search_unicode::char_letter);
        let layout = alternate.as_deref().map(Utf16Matcher::new);
        Some(Self {
            pattern,
            main,
            layout,
        })
    }

    /// Evaluate once so fragments and degree always come from the same branch
    /// of the fallback chain. Main > corrected layout > original-pattern pinyin.
    pub fn find(&self, name: &str) -> Option<Match> {
        let mut matcher = &self.main;
        let mut fragments = matcher.find(name);
        if fragments.as_ref().is_none_or(Vec::is_empty) {
            if let Some(layout) = &self.layout {
                matcher = layout;
                fragments = matcher.find(name);
            }
        }
        if fragments.as_ref().is_none_or(Vec::is_empty) {
            if let Some(pinyin) = PinyinMatcher::new(&self.pattern) {
                let fragment = pinyin.find(name)?;
                let degree = pinyin.matching_degree(name, Some(&fragment));
                return Some(weight(vec![fragment], degree));
            }
        }
        let fragments = fragments?;
        let degree = matcher.matching_degree(name, false, Some(&fragments));
        Some(weight(fragments, degree))
    }
}

fn weight(fragments: Vec<Range<usize>>, degree: i32) -> Match {
    // JVM Int arithmetic wraps, including unusual very long candidate offsets.
    let degree = fragments.first().map_or(degree, |range| {
        degree.wrapping_add(10000).wrapping_sub(range.start as i32)
    });
    Match { fragments, degree }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn search(input: &str) -> Search {
        Search::new(Some(input), &Layout::default(), false).unwrap()
    }

    #[test]
    fn displayed_highlights_convert_unicode_without_slicing_full_ref_offsets() {
        let query=search("topic");
        let label="topic";
        assert_eq!(byte_ranges(label,&query.find(label).unwrap().fragments),vec![0..5]);
        assert!(query.find("feature").is_none());
        assert_eq!(byte_ranges("😀中文/修复",&[2..3,5..7]),vec![4..7,11..17]);
        assert_eq!(byte_ranges("a😀b",&[2..3]),vec![1..5]);
        assert_eq!(byte_ranges("a😀b",&[1..2,2..4]),vec![1..6]);
        assert_eq!(byte_ranges("你好",&[]),Vec::<Range<usize>>::new());
    }
    #[test]
    fn popup_selection_actions_expansion_and_clear_follow_single_repo_policy() {
        use crate::branch_popup::{Expansion, FolderKey, Group, Reference, Section};
        let make = || {
            vec![Group {
                section: Section::Local,
                references: vec![
                    Reference {
                        full_name: "refs/heads/feature/main",
                        name: "feature/main",
                        id: "a",
                        current: true,
                        favorite: false,
                    },
                    Reference {
                        full_name: "refs/heads/feature/revision",
                        name: "feature/revision",
                        id: "b",
                        current: false,
                        favorite: false,
                    },
                    Reference {
                        full_name: "refs/heads/unrelated/topic",
                        name: "unrelated/topic",
                        id: "c",
                        current: false,
                        favorite: false,
                    },
                ],
            }]
        };
        let mut expansion = Expansion::default();
        let saved = FolderKey {
            section: Section::Local,
            prefix: "user/expanded".into(),
        };
        expansion.expanded.insert(saved.clone());
        let actions = [("revision", "Checkout Tag or Revision…")];
        let query = search("revision");
        let result = apply_to_popup(
            Some(&query),
            &mut make(),
            &actions,
            true,
            None,
            &mut expansion,
            true,
        );
        assert_eq!(result.selected, Some(Selection::Action("revision".into())));
        assert_eq!(result.actions, vec!["revision"]);
        assert!(expansion.expanded.contains(&saved));
        let mut groups = make();
        let result = apply_to_popup(
            Some(&query),
            &mut groups,
            &actions,
            false,
            None,
            &mut expansion,
            true,
        );
        assert_eq!(
            result.selected,
            Some(Selection::Reference(
                Section::Local,
                "refs/heads/feature/revision".into()
            ))
        );
        assert!(expansion.expanded.contains(&FolderKey {
            section: Section::Local,
            prefix: "feature".into()
        }));
        assert!(!expansion.expanded.contains(&FolderKey {
            section: Section::Local,
            prefix: "unrelated".into()
        }));
        let result = apply_to_popup(
            None,
            &mut make(),
            &actions,
            true,
            Some("refs/heads/feature/main"),
            &mut expansion,
            true,
        );
        assert_eq!(
            result.selected,
            Some(Selection::Reference(
                Section::Local,
                "refs/heads/feature/main".into()
            ))
        );
        assert!(expansion.expanded.contains(&saved));
        let result = apply_to_popup(
            Some(&search("absent")),
            &mut make(),
            &actions,
            true,
            None,
            &mut expansion,
            true,
        );
        assert!(result.selected.is_none());
        assert!(result.actions.is_empty());
        assert_eq!(popup_pattern("  \u{a0}"), None);
        assert_eq!(popup_pattern(" main \u{a0}"), Some(" main"));
        assert_eq!(popup_pattern("/ "), Some("/"));
    }
    #[test]
    fn main_match_wins_over_layout_and_pinyin_and_scores_its_own_ranges() {
        let query = search("nh");
        let matched = query.find("你好/nh").unwrap();
        assert_eq!(matched.fragments, vec![3..5]); // Pinyin would match 0..2.
        let main = Utf16Matcher::new("*nh");
        assert_eq!(
            matched.degree,
            main.matching_degree("你好/nh", false, Some(&matched.fragments)) + 9997
        );
        assert_eq!(
            query.find("你好"),
            Some(Match {
                fragments: vec![0..2],
                degree: 10502
            })
        );
        assert_eq!(
            query.find("😀你好"),
            Some(Match {
                fragments: vec![2..4],
                degree: 10000
            })
        );
        let query = search("ыек");
        assert_eq!(query.find("String/ыек").unwrap().fragments, vec![7..10]);
        assert_eq!(query.find("String").unwrap().fragments, vec![0..3]);
        assert!(search("ыекA").find("StringA").is_none());
        assert!(search("NH").find("你好").is_none());
    }

    #[test]
    fn search_preprocessing_and_offset_weight_follow_popup_rules() {
        assert!(Search::new(None, &Layout::default(), false).is_none());
        assert!(Search::new(Some("/"), &Layout::default(), false).is_none());
        assert!(search(" / ").find("main").is_none());
        assert_eq!(
            search(" / ").find("feature/topic").unwrap().fragments,
            vec![7..8]
        );
        assert_eq!(
            search("\u{a0}cache\u{a0}").find("cache"),
            search("cache").find("cache")
        );
        assert_eq!(
            search("").find("main"),
            Some(Match {
                fragments: vec![],
                degree: 0
            })
        );
        assert!(
            search("cache").find("cache").unwrap().degree
                > search("cache").find("feature/cache").unwrap().degree
        );
        assert!(search("nothing").find("main").is_none());
    }

    #[test]
    fn observed_layout_uses_java_char_properties_and_windows_fallback_rules() {
        let mut layout = Layout::default();
        layout.observe(0, '😀'); // A Java Char observation cannot contain this scalar.
        assert_eq!(
            search("ыек").find("String"),
            Search::new(Some("ыек"), &layout, false)
                .unwrap()
                .find("String")
        );
        layout.observe(12, '中');
        layout.observe(0, '中'); // First observation wins.
        assert!(
            Search::new(Some("中"), &layout, false)
                .unwrap()
                .find("main")
                .is_some()
        );
        assert!(
            Search::new(Some("ыек"), &layout, false)
                .unwrap()
                .find("String")
                .is_none()
        );
        assert!(
            Search::new(Some("ыек"), &layout, true)
                .unwrap()
                .find("String")
                .is_some()
        );
    }

    #[test]
    fn preferred_reference_preserves_display_order_and_group_precedence() {
        use crate::branch_popup::{Group, Reference, Section};
        let reference = |full_name, name| Reference {
            full_name,
            name,
            id: "oid",
            current: false,
            favorite: false,
        };
        let recent = reference("refs/heads/feature/cache", "feature/cache");
        let exact = reference("refs/heads/cache", "cache");
        let other = reference("refs/heads/other", "other");
        let mut groups = vec![
            Group {
                section: Section::Recent,
                references: vec![recent],
            },
            Group {
                section: Section::Local,
                references: vec![recent, exact, other],
            },
            Group {
                section: Section::Tags,
                references: vec![reference("refs/tags/cache", "cache")],
            },
        ];
        assert_eq!(
            search("cache").filter_groups(&mut groups),
            Some((Section::Recent, recent.full_name))
        );
        assert_eq!(
            groups[1]
                .references
                .iter()
                .map(|r| r.full_name)
                .collect::<Vec<_>>(),
            vec![exact.full_name]
        );
        let mut groups = vec![Group {
            section: Section::Local,
            references: vec![recent, exact],
        }];
        assert_eq!(
            search("cache").filter_groups(&mut groups),
            Some((Section::Local, exact.full_name))
        );
        assert_eq!(groups[0].references[0].full_name, recent.full_name); // Score does not reorder rows.
        assert_eq!(
            search("").filter_groups(&mut groups),
            Some((Section::Local, recent.full_name))
        ); // Equal score keeps first.
        assert_eq!(search("missing").filter_groups(&mut groups), None);
        assert!(groups.is_empty());
    }
}
