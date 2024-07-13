use clap::Parser;
use duxcore::error::Error;

pub fn parse_cli_args_scalable_controller() -> Result<CliArgsScalableController, Error> {
    Ok(CliArgsScalableController::parse())
}

/// Dux scalable use case (controller) : based on a task list and a host list, generate assignments and have them applied by workers
#[derive(Parser, Debug)]
#[command(arg_required_else_help(true))]
#[command(version)]
pub struct CliArgsScalableController {
    /// Path to configuration file
    #[arg(short, long)]
    pub conf: Option<String>,

    /// Path to TaskList file
    #[arg(short, long)]
    pub tasklist: Option<String>,

    /// Path to HostList file
    #[arg(short = 'l', long)]
    pub hostlist: Option<String>,

    /// Username to use on remote hosts
    #[arg(short, long)]
    pub user: Option<String>,

    /// Password to use on remote hosts
    #[arg(short, long)]
    pub password: Option<String>,

    /// Path to private SSH2 key to use
    #[arg(short = 'k', long)]
    pub key: Option<String>,
}

