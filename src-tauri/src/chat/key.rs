//! API keys live in Windows Credential Manager (encrypted for this user),
//! never in settings files and never sent to the page. One entry per provider,
//! shown in Credential Manager as "Escape/<provider>-api-key" (the app's old
//! name, kept so keys saved before the rename to Cuba still work).

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};

use super::Provider;
use crate::error::{AppError, AppResult};

fn target(provider: Provider) -> String {
    format!("Escape/{}-api-key", provider.id())
}

pub fn load(provider: Provider) -> Option<String> {
    let name = HSTRING::from(target(provider));
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: on success `credential` points to a block owned by the system that we
    // read once and release with CredFree below; `name` outlives the call.
    unsafe {
        CredReadW(&name, CRED_TYPE_GENERIC, None, &mut credential).ok()?;
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

pub fn save(provider: Provider, key: &str) -> AppResult<()> {
    let mut name: Vec<u16> = target(provider)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut blob = key.as_bytes().to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(name.as_mut_ptr()),
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

pub fn delete(provider: Provider) {
    let name = HSTRING::from(target(provider));
    // SAFETY: plain call; a missing entry just fails.
    let _ = unsafe { CredDeleteW(&name, CRED_TYPE_GENERIC, None) };
}
