#![forbid(unsafe_code)]

mod commands;
mod runtime;

pub fn run() {
    runtime::run();
}
