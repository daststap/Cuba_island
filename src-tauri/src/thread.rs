//! Background threads that survive panics (PLAN.md §6).

use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Duration;

const RESTART_DELAY: Duration = Duration::from_secs(1);

/// Spawns a named background thread running `body`.
///
/// If `body` panics, the panic is logged and `body` is started again after a short
/// delay. `body` normally loops forever; returning ends the thread.
pub fn spawn_supervised<F>(name: &'static str, body: F)
where
    F: Fn() + Send + 'static,
{
    let spawned = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || run_supervised(name, &body, RESTART_DELAY));
    if let Err(err) = spawned {
        log::error!("failed to spawn thread `{name}`: {err}");
    }
}

fn run_supervised(name: &str, body: &dyn Fn(), restart_delay: Duration) {
    while let Err(payload) = catch_unwind(AssertUnwindSafe(body)) {
        log::error!(
            "thread `{name}` panicked: {}; restarting",
            panic_message(&*payload)
        );
        std::thread::sleep(restart_delay);
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("<non-string panic>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn restarts_body_after_panic() {
        let runs = AtomicUsize::new(0);
        run_supervised(
            "test",
            &|| {
                if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                    panic!("first run fails");
                }
            },
            Duration::ZERO,
        );
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }
}
