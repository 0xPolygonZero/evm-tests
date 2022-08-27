#[macro_export]
macro_rules! unwrap_or_continue {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => continue,
        }
    };
}

#[macro_export]
macro_rules! unwrap_or_return {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => continue,
        }
    };
}

pub fn init_env_logger() {
    pretty_env_logger::init();
}
