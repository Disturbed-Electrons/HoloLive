use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub(crate) struct LatestSlot<T> {
    state: Mutex<LatestState<T>>,
    ready: Condvar,
}

struct LatestState<T> {
    value: Option<T>,
    revision: u64,
}

impl<T> LatestSlot<T> {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(LatestState {
                value: None,
                revision: 0,
            }),
            ready: Condvar::new(),
        }
    }

    pub(crate) fn publish(&self, value: T) -> Option<T> {
        let mut state = self.state.lock().expect("latest-value lock poisoned");
        let previous = state.value.replace(value);
        state.revision = state.revision.wrapping_add(1);
        drop(state);
        self.ready.notify_one();
        previous
    }

    pub(crate) fn take(&self) -> Option<T> {
        self.state
            .lock()
            .expect("latest-value lock poisoned")
            .value
            .take()
    }

    pub(crate) fn wake(&self) {
        let mut state = self.state.lock().expect("latest-value lock poisoned");
        state.revision = state.revision.wrapping_add(1);
        drop(state);
        self.ready.notify_one();
    }

    pub(crate) fn wait_for_change(&self, revision: &mut u64, cancelled: &AtomicBool) {
        let mut state = self.state.lock().expect("latest-value lock poisoned");
        while state.revision == *revision && !cancelled.load(Ordering::Acquire) {
            state = self.ready.wait(state).expect("latest-value lock poisoned");
        }
        *revision = state.revision;
    }
}
