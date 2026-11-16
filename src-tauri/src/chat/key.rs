//! The API key lives in Windows Credential Manager (encrypted for this user),
//! never in settings files and never sent to the page.

use windows::core::{w, PWSTR};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};

use crate::error::{AppError, AppResult};

/// Shown in Credential Manager as "Escape/anthropic-api-key".
const TARGET: windows::core::PCWSTR = w!("Escape/anthropic-api-key");

pub fn load() -> Option<String> {
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: on success `credential` points to a block owned by the system that we
    // read once and release with CredFree below.
    unsafe {
        CredReadW(TARGET, CRED_TYPE_GENERIC, None, &mut credential).ok()?;
        let cred = &*credential;
        if cred.CredentialBlob.is_null() || cred.CredentialBlobSize == 0 {
            CredFree(credential.cast());
            return None;
        }
        let blob =
            std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize);
        let key = String::from_utf8(blob.to_vec()).ok();
        CredFree(credential.cast());
        key.filter(|k| !k.is_empty())
    }
}

pub fn save(key: &str) -> AppResult<()> {
    let mut target: Vec<u16> = "Escape/anthropic-api-key\0".encode_utf16().collect();
    let mut blob = key.as_bytes().to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        CredentialBlobSize: u32::try_from(blob.len())
            .map_err(|_| AppError::InvalidArgument("слишком длинный ключ".into()))?,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    // SAFETY: every pointer in `credential` refers to buffers that outlive the call.
    unsafe { CredWriteW(&credential, 0) }
        .map_err(|err| AppError::Internal(format!("не удалось сохранить ключ: {err}")))
}

pub fn delete() {
    // SAFETY: plain call with a static name; a missing entry just fails.
    let _ = unsafe { CredDeleteW(TARGET, CRED_TYPE_GENERIC, None) };
}
