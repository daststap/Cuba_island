//! The Notification Center through `UserNotificationListener`. Current Windows 11
//! builds let ordinary (unpackaged) desktop apps use it; access is the privacy
//! switch "Let desktop apps access your notifications".

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use windows::core::Interface;
use windows::Foundation::Size;
use windows::Storage::Streams::{DataReader, IRandomAccessStream};
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};

use super::model::{split_texts, Item};
use super::Access;

const E_FAIL: windows::core::HRESULT = windows::core::HRESULT(0x8000_4005_u32 as i32);
/// Logos are shown at 20–28 logical px.
const LOGO_SIZE: f32 = 64.0;
/// Windows FILETIME of the Unix epoch, in 100 ns ticks.
const UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;

fn listener() -> windows::core::Result<UserNotificationListener> {
    UserNotificationListener::Current()
}

pub fn access() -> Access {
    match listener().and_then(|l| l.GetAccessStatus()) {
        Ok(UserNotificationListenerAccessStatus::Allowed) => Access::Allowed,
        Ok(UserNotificationListenerAccessStatus::Denied) => Access::Denied,
        Ok(_) => Access::Unspecified,
        Err(err) => {
            log::warn!("notification listener unavailable: {err}");
            Access::Denied
        }
    }
}

/// Shows Windows' own "allow access to notifications?" prompt; must run on the UI thread.
pub fn request_access(done: impl FnOnce(Access) + Send + 'static) {
    let asked = listener()
        .and_then(|l| l.RequestAccessAsync())
        .and_then(|operation| {
            operation.when(move |answer| {
                done(match answer {
                    Ok(UserNotificationListenerAccessStatus::Allowed) => Access::Allowed,
                    Ok(UserNotificationListenerAccessStatus::Denied) => Access::Denied,
                    _ => Access::Unspecified,
                });
            })
        });
    if let Err(err) = asked {
        log::warn!("could not ask for notification access: {err}");
    }
}

/// Reads app logos once per app and keeps them as PNG files.
pub struct Icons {
    dir: PathBuf,
    known: HashMap<String, Option<String>>,
}

impl Icons {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            known: HashMap::new(),
        }
    }

    fn get(&mut self, notification: &UserNotification, app_id: &str) -> Option<String> {
        if let Some(cached) = self.known.get(app_id) {
            return cached.clone();
        }
        let path = self.dir.join(format!("notif-{:016x}.png", hash(app_id)));
        let saved = path.exists() || save_logo(notification, &path).is_ok();
        let result = saved.then(|| path.display().to_string());
        self.known.insert(app_id.to_owned(), result.clone());
        result
    }
}

fn hash(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn save_logo(notification: &UserNotification, path: &Path) -> windows::core::Result<()> {
    let reference = notification.AppInfo()?.DisplayInfo()?.GetLogo(Size {
        Width: LOGO_SIZE,
        Height: LOGO_SIZE,
    })?;
    let stream: IRandomAccessStream = reference.OpenReadAsync()?.join()?.cast()?;
    let size = u32::try_from(stream.Size()?).unwrap_or(0);
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.join()?;
    let mut bytes = vec![0; size as usize];
    reader.ReadBytes(&mut bytes)?;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(path, bytes).map_err(|err| windows::core::Error::new(E_FAIL, err.to_string()))
}

fn texts(notification: &UserNotification) -> windows::core::Result<Vec<String>> {
    let binding = notification
        .Notification()?
        .Visual()?
        .GetBinding(&KnownNotificationBindings::ToastGeneric()?)?;
    let mut lines = Vec::new();
    for element in &binding.GetTextElements()? {
        lines.push(element.Text()?.to_string());
    }
    Ok(lines)
}

/// One notification, or `None` if Windows could not describe it.
fn item(notification: &UserNotification, icons: &mut Icons) -> Option<Item> {
    let info = notification.AppInfo().ok()?;
    let app = info
        .DisplayInfo()
        .and_then(|d| d.DisplayName())
        .map(|n| n.to_string())
        .unwrap_or_default();
    let app_id = info
        .AppUserModelId()
        .map(|id| id.to_string())
        .unwrap_or_default();
    let (title, body) = split_texts(&texts(notification).unwrap_or_default());
    let time = notification
        .CreationTime()
        .map(|t| (t.UniversalTime - UNIX_EPOCH_TICKS) / 10_000)
        .unwrap_or(0);
    Some(Item {
        id: notification.Id().ok()?,
        app: if app.is_empty() { app_id.clone() } else { app },
        title,
        body,
        time,
        icon: icons.get(notification, &app_id),
        app_id,
    })
}

/// Everything in the Notification Center now.
pub fn current(icons: &mut Icons) -> windows::core::Result<Vec<Item>> {
    let list = listener()?
        .GetNotificationsAsync(NotificationKinds::Toast)?
        .join()?;
    Ok((&list)
        .into_iter()
        .filter_map(|n| item(&n, icons))
        .collect())
}

/// The app id of a notification still in the center.
pub fn app_id(id: u32) -> Option<String> {
    listener()
        .ok()?
        .GetNotification(id)
        .ok()?
        .AppInfo()
        .ok()?
        .AppUserModelId()
        .ok()
        .map(|id| id.to_string())
}

pub fn remove(id: u32) {
    if let Err(err) = listener().and_then(|l| l.RemoveNotification(id)) {
        log::warn!("could not remove notification {id}: {err}");
    }
}

/// Opens the app that sent a notification (by its AppUserModelID).
pub fn open_app(app_id: &str) -> std::io::Result<()> {
    std::process::Command::new("explorer.exe")
        .arg(format!("shell:AppsFolder\\{app_id}"))
        .spawn()
        .map(|_| ())
}
