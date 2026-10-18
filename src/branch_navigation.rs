//! Filtered popup leaf traversal from Rebased 4aec352 GitBranchesTreeUtil.
//! JetBrains and contributors, Apache-2.0. Unfiltered Swing row navigation is
//! a separate policy and must not be replaced with this leaf-only traversal.
use crate::branch_popup::{Group, Node, Section};
use crate::branch_search::Selection;

#[derive(Clone, Copy)]
pub enum Move {
    Next,
    Previous,
    First,
    Last,
}

#[derive(Clone, Copy)]
enum Leaf<'a> {
    Action(&'a str),
    Reference(Section, &'a str),
}
impl Leaf<'_> {
    fn selected(self, selection: &Selection) -> bool {
        match (self, selection) {
            (Self::Action(id), Selection::Action(selected)) => id == selected,
            (Self::Reference(section, name), Selection::Reference(s, n)) => {
                section == *s && name == n
            }
            _ => false,
        }
    }
    fn owned(self) -> Selection {
        match self {
            Self::Action(id) => Selection::Action(id.into()),
            Self::Reference(section, name) => Selection::Reference(section, name.into()),
        }
    }
}

/// Includes hidden descendants: choosing a leaf must reveal its ancestors in
/// the view. Only the selected identity is copied; traversal borrows ref names.
pub fn navigate(
    groups: &[Group<'_>],
    actions: &[&str],
    current: Option<&Selection>,
    direction: Move,
    directory_grouping: bool,
    cycle: bool,
) -> Option<Selection> {
    let mut leaves: Vec<_> = actions.iter().map(|id| Leaf::Action(id)).collect();
    for group in groups {
        let mut stack: Vec<_> = group
            .children("", directory_grouping)
            .into_iter()
            .rev()
            .collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Ref { reference, .. } => {
                    leaves.push(Leaf::Reference(group.section, reference.full_name))
                }
                Node::Folder { key, .. } => stack.extend(
                    group
                        .children(&key.prefix, directory_grouping)
                        .into_iter()
                        .rev(),
                ),
            }
        }
    }
    if leaves.is_empty() {
        return None;
    }
    let last = leaves.len() - 1;
    let index = current.and_then(|selected| leaves.iter().position(|leaf| leaf.selected(selected)));
    // A parent can become selected through Left even while filtering.
    if let Some(Selection::Folder(key))=current {
        let first=leaves.iter().position(|leaf| match leaf {
            Leaf::Reference(section,name) if *section==key.section => groups.iter().filter(|g|g.section==*section)
                .flat_map(|g|&g.references).any(|r|r.full_name==*name && (key.prefix.is_empty() || r.name.strip_prefix(&key.prefix).is_some_and(|tail|tail.starts_with('/')))),
            _=>false,
        });
        if let Some(first)=first {
            match direction {
                Move::Next=>return Some(leaves[first].owned()),
                Move::Previous if first>0=>return Some(leaves[first-1].owned()),
                Move::Previous=>return cycle.then(||leaves[last].owned()),
                _=>{},
            }
        }
    }

    let target = match direction {
        Move::First => 0,
        Move::Last => last,
        Move::Next => match index {
            Some(i) if i < last => i + 1,
            Some(_) if !cycle => return None,
            _ => 0,
        },
        Move::Previous => match index {
            Some(i) if i > 0 => i - 1,
            Some(_) if !cycle => return None,
            _ => last,
        },
    };
    Some(leaves[target].owned())
}

/// A visible tree row has a stable identity even when its children are collapsed.
pub fn row_selection(row: &crate::branch_popup::VisibleRow<'_>) -> Selection {
    use crate::branch_popup::{VisibleNode, FolderKey};
    match &row.node {
        VisibleNode::Section(section) => Selection::Folder(FolderKey { section:*section, prefix:String::new() }),
        VisibleNode::Entry(Node::Folder {key,..}) => Selection::Folder(key.clone()),
        VisibleNode::Entry(Node::Ref {reference,..}) => Selection::Reference(row.section,reference.full_name.into()),
    }
}

pub fn row_matches(row: &crate::branch_popup::VisibleRow<'_>, selected: &Selection) -> bool {
    use crate::branch_popup::VisibleNode;
    match (&row.node,selected) {
        (VisibleNode::Section(section),Selection::Folder(key)) => *section==key.section && key.prefix.is_empty(),
        (VisibleNode::Entry(Node::Folder {key,..}),Selection::Folder(other)) => key==other,
        (VisibleNode::Entry(Node::Ref {reference,..}),Selection::Reference(section,name)) => *section==row.section && name==reference.full_name,
        _ => false,
    }
}

