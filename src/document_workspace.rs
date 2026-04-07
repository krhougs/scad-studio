use std::collections::HashMap;
use std::path::Path;

use crate::document_session::{DocumentDescriptor, DocumentKey, DocumentKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentOpenOutcome {
    Opened,
    ActivatedExisting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTab {
    pub key: DocumentKey,
    pub kind: DocumentKind,
    pub title: String,
    pub active: bool,
}

#[derive(Debug)]
pub struct DocumentSlot<T> {
    descriptor: DocumentDescriptor,
    session: T,
}

impl<T> DocumentSlot<T> {
    pub fn new(descriptor: DocumentDescriptor, session: T) -> Self {
        Self {
            descriptor,
            session,
        }
    }

    pub fn key(&self) -> &DocumentKey {
        &self.descriptor.key
    }

    pub fn descriptor(&self) -> &DocumentDescriptor {
        &self.descriptor
    }

    pub fn session(&self) -> &T {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut T {
        &mut self.session
    }
}

#[derive(Debug)]
pub struct DocumentWorkspace<T> {
    documents: Vec<DocumentSlot<T>>,
    active_key: Option<DocumentKey>,
}

impl<T> Default for DocumentWorkspace<T> {
    fn default() -> Self {
        Self {
            documents: Vec::new(),
            active_key: None,
        }
    }
}

impl<T> DocumentWorkspace<T> {
    pub fn open_or_activate(&mut self, slot: DocumentSlot<T>) -> DocumentOpenOutcome {
        if let Some(existing) = self.documents.iter().position(|current| current.key() == slot.key()) {
            self.active_key = Some(self.documents[existing].key().clone());
            return DocumentOpenOutcome::ActivatedExisting;
        }
        self.active_key = Some(slot.key().clone());
        self.documents.push(slot);
        DocumentOpenOutcome::Opened
    }

    pub fn close(&mut self, key: &DocumentKey) -> Option<DocumentSlot<T>> {
        let index = self.documents.iter().position(|current| current.key() == key)?;
        let removed = self.documents.remove(index);
        if self.active_key.as_ref() == Some(key) {
            self.active_key = self
                .documents
                .get(index)
                .or_else(|| self.documents.last())
                .map(|slot| slot.key().clone());
        }
        Some(removed)
    }

    pub fn contains(&self, key: &DocumentKey) -> bool {
        self.documents.iter().any(|current| current.key() == key)
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    pub fn set_active(&mut self, key: DocumentKey) {
        if self.contains(&key) {
            self.active_key = Some(key);
        }
    }

    #[cfg(test)]
    pub fn active_key(&self) -> Option<DocumentKey> {
        self.active_key.clone()
    }

    pub fn active(&self) -> Option<&DocumentSlot<T>> {
        let active = self.active_key.as_ref()?;
        self.documents.iter().find(|slot| slot.key() == active)
    }

    pub fn active_mut(&mut self) -> Option<&mut DocumentSlot<T>> {
        let active = self.active_key.clone()?;
        self.documents.iter_mut().find(|slot| slot.key() == &active)
    }

    pub fn tabs(&self) -> Vec<DocumentTab> {
        let titles = resolved_titles(&self.documents);
        self.documents
            .iter()
            .map(|slot| DocumentTab {
                key: slot.key().clone(),
                kind: slot.key().kind,
                title: titles
                    .get(slot.key())
                    .cloned()
                    .unwrap_or_else(|| slot.descriptor().base_title.clone()),
                active: self.active_key.as_ref() == Some(slot.key()),
            })
            .collect()
    }

    pub fn slots_mut(&mut self) -> &mut [DocumentSlot<T>] {
        &mut self.documents
    }
}

fn resolved_titles<T>(documents: &[DocumentSlot<T>]) -> HashMap<DocumentKey, String> {
    let mut groups: HashMap<&str, Vec<&DocumentDescriptor>> = HashMap::new();
    for slot in documents {
        groups
            .entry(slot.descriptor().base_title.as_str())
            .or_default()
            .push(slot.descriptor());
    }
    let mut titles = HashMap::new();
    for descriptors in groups.into_values() {
        if descriptors.len() == 1 {
            let descriptor = descriptors[0];
            titles.insert(descriptor.key.clone(), descriptor.base_title.clone());
            continue;
        }
        let suffixes = build_conflict_suffixes(&descriptors);
        for descriptor in descriptors {
            let suffix = suffixes
                .get(&descriptor.key)
                .cloned()
                .unwrap_or_else(|| descriptor.path().display().to_string());
            titles.insert(
                descriptor.key.clone(),
                format!("{} · {suffix}/", descriptor.base_title),
            );
        }
    }
    titles
}

fn build_conflict_suffixes(descriptors: &[&DocumentDescriptor]) -> HashMap<DocumentKey, String> {
    let parts = descriptors
        .iter()
        .map(|descriptor| ancestor_names(descriptor.path()))
        .collect::<Vec<_>>();
    let max_depth = parts.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let mut suffixes = HashMap::new();

    for (index, descriptor) in descriptors.iter().enumerate() {
        let mut chosen = None;
        for depth in 1..=max_depth {
            let candidate = ancestor_suffix(&parts[index], depth);
            let unique = descriptors.iter().enumerate().all(|(other_index, _)| {
                other_index == index || ancestor_suffix(&parts[other_index], depth) != candidate
            });
            if unique {
                chosen = Some(candidate);
                break;
            }
        }
        suffixes.insert(
            descriptor.key.clone(),
            chosen.unwrap_or_else(|| descriptor.path().display().to_string()),
        );
    }

    suffixes
}

fn ancestor_names(path: &Path) -> Vec<String> {
    path.parent()
        .into_iter()
        .flat_map(|parent| parent.components())
        .filter_map(|component| {
            let value = component.as_os_str().to_str()?;
            (!value.is_empty() && value != "/").then(|| value.to_owned())
        })
        .rev()
        .collect()
}

fn ancestor_suffix(names: &[String], depth: usize) -> String {
    if names.is_empty() {
        return "workspace".to_string();
    }
    names
        .iter()
        .take(depth)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("/")
}
