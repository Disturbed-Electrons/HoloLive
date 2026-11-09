use std::process::ExitCode;

use clap::Parser;
use hololive_app::{Arguments, run};

fn main() -> ExitCode {
    run(Arguments::parse())
}
