use std::process::ExitCode;

// Binary entrypoints cannot be declared `const fn` on stable Rust.
#[allow(clippy::missing_const_for_fn)]
fn main() -> ExitCode {
    ExitCode::SUCCESS
}
