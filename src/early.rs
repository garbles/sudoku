//! A calculation that starts on a background thread before the app needs its result: the reading of
//! the ink at each pen lift (recognizer) and the puzzle of the next new game (generator). The result
//! is a function of the key only, thus a result that started early is the same as a result that
//! the app calculates when it needs it.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// One calculation at a time: a new start replaces the old one.
pub struct Early<K, V> {
    slot: Mutex<Option<(K, Arc<OnceLock<V>>)>>,
}

impl<K: PartialEq + Clone + Send + 'static, V: Clone + Send + Sync + 'static> Early<K, V> {
    pub const fn new() -> Early<K, V> {
        Early { slot: Mutex::new(None) }
    }

    fn slot(&self) -> MutexGuard<'_, Option<(K, Arc<OnceLock<V>>)>> {
        // The slot has no state that a panic can make incorrect.
        self.slot.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts `calc` for `key` on a background thread, if the calculation of the same key is not
    /// already started.
    pub fn start(&self, key: K, calc: impl FnOnce(&K) -> V + Send + 'static) {
        let mut slot = self.slot();
        if slot.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let result = Arc::new(OnceLock::new());
        *slot = Some((key.clone(), Arc::clone(&result)));
        std::thread::spawn(move || {
            result.get_or_init(|| calc(&key));
        });
    }

    /// The result for `key`: the result of the early calculation of the same key (this waits for its
    /// end), or else `calc`. If the thread did not start yet, `calc` gives the result, and the
    /// thread then does nothing.
    pub fn get(&self, key: &K, calc: impl FnOnce() -> V) -> V {
        let early = self.slot().take_if(|(k, _)| k == key);
        match early {
            Some((_, result)) => result.get_or_init(calc).clone(),
            None => calc(),
        }
    }

    /// Waits for the end of the early calculation, if there is one (`calc` calculates it if its
    /// thread did not start yet). The bench uses this: the device waits for the next input or timer
    /// while the calculation continues.
    pub fn finish(&self, calc: impl FnOnce(&K) -> V) {
        let early = self.slot().clone();
        if let Some((key, result)) = early {
            result.get_or_init(|| calc(&key));
        }
    }
}
