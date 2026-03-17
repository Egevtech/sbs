use crate::log;

pub trait SBSExpect<T, E> {
    fn log_expect(self, message: &str) -> T;
}

impl <T, E>SBSExpect<T, E> for std::result::Result<T, E>
where E: std::fmt::Display
{
    fn log_expect(self, message: &str) -> T {
        self.unwrap_or_else(|err| {
            let mut msg = String::from(message);
            if !msg.is_empty() {
                msg.insert(0, '\'');
                msg.push_str("': ");
            }

            log!(PANIC, "Expect catched: {} {}", msg, err);
        })
    }
}