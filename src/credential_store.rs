//! Plico-owned credentials only. No Git config, URL rewriting or plaintext files.
//! The in-memory backend isolates tests and is the non-Windows session fallback.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
#[cfg(windows)]
mod windows;
static MUTATION: Mutex<()> = Mutex::new(());
#[derive(Clone)]
pub struct Store(Arc<Backend>);
enum Backend {
    #[cfg(test)]
    Unavailable,
    Memory(Mutex<HashMap<String, String>>),
    #[cfg(windows)]
    Windows,
}
impl Default for Store {
    fn default() -> Self {
        Self(Arc::new(Backend::Memory(Mutex::new(HashMap::new()))))
    }
}
impl Store {
    #[cfg(test)]
    pub(crate) fn unavailable() -> Self {
        Self(Arc::new(Backend::Unavailable))
    }
    pub(crate) fn is_system(&self) -> bool {
        Arc::ptr_eq(&self.0, &Self::system().0)
    }
    pub fn system() -> Self {
        static STORE: OnceLock<Store> = OnceLock::new();
        STORE
            .get_or_init(|| {
                #[cfg(windows)]
                {
                    Self(Arc::new(Backend::Windows))
                }
                #[cfg(not(windows))]
                {
                    Self::default()
                }
            })
            .clone()
    }
    pub(crate) fn get(&self, key: &str) -> Result<Option<String>, String> {
        match &*self.0 {
            #[cfg(test)]
            Backend::Unavailable => Err("Credential storage unavailable".into()),
            Backend::Memory(values) => Ok(values
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(key)
                .cloned()),
            #[cfg(windows)]
            Backend::Windows => windows::get(key),
        }
    }
    pub(crate) fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let _gate = MUTATION.lock().unwrap_or_else(|e| e.into_inner());
        match &*self.0 {
            #[cfg(test)]
            Backend::Unavailable => Err("Credential storage unavailable".into()),
            Backend::Memory(values) => {
                values
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(key.into(), value.into());
                Ok(())
            }
            #[cfg(windows)]
            Backend::Windows => windows::set(key, value),
        }
    }
    pub(crate) fn remove_if(&self, key: &str, value: &str) -> Result<(), String> {
        // Serialize this application's read/compare/delete, preserving a newer login.
        let _gate = MUTATION.lock().unwrap_or_else(|e| e.into_inner());
        match &*self.0 {
            #[cfg(test)]
            Backend::Unavailable => Err("Credential storage unavailable".into()),
            Backend::Memory(values) => {
                let mut values = values.lock().unwrap_or_else(|e| e.into_inner());
                if values.get(key).is_some_and(|v| v == value) {
                    values.remove(key);
                }
                Ok(())
            }
            #[cfg(windows)]
            Backend::Windows => {
                if windows::get(key)?.as_deref() == Some(value) {
                    windows::remove(key)?;
                }
                Ok(())
            }
        }
    }
}
