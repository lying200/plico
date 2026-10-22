//! Windows Credential Manager generic credentials, scoped to the current user.
use windows_sys::Win32::{
    Foundation::{ERROR_NOT_FOUND, GetLastError},
    Security::Credentials::*,
};
fn key(key: &str) -> Result<Vec<u16>, String> {
    let name = format!("Plico:git-http:v1:{key}");
    if name.contains('\0') || name.encode_utf16().count() > 32767 {
        return Err("Credential identity is too long".into());
    }
    Ok(name.encode_utf16().chain(Some(0)).collect())
}
fn error() -> String {
    format!("Windows credential storage failed ({})", unsafe {
        GetLastError()
    })
}
pub(super) fn get(name: &str) -> Result<Option<String>, String> {
    let name = key(name)?;
    let mut ptr = std::ptr::null_mut();
    if unsafe { CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &mut ptr) } == 0 {
        return if unsafe { GetLastError() } == ERROR_NOT_FOUND {
            Ok(None)
        } else {
            Err(error())
        };
    }
    // CredRead returns one allocation; all pointers remain valid until CredFree.
    let result = unsafe {
        let bytes = if (*ptr).CredentialBlobSize == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts((*ptr).CredentialBlob, (*ptr).CredentialBlobSize as usize)
        };
        let result = String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(|_| "Invalid saved credential".into());
        CredFree(ptr.cast());
        result
    };
    result
}
pub(super) fn set(name: &str, value: &str) -> Result<(), String> {
    let mut name = key(name)?;
    if value.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
        return Err("Credential exceeds system storage limit".into());
    }
    let mut blob = value.as_bytes().to_vec();
    let mut credential: CREDENTIALW = unsafe { std::mem::zeroed() };
    credential.Type = CRED_TYPE_GENERIC;
    credential.TargetName = name.as_mut_ptr();
    credential.CredentialBlob = blob.as_mut_ptr();
    credential.CredentialBlobSize = blob.len() as u32;
    credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
    let ok = unsafe { CredWriteW(&credential, 0) } != 0;
    let result = if ok { Ok(()) } else { Err(error()) };
    blob.fill(0);
    result
}
pub(super) fn remove(name: &str) -> Result<(), String> {
    let name = key(name)?;
    if unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) } != 0
        || unsafe { GetLastError() } == ERROR_NOT_FOUND
    {
        Ok(())
    } else {
        Err(error())
    }
}
