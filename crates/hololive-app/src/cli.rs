use std::{path::Path, process::ExitCode, thread};

use clap::{Parser, Subcommand, ValueEnum};
use hololive_core::Precision;
use temscript_client::TemscriptClient;
use temscript_simulator::{SIMULATED_CAMERA, SimulatorConfig, SimulatorHandle};

use crate::{AppConfig, ui::run_gui};

#[derive(Debug, Parser)]
#[command(
    name = "HoloLive",
    version,
    about = "Live off-axis electron-holography reconstruction"
)]
pub struct Arguments {
    /// Acquisition source used by the desktop application.
    #[arg(long, value_enum, default_value_t = Source::Simulated)]
    pub source: Source,
    /// Numeric precision selected once for the complete scientific pipeline.
    #[arg(long, value_enum, default_value_t = PrecisionArgument::F32)]
    pub precision: PrecisionArgument,
    /// Base URL of a live temscript server.
    #[arg(long)]
    pub server: Option<String>,
    /// Camera name exposed by temscript.
    #[arg(long)]
    pub camera: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum Source {
    Simulated,
    Live,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PrecisionArgument {
    F32,
    F64,
}

impl From<PrecisionArgument> for Precision {
    fn from(value: PrecisionArgument) -> Self {
        match value {
            PrecisionArgument::F32 => Self::F32,
            PrecisionArgument::F64 => Self::F64,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run supporting development services.
    Simulator {
        #[command(subcommand)]
        command: SimulatorCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SimulatorCommand {
    /// Serve a standalone temscript-compatible synthetic camera.
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8080)]
        port: u16,
        #[arg(long)]
        width: Option<usize>,
        #[arg(long)]
        height: Option<usize>,
        #[arg(long)]
        fps: Option<f64>,
        #[arg(long)]
        jitter: Option<f64>,
    },
}

pub fn run(arguments: Arguments) -> ExitCode {
    if let Some(Command::Simulator { command }) = arguments.command {
        return run_simulator(command);
    }

    let mut config = AppConfig::default();
    if let Some(server) = arguments.server {
        config.live_server = server;
    }
    if let Some(camera) = arguments.camera {
        config.camera = camera;
    }
    let (client, simulator) = match open_source(arguments.source, &mut config) {
        Ok(source) => source,
        Err(error) => return fail(&error),
    };
    if arguments.source == Source::Simulated
        && let Err(error) = load_camera_capabilities(&client, &mut config)
    {
        return fail(&format!("failed to query camera capabilities: {error}"));
    }
    match run_gui(arguments.precision.into(), client, simulator, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail(&format!("HoloLive failed: {error}")),
    }
}

fn fail(error: &str) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}

fn open_source(
    source: Source,
    config: &mut AppConfig,
) -> Result<(TemscriptClient, Option<SimulatorHandle>), String> {
    match source {
        Source::Simulated => build_simulated_client(config),
        Source::Live => build_live_client(config),
    }
}

fn build_simulated_client(
    config: &mut AppConfig,
) -> Result<(TemscriptClient, Option<SimulatorHandle>), String> {
    config.camera = SIMULATED_CAMERA.into();
    let (path, simulator_config) =
        SimulatorConfig::load_fixture().map_err(|error| error.to_string())?;
    report_simulation(path.as_deref(), &simulator_config);
    config.exposure_seconds = simulator_config.exposure_seconds;
    config.binning = simulator_config.binning;
    let simulator = SimulatorHandle::start("127.0.0.1:0", simulator_config)
        .map_err(|error| format!("failed to start simulator: {error}"))?;
    let client = TemscriptClient::new(simulator.address())
        .map_err(|error| format!("failed to create simulator client: {error}"))?;
    Ok((client, Some(simulator)))
}

fn load_camera_capabilities(
    client: &TemscriptClient,
    config: &mut AppConfig,
) -> Result<(), String> {
    let camera = client
        .camera_info(&config.camera)
        .map_err(|error| error.to_string())?;
    config.apply_camera_info(&camera).map_err(str::to_owned)
}

fn build_live_client(
    config: &AppConfig,
) -> Result<(TemscriptClient, Option<SimulatorHandle>), String> {
    TemscriptClient::new(&config.live_server)
        .map(|client| (client, None))
        .map_err(|error| format!("invalid temscript server URL: {error}"))
}

pub(crate) fn run_simulator(command: SimulatorCommand) -> ExitCode {
    match command {
        SimulatorCommand::Serve {
            host,
            port,
            width,
            height,
            fps,
            jitter,
        } => {
            if host != "127.0.0.1" && host != "localhost" && host != "::1" {
                eprintln!(
                    "warning: the simulator has no authentication and is exposed beyond loopback"
                );
            }
            let (path, mut config) = match SimulatorConfig::load_fixture() {
                Ok(fixture) => fixture,
                Err(error) => return fail(&error.to_string()),
            };
            if let Some(width) = width {
                config.simulation.width = width;
            }
            if let Some(height) = height {
                config.simulation.height = height;
            }
            if let Some(fps) = fps {
                config.simulation.target_fps = fps;
            }
            if let Some(jitter) = jitter {
                config.simulation.jitter_fraction = jitter;
            }
            report_simulation(path.as_deref(), &config);
            let bind = if host.contains(':') && !host.starts_with('[') {
                format!("[{host}]:{port}")
            } else {
                format!("{host}:{port}")
            };
            match SimulatorHandle::start(&bind, config) {
                Ok(server) => {
                    println!("HoloLive simulator listening on {}", server.address());
                    println!("Press Ctrl-C to stop.");
                    loop {
                        thread::park();
                    }
                }
                Err(error) => fail(&format!("failed to start simulator: {error}")),
            }
        }
    }
}

fn report_simulation(path: Option<&Path>, config: &SimulatorConfig) {
    eprintln!(
        "simulation: {} ({}×{}, {:.0} FPS, scene {:?})",
        simulation_source(path),
        config.simulation.width,
        config.simulation.height,
        config.simulation.target_fps,
        config.simulation.scene
    );
}

fn simulation_source(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "built-in defaults".to_owned())
}
