use std::{fs, process::ExitCode};

use clap::Parser;
use directories::ProjectDirs;
use hololive_app::{Arguments, init_tracing, run};

fn main() -> ExitCode {
    let log_directory = ProjectDirs::from("dev", "HoloSuite", "HoloLive")
        .map(|directories| directories.data_local_dir().join("logs"))
        .unwrap_or_else(|| "logs".into());
    let _ = fs::create_dir_all(&log_directory);
    let _log_guard = init_tracing(log_directory);
    run(Arguments::parse())
}
