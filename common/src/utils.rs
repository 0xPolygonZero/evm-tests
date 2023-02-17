use std::env;

use pretty_env_logger::env_logger::DEFAULT_FILTER_ENV;

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
    // So verbose... Why?
    let level = env::var(DEFAULT_FILTER_ENV)
        .ok()
        .unwrap_or_else(|| "plonky2::util::timing=info".to_string());

    let _ = pretty_env_logger::formatted_builder()
        .parse_filters(&level)
        .try_init();
}