/// Unfiltered TreeAction line movement includes groups/directories, but only
/// visible rows. Separators are not included in either input sequence.
pub fn navigate_visible(rows: &[crate::branch_popup::VisibleRow<'_>], actions: &[&str], current: Option<&Selection>, direction: Move, cycle: bool) -> Option<Selection> {
    let count=actions.len()+rows.len();
    if count==0 { return None; }
    let index=current.and_then(|selected| {
        actions.iter().position(|id| matches!(selected,Selection::Action(name) if name==id))
            .or_else(||rows.iter().position(|row|row_matches(row,selected)).map(|i|i+actions.len()))
    });
    let target=match direction {
        Move::First=>0, Move::Last=>count-1,
        Move::Next=>match index { None=>0,Some(i) if i+1<count=>i+1,Some(_) if cycle=>0,_=>return None },
        Move::Previous=>match index { None=>0,Some(i) if i>0=>i-1,Some(_) if cycle=>count-1,_=>return None },
    };
    if target<actions.len() { Some(Selection::Action(actions[target].into())) }
    else { Some(row_selection(&rows[target-actions.len()])) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch_popup::{Expansion, Reference, VisibleNode, visible_rows};
    #[test]
    fn visible_navigation_includes_collapsed_groups_and_respects_parent_identity() {
        use crate::branch_popup::{FolderKey,Group};
        let reference=Reference { full_name:"refs/heads/feature/topic",name:"feature/topic",id:"oid",current:false,favorite:false };
        let groups=vec![Group {section:Section::Local,references:vec![reference]}];
        let mut expansion=Expansion::default();
        let root=Selection::Folder(FolderKey{section:Section::Local,prefix:String::new()});
        let action=Selection::Action("revision".into());
        let rows=visible_rows(&groups,&expansion,true);
        assert_eq!(navigate_visible(&rows,&["revision"],Some(&action),Move::Next,false),Some(root.clone()));
        assert_eq!(navigate_visible(&rows,&["revision"],Some(&root),Move::Next,false),None);
        assert_eq!(navigate_visible(&rows,&["revision"],None,Move::Previous,false),Some(action));
        expansion.reveal(Section::Local,"feature/topic",true);
        let rows=visible_rows(&groups,&expansion,true);
        let folder=Selection::Folder(FolderKey{section:Section::Local,prefix:"feature".into()});
        assert_eq!(navigate_visible(&rows,&[],Some(&root),Move::Next,false),Some(folder.clone()));
        assert_eq!(navigate(&groups,&[],Some(&folder),Move::Next,true,false),Some(Selection::Reference(Section::Local,"refs/heads/feature/topic".into())));
        assert_eq!(navigate(&groups,&["revision"],Some(&folder),Move::Previous,true,false),Some(Selection::Action("revision".into())));
    }
    #[test]
    fn filtered_traversal_preserves_subtree_order_and_reveals_hidden_descendants() {
        let r = |full_name, name, favorite| Reference {
            full_name,
            name,
            id: "oid",
            current: false,
            favorite,
        };
        let groups = vec![Group {
            section: Section::Local,
            references: vec![
                r("refs/heads/a/plain", "a/plain", false),
                r("refs/heads/a/sub/deep", "a/sub/deep", false),
                r("refs/heads/a/favorite", "a/favorite", true),
                r("refs/heads/z", "z", false),
            ],
        }];
        let action = Selection::Action("revision".into());
        let first = navigate(
            &groups,
            &["revision"],
            Some(&action),
            Move::Next,
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            first,
            Selection::Reference(Section::Local, "refs/heads/a/favorite".into())
        );
        let deep = navigate(&groups, &[], Some(&first), Move::Next, true, false).unwrap();
        assert_eq!(
            deep,
            Selection::Reference(Section::Local, "refs/heads/a/sub/deep".into())
        );
        assert_eq!(
            navigate(&groups, &[], Some(&deep), Move::Previous, true, false),
            Some(first)
        );
        let mut expansion = Expansion::default();
        assert_eq!(visible_rows(&groups, &expansion, true).len(), 1);
        expansion.reveal(Section::Local, "a/sub/deep", true);
        assert!(visible_rows(&groups,&expansion,true).iter().any(|row|matches!(&row.node,VisibleNode::Entry(Node::Ref {reference,..}) if reference.full_name=="refs/heads/a/sub/deep")));
    }
    #[test]
    fn boundaries_cycles_actions_and_duplicate_section_identity() {
        let reference = Reference {
            full_name: "refs/heads/main",
            name: "main",
            id: "oid",
            current: true,
            favorite: false,
        };
        let groups = vec![
            Group {
                section: Section::Recent,
                references: vec![reference],
            },
            Group {
                section: Section::Local,
                references: vec![reference],
            },
        ];
        let first = Selection::Reference(Section::Recent, reference.full_name.into());
        let last = Selection::Reference(Section::Local, reference.full_name.into());
        assert_eq!(
            navigate(&groups, &[], None, Move::First, true, false),
            Some(first)
        );
        assert_eq!(
            navigate(&groups, &[], None, Move::Last, true, false),
            Some(last)
        );
        let last = Selection::Reference(Section::Local, reference.full_name.into());
        assert_eq!(
            navigate(&groups, &["revision"], Some(&last), Move::Next, true, false),
            None
        );
        assert_eq!(
            navigate(&groups, &["revision"], Some(&last), Move::Next, true, true),
            Some(Selection::Action("revision".into()))
        );
        assert_eq!(navigate(&[], &[], None, Move::First, true, true), None);
    }
}
