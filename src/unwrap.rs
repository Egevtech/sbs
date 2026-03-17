use crate::log;

pub trait SBSUnwrap<T> {
    fn log_unwrap(self, message: &str) -> T;
}

impl<T> SBSUnwrap<T> for Option<T> {
    fn log_unwrap(self, message: &str) -> T {
        match self {
            Some(x) => x,
            None => {
                log!(PANIC, "Unwrap failed: {}", message);
            },
        }
    }
}

