use flexi_logger::Logger;

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
    let _ = Logger::try_with_env_or_str("plonky2::util::timing=info")
        .unwrap()
        .start();
}
