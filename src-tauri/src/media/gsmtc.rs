//! The media worker: owns the GSMTC session manager, tracks the current session and
//! turns its events into `MediaState` updates.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

use super::model::{filetime_to_unix_ms, ticks_to_ms, track_id, MediaCommand, MediaState};
use super::{cover, Msg};
use crate::error::{AppError, AppResult};

/// WinRT sends events in bursts (metadata, then artwork, then timeline); wait for quiet.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// Runs until the channel closes. Errors (e.g. GSMTC unavailable) bubble up to the caller.
pub fn run(
    tx: &Sender<Msg>,
    rx: &Receiver<Msg>,
    publish: &dyn Fn(Option<MediaState>),
) -> windows::core::Result<()> {
    // SAFETY: initialises WinRT for this thread only; a repeated call is harmless.
    if let Err(err) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        log::debug!("RoInitialize: {err}");
    }

    let manager = Manager::RequestAsync()?.join()?;
    let notify = tx.clone();
    let session_changed = manager.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
        let _ = notify.send(Msg::Changed);
        Ok(())
    }))?;

    let mut worker = Worker {
        manager,
        tx: tx.clone(),
        subscription: None,
        cover: CoverCache::default(),
        last: None,
    };
    worker.refresh(publish);

    loop {
        match rx.recv() {
            Ok(Msg::Changed) => {
                if !worker.debounce(rx) {
                    break;
                }
                worker.refresh(publish);
            }
            Ok(Msg::Command(cmd, reply)) => {
                let _ = reply.send(worker.execute(cmd));
            }
            Err(_) => break,
        }
    }

    worker.unsubscribe();
    worker
        .manager
        .RemoveCurrentSessionChanged(session_changed)?;
    Ok(())
}

struct Subscription {
    session: Session,
    app_id: String,
    tokens: [i64; 3],
}

#[derive(Default)]
struct CoverCache {
    track_id: String,
    cover: Option<String>,
}

struct Worker {
    manager: Manager,
    tx: Sender<Msg>,
    subscription: Option<Subscription>,
    cover: CoverCache,
    last: Option<MediaState>,
}

impl Worker {
    /// Waits until no `Changed` arrived for `DEBOUNCE`, still serving commands.
    /// Returns `false` when the channel is closed.
    fn debounce(&self, rx: &Receiver<Msg>) -> bool {
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(Msg::Changed) => {}
                Ok(Msg::Command(cmd, reply)) => {
                    let _ = reply.send(self.execute(cmd));
                }
                Err(RecvTimeoutError::Timeout) => return true,
                Err(RecvTimeoutError::Disconnected) => return false,
            }
        }
    }

    fn refresh(&mut self, publish: &dyn Fn(Option<MediaState>)) {
        let session = self.manager.GetCurrentSession().ok();
        self.resubscribe(session.as_ref());

        let state = match &self.subscription {
            Some(sub) => read_state(sub, &mut self.cover).unwrap_or_else(|err| {
                log::warn!("failed to read media state of {}: {err}", sub.app_id);
                None
            }),
            None => None,
        };
        if state != self.last {
            self.last = state.clone();
            publish(state);
        }
    }

    /// Moves the session event subscriptions to `session` if the current session changed.
    fn resubscribe(&mut self, session: Option<&Session>) {
        let app_id = session
            .and_then(|s| s.SourceAppUserModelId().ok())
            .map(|id| id.to_string());
        let current = self.subscription.as_ref().map(|s| s.app_id.as_str());
        if current == app_id.as_deref() {
            return;
        }
        self.unsubscribe();
        let (Some(session), Some(app_id)) = (session, app_id) else {
            return;
        };
        match self.subscribe(session) {
            Ok(tokens) => {
                log::info!("media session: {app_id}");
                self.subscription = Some(Subscription {
                    session: session.clone(),
                    app_id,
                    tokens,
                });
            }
            Err(err) => log::warn!("failed to subscribe to media session {app_id}: {err}"),
        }
    }

    fn subscribe(&self, session: &Session) -> windows::core::Result<[i64; 3]> {
        Ok([
            session.MediaPropertiesChanged(&notify_handler(&self.tx))?,
            session.PlaybackInfoChanged(&notify_handler(&self.tx))?,
            session.TimelinePropertiesChanged(&notify_handler(&self.tx))?,
        ])
    }

    fn unsubscribe(&mut self) {
        if let Some(sub) = self.subscription.take() {
            let [media, playback, timeline] = sub.tokens;
            let _ = sub.session.RemoveMediaPropertiesChanged(media);
            let _ = sub.session.RemovePlaybackInfoChanged(playback);
            let _ = sub.session.RemoveTimelinePropertiesChanged(timeline);
        }
    }

    fn execute(&self, cmd: MediaCommand) -> AppResult<()> {
        let session = self
            .manager
            .GetCurrentSession()
            .map_err(|_| AppError::Unavailable("Нет активного плеера".into()))?;
        let operation = match cmd {
            MediaCommand::Toggle => session.TryTogglePlayPauseAsync()?,
            MediaCommand::Next => session.TrySkipNextAsync()?,
            MediaCommand::Prev => session.TrySkipPreviousAsync()?,
        };
        if operation.join()? {
            Ok(())
        } else {
            Err(AppError::Unavailable(
                "Плеер не поддерживает это действие".into(),
            ))
        }
    }
}

/// An event handler that only pokes the worker; the args type differs per event.
fn notify_handler<Args>(tx: &Sender<Msg>) -> TypedEventHandler<Session, Args>
where
    Args: windows::core::RuntimeType + 'static,
{
    let tx = tx.clone();
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(Msg::Changed);
        Ok(())
    })
}

fn read_state(
    sub: &Subscription,
    cover_cache: &mut CoverCache,
) -> windows::core::Result<Option<MediaState>> {
    let session = &sub.session;
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    let title = props.Title()?.to_string();
    if title.trim().is_empty() {
        return Ok(None);
    }
    let artist = props.Artist()?.to_string();
    let album = props.AlbumTitle()?.to_string();
    let is_playing = session.GetPlaybackInfo()?.PlaybackStatus()? == PlaybackStatus::Playing;

    let timeline = session.GetTimelineProperties()?;
    let start = timeline.StartTime()?.Duration;
    let duration_ms = ticks_to_ms(timeline.EndTime()?.Duration - start);
    let position_ms = ticks_to_ms(timeline.Position()?.Duration - start).min(duration_ms);
    let position_updated_at =
        filetime_to_unix_ms(timeline.LastUpdatedTime()?.UniversalTime).unwrap_or_else(now_ms);

    let track_id = track_id(&sub.app_id, &title, &artist, &album);
    if cover_cache.track_id != track_id || cover_cache.cover.is_none() {
        // Artwork often arrives after the title, so a missing cover is retried later.
        let cover = props
            .Thumbnail()
            .ok()
            .and_then(|thumb| match cover::load(&thumb) {
                Ok(cover) => cover,
                Err(err) => {
                    log::debug!("no cover for {title}: {err}");
                    None
                }
            });
        *cover_cache = CoverCache {
            track_id: track_id.clone(),
            cover,
        };
    }

    Ok(Some(MediaState {
        track_id,
        title,
        artist,
        album,
        app_id: sub.app_id.clone(),
        is_playing,
        position_ms,
        duration_ms,
        position_updated_at,
        cover: cover_cache.cover.clone(),
    }))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
