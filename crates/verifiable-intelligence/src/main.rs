//! Umbrella `vi` binary; thin wrapper around `vi_cli::run`.

#![cfg_attr(not(test), deny(unsafe_code))]

fn main() {
    std::process::exit(vi_cli::run());
}
