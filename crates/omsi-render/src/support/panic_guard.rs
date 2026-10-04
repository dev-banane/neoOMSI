thread_local! {
    pub(crate) static CATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn catching() -> bool {
    CATCHING.get()
}

pub fn catch<R>(f: impl FnOnce() -> R) -> Option<R> {
    let was = CATCHING.replace(true);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CATCHING.set(was);
    r.ok()
}
