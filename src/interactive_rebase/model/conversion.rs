// Adapted from GitRebaseTodoModelConverter.kt, Apache-2.0.
// Copyright 2000-2024 JetBrains s.r.o. and contributors.
// Source/version and license: docs/THIRD-PARTY.txt.
//! Convert Git's initial todo into contiguous editable groups without changing Git.
use super::{Action, Model, Row, RowId};
use crate::{
    git::{HISTORY_MAX, OUTPUT_LIMIT},
    interactive_rebase::todo::{Command, Entry},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Item {
    pub entry: Entry,
    /// Full commit message, absent for update-ref or unavailable metadata.
    pub message: Option<Arc<str>>,
}

pub fn from_entries(entries: Vec<Arc<Item>>) -> Result<Model<Arc<Item>>, String> {
    if entries.len() > HISTORY_MAX {
        return Err("Rebase plan exceeds history limit".into());
    }
    let mut bytes = 0usize;
    for item in &entries {
        for text in [
            Some(item.entry.target.as_str()),
            Some(item.entry.subject.as_str()),
            item.message.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            bytes = bytes
                .checked_add(text.len())
                .ok_or("Rebase plan exceeds preview limit")?;
            if bytes > OUTPUT_LIMIT || text.contains('\0') {
                return Err("Invalid or oversized rebase plan".into());
            }
        }
    }
    let mut rows: Vec<Row<Arc<Item>>> = Vec::with_capacity(entries.len());
    let mut dropped = Vec::new();
    for entry in entries {
        if entry.entry.command == Command::Drop {
            dropped.push(entry);
            continue;
        }
        let id = RowId(rows.len());
        let mut parent = None;
        let action = match entry.entry.command {
            Command::Pick | Command::Reword => Action::Pick,
            Command::Edit => Action::Edit,
            Command::UpdateRef => Action::UpdateRef,
            Command::Drop => unreachable!(),
            Command::Fixup { .. } | Command::Squash => {
                let last = rows
                    .last()
                    .ok_or("Fixup or squash has no preceding commit")?;
                let root = last.parent.unwrap_or(last.id);
                if !rows[root.0].action.keep() {
                    return Err("Fixup or squash cannot follow update-ref".into());
                }
                if let Some(message) = amended_message(&rows[root.0], &entry)? {
                    rows[root.0].action = Action::Reword(message.into());
                }
                parent = Some(root);
                Action::Fixup
            }
        };
        rows.push(Row {
            id,
            entry,
            action,
            parent,
        });
    }
    for entry in dropped {
        rows.push(Row {
            id: RowId(rows.len()),
            entry,
            action: Action::Drop,
            parent: None,
        });
    }
    Ok(Model { rows })
}

fn amended_message(root: &Row<Arc<Item>>, child: &Item) -> Result<Option<String>, String> {
    let (Some(original), Some(message)) = (root.entry.message.as_deref(), child.message.as_deref())
    else {
        return Ok(None);
    };
    let old = match &root.action {
        Action::Reword(message) => message.as_ref(),
        _ => original,
    };
    let subject = message.lines().next().unwrap_or("");
    let autosquash = ["fixup! ", "squash! ", "amend! "]
        .iter()
        .any(|p| subject.strip_prefix(p).is_some_and(|s| !s.is_empty()));
    let message = if autosquash {
        message.split_once('\n').map_or("", |(_, body)| body).trim()
    } else {
        message
    };
    if message.trim().is_empty() {
        return Ok(None);
    }
    let next = match child.entry.command {
        Command::Squash => {
            if old.len().saturating_add(message.len()).saturating_add(3) > OUTPUT_LIMIT {
                return Err("Rebase message exceeds preview limit".into());
            }
            format!("{old}\n\n\n{message}")
        }
        Command::Fixup {
            override_message: true,
        } => message.to_owned(),
        _ => return Ok(None),
    };
    Ok((next != old).then_some(next))
}

pub fn to_entries(model: &Model<Arc<Item>>) -> Vec<Entry> {
    model
        .rows
        .iter()
        .map(|row| Entry {
            command: match row.action {
                Action::Pick => Command::Pick,
                Action::Edit => Command::Edit,
                Action::Reword(_) => Command::Reword,
                Action::Drop => Command::Drop,
                Action::UpdateRef => Command::UpdateRef,
                Action::Fixup => Command::Fixup {
                    override_message: false,
                },
            },
            target: row.entry.entry.target.clone(),
            subject: row.entry.entry.subject.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(n: usize, command: Command, message: &str) -> Arc<Item> {
        Arc::new(Item {
            entry: Entry {
                command,
                target: format!("{n:040x}"),
                subject: message.lines().next().unwrap_or("").into(),
            },
            message: Some(message.into()),
        })
    }
    #[test]
    fn autosquash_import_groups_and_preserves_distinct_message_rules() {
        let m = from_entries(vec![
            item(1, Command::Pick, "first"),
            item(
                2,
                Command::Fixup {
                    override_message: false,
                },
                "fixup! first\n\nignored body",
            ),
            item(3, Command::Pick, "second"),
            item(
                4,
                Command::Squash,
                "squash! second\n\nextra squash explanation",
            ),
            item(5, Command::Edit, "third"),
        ])
        .unwrap();
        assert_eq!(m.rows[1].parent, Some(m.rows[0].id));
        assert_eq!(m.rows[0].action, Action::Pick);
        assert_eq!(m.rows[3].parent, Some(m.rows[2].id));
        assert_eq!(
            m.rows[2].action,
            Action::Reword("second\n\n\nextra squash explanation".into())
        );
        assert_eq!(m.rows[4].action, Action::Edit);
        let exported = to_entries(&m);
        assert_eq!(exported[2].command, Command::Reword);
        assert_eq!(
            exported[3].command,
            Command::Fixup {
                override_message: false
            }
        );
    }
    #[test]
    fn override_empty_body_and_drop_conversion() {
        let m = from_entries(vec![
            item(0, Command::Drop, "dropped"),
            item(1, Command::Reword, "original"),
            item(
                2,
                Command::Fixup {
                    override_message: true,
                },
                "amend! original\n\nreplacement",
            ),
            item(3, Command::Squash, "squash! original"),
            item(4, Command::Squash, "regular message"),
        ])
        .unwrap();
        assert_eq!(
            m.rows[0].action,
            Action::Reword("replacement\n\n\nregular message".into())
        );
        assert_eq!(m.rows.last().unwrap().action, Action::Drop);
        assert!(m.rows[1..4].iter().all(|r| r.parent == Some(m.rows[0].id)));
        assert!(m.valid());
    }
    #[test]
    fn cannot_join_missing_root_or_cross_update_ref() {
        assert!(from_entries(vec![item(0, Command::Squash, "bad")]).is_err());
        assert!(
            from_entries(vec![
                item(0, Command::Pick, "base"),
                item(1, Command::UpdateRef, ""),
                item(
                    2,
                    Command::Fixup {
                        override_message: false
                    },
                    "bad"
                )
            ])
            .is_err()
        );
        let m = from_entries(vec![
            item(0, Command::Pick, "base"),
            item(1, Command::UpdateRef, ""),
        ])
        .unwrap();
        assert_eq!(to_entries(&m)[1].command, Command::UpdateRef);
    }
}
