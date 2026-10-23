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
    Memory(Memory),
    #[cfg(windows)]
    Windows,
}
#[derive(Default)]
struct Memory {
    values: Mutex<HashMap<String, String>>,
    #[cfg(test)]
    failure: Mutex<Option<(String, bool)>>,
}
impl Default for Store {
    fn default() -> Self {
        Self(Arc::new(Backend::Memory(Memory::default())))
    }
}
impl Store {
    #[cfg(test)]
    pub(crate) fn fail_next_mutation(&self, key: &str, remove: bool) {
        let Backend::Memory(memory) = &*self.0 else {
            panic!("Fault injection requires an isolated memory store");
        };
        *memory.failure.lock().unwrap() = Some((key.into(), remove));
    }
    #[cfg(test)]
    fn check_mutation(&self, key: &str, remove: bool) -> Result<(), String> {
        if let Backend::Memory(memory) = &*self.0 {
            let mut failure = memory.failure.lock().unwrap();
            if failure
                .as_ref()
                .is_some_and(|(k, r)| k == key && *r == remove)
            {
                *failure = None;
                return Err("Injected credential mutation failure".into());
            }
        }
        Ok(())
    }

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
            Backend::Memory(memory) => Ok(memory
                .values
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
        #[cfg(test)]
        self.check_mutation(key, false)?;
        match &*self.0 {
            #[cfg(test)]
            Backend::Unavailable => Err("Credential storage unavailable".into()),
            Backend::Memory(memory) => {
                memory
                    .values
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
        #[cfg(test)]
        self.check_mutation(key, true)?;
        match &*self.0 {
            #[cfg(test)]
            Backend::Unavailable => Err("Credential storage unavailable".into()),
            Backend::Memory(memory) => {
                let mut values = memory.values.lock().unwrap_or_else(|e| e.into_inner());
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

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Entry(String);
    impl Drop for Entry {
        fn drop(&mut self) {
            // Only this test's uniquely named credential; never enumerate/delete
            // the user's real HTTP or hosted-account records.
            let _ = windows::remove(&self.0);
        }
    }
    #[test]
    fn native_store_failed_write_preserves_previous_value_and_stale_delete_is_ignored() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let entry = Entry(format!(
            "native-test:{}:{}:{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let store = Store(Arc::new(Backend::Windows));
        assert!(store.get(&entry.0).unwrap().is_none());
        store.set(&entry.0, "fixture-old").unwrap();
        // Use multibyte text so the limit is tested in bytes, not characters.
        let too_large = "测".repeat(1000);
        assert!(store.set(&entry.0, &too_large).is_err());
        assert_eq!(store.get(&entry.0).unwrap().as_deref(), Some("fixture-old"));
        store.set(&entry.0, "fixture-new").unwrap();
        store.remove_if(&entry.0, "fixture-old").unwrap();
        assert_eq!(store.get(&entry.0).unwrap().as_deref(), Some("fixture-new"));
        store.remove_if(&entry.0, "fixture-new").unwrap();
        assert!(store.get(&entry.0).unwrap().is_none());
        store.remove_if(&entry.0, "fixture-new").unwrap();
    }
}
