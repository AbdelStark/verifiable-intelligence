//! Umbrella `vi` binary; thin wrapper around `vi_cli::run`.

fn main() {
    std::process::exit(vi_cli::process_main());
}
