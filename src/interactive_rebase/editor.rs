//! Sequence-editor boundary: load Git's real todo, edit a model, then save it.
use super::{
    messages::{self, Mapping},
    model::{
        Action, Model,
        conversion::{self, Item},
    },
    todo::{self, Command, Entry},
};
use crate::{
    editor::Document,
    git::{Cancel, OUTPUT_LIMIT, Repository, is_oid},
};
use std::{collections::HashMap, sync::Arc};

pub enum Input {
    Plan(Plan),
    /// Git's noop confirmation is distinct from a cancelled or empty plan.
    Noop,
    /// Preserve unknown commands in the ordinary text editor.
    Text,
}
pub struct Plan {
    initial: Vec<Entry>,
    pub model: Model<Arc<Item>>,
}
pub fn load(repo: &Repository, document: &Document, token: &Cancel) -> Result<Input, String> {
    if !document.sequence {
        return Err("Expected sequence editor document".into());
    }
    let entries = match todo::parse(document.text(), "\u{1}") {
        Ok(todo::Document::Noop) => return Ok(Input::Noop),
        Ok(todo::Document::Entries(entries)) => entries,
        Err(todo::ParseError::Unsupported(_)) => return Ok(Input::Text),
        Err(error) => return Err(error.to_string()),
    };
    let metadata = messages_for(repo, &entries, token)?;
    let mut items = Vec::with_capacity(entries.len());
    for entry in &entries {
        if entry.command == Command::UpdateRef {
            items.push(Arc::new(Item {
                entry: entry.clone(),
                message: None,
            }));
        } else {
            let (oid, message) = metadata
                .get(&entry.target)
                .ok_or("Missing rebase commit details")?;
            let mut full = entry.clone();
            full.target = oid.clone();
            items.push(Arc::new(Item {
                entry: full,
                message: Some(message.clone()),
            }));
        }
    }
    Ok(Input::Plan(Plan {
        initial: entries,
        model: conversion::from_entries(items)?,
    }))
}
/// Keep process argument size bounded on Windows; at most 64 commit names per
/// read, rather than spawning Git for every row or retaining a global cache.
fn messages_for(
    repo: &Repository,
    entries: &[Entry],
    token: &Cancel,
) -> Result<HashMap<String, (String, Arc<str>)>, String> {
    let targets = entries
        .iter()
        .filter(|e| e.command != Command::UpdateRef)
        .map(|e| e.target.as_str())
        .collect::<Vec<_>>();
    let mut result = HashMap::new();
    let mut total = 0usize;
    for chunk in targets.chunks(64) {
        let mut args = vec![
            "show",
            "--no-patch",
            "--no-show-signature",
            "--encoding=UTF-8",
            "--format=%H%x00%B%x00",
        ];
        args.extend_from_slice(chunk);
        args.push("--");
        let output = repo.text(&args, token)?;
        total = total
            .checked_add(output.len())
            .ok_or("Rebase metadata exceeds preview limit")?;
        if total > OUTPUT_LIMIT {
            return Err("Rebase metadata exceeds preview limit".into());
        }
        let mut fields = output.split('\0');
        let mut found = HashMap::new();
        loop {
            let Some(hash) = fields.next() else {
                break;
            };
            let hash = hash.trim();
            if hash.is_empty() && fields.clone().next().is_none() {
                break;
            }
            if !is_oid(hash) {
                return Err("Invalid rebase metadata identity".into());
            }
            let message = fields.next().ok_or("Missing rebase metadata message")?;
            found.insert(hash.to_owned(), Arc::<str>::from(message));
        }
        for target in chunk {
            let mut matches = found.iter().filter(|(oid, _)| oid.starts_with(target));
            let (oid, message) = matches
                .next()
                .ok_or("Rebase commit metadata was not returned")?;
            if matches.next().is_some() {
                return Err("Ambiguous rebase commit abbreviation".into());
            }
            result.insert((*target).to_owned(), (oid.clone(), message.clone()));
        }
    }
    Ok(result)
}
impl Plan {
    /// Consumes the original plan to prevent a second apply. The UI can change
    /// actions/order/messages, but cannot add/remove or substitute row identities.
    pub fn save(
        self,
        edited: &Model<Arc<Item>>,
        document: &mut Document,
        head: &str,
    ) -> Result<(), String> {
        let actual = match todo::parse(document.text(), "\u{1}").map_err(|e| e.to_string())? {
            todo::Document::Entries(entries) => entries,
            todo::Document::Noop => return Err("Rebase todo changed".into()),
        };
        if !todo::matches_prepared(&self.initial, &actual) {
            return Err("Rebase todo changed".into());
        }
        let mut expected = self
            .model
            .rows()
            .iter()
            .map(|r| (r.id, r.entry.entry.target.as_str()))
            .collect::<Vec<_>>();
        let mut selected = edited
            .rows()
            .iter()
            .map(|r| (r.id, r.entry.entry.target.as_str()))
            .collect::<Vec<_>>();
        expected.sort_unstable();
        selected.sort_unstable();
        if expected != selected {
            return Err("Edited rebase plan changed commit identities".into());
        }
        let mut mappings = Vec::new();
        for row in edited.rows() {
            if let Action::Reword(message) = row.action() {
                mappings.push(Mapping {
                    original: row
                        .entry
                        .message
                        .as_deref()
                        .ok_or("Missing original reword message")?
                        .into(),
                    replacement: message.to_string(),
                });
            }
        }
        let rendered = todo::render(&conversion::to_entries(edited))?;
        messages::install(document.git_directory(), head, mappings).map_err(|e| e.to_string())?;
        document.save(&rendered).map_err(|e| e.to_string())
    }
}

/// Explicit outcomes keep a dismissed structured dialog from accidentally
/// falling through to a second, ordinary editor dialog.
pub enum Response {
    Saved,
    Cancelled,
    Text,
}
pub fn handle(
    repo: &Repository,
    document: &mut Document,
    head: &str,
    requests: &super::interaction::Requests,
    stop: &Cancel,
    token: &Cancel,
) -> Result<Response, String> {
    use super::interaction::{Answer, Question};
    match load(repo, document, token)? {
        Input::Text => Ok(Response::Text),
        Input::Noop => match requests.ask(Question::Noop, stop)? {
            Some(Answer::Proceed) => {
                if token.cancelled() || stop.cancelled() {
                    return Ok(Response::Cancelled);
                }
                document.save("noop\n").map_err(|e| e.to_string())?;
                Ok(Response::Saved)
            }
            None => Ok(Response::Cancelled),
            _ => Err("Unexpected rebase confirmation".into()),
        },
        Input::Plan(plan) => match requests.ask(
            Question::Plan(
                plan.model.clone(),
                crate::editor::MessageEncoding::load(repo, token).map_err(|e| e.to_string())?,
            ),
            stop,
        )? {
            Some(Answer::Plan(edited)) => {
                if token.cancelled() || stop.cancelled() {
                    return Ok(Response::Cancelled);
                }
                plan.save(&edited, document, head)?;
                Ok(Response::Saved)
            }
            None => Ok(Response::Cancelled),
            _ => Err("Unexpected rebase plan response".into()),
        },
    }
}
