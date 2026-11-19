use std::path::PathBuf;

use clap::{Parser, Subcommand};

const DATA_HELP: &str = "Data folder (default: TAZKO_DATA_DIR from the settings file)";
const ENV_FILE_HELP: &str = "Settings file, used when --data is not given";
const SNAPSHOTS_HELP: &str = "Folder that holds the snapshots";

#[derive(Parser, Debug)]
#[command(
    name = "tazko",
    version,
    about = "Tazko work tracker: server and command line"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(about = "Run the web server")]
    Serve {
        #[arg(
            long,
            value_name = "PATH",
            help = "Settings file (default: TAZKO_ENV_FILE, then .env.local, then /etc/tazko/tazko.env)"
        )]
        env_file: Option<PathBuf>,
    },
    #[command(about = "Make a person a super admin, adding them if they are new")]
    Admin {
        #[arg(long, help = "Google email address of the person")]
        email: String,
        #[arg(
            long,
            value_name = "DIR",
            help = DATA_HELP
        )]
        data: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = ENV_FILE_HELP
        )]
        env_file: Option<PathBuf>,
    },
    #[command(about = "Back up the data folder into a dated snapshot, then check it")]
    Backup {
        #[command(subcommand)]
        action: Option<BackupAction>,
        #[arg(
            long,
            value_name = "DIR",
            help = DATA_HELP
        )]
        data: Option<PathBuf>,
        #[arg(long, value_name = "DIR", help = SNAPSHOTS_HELP)]
        to: Option<PathBuf>,
        #[arg(
            long,
            default_value_t = crate::backup::DEFAULT_KEEP_DAYS,
            help = "Remove snapshots older than this many days"
        )]
        keep_days: i64,
        #[arg(
            long,
            value_name = "PATH",
            help = ENV_FILE_HELP
        )]
        env_file: Option<PathBuf>,
    },
    #[command(about = "Tools for local development")]
    Dev {
        #[command(subcommand)]
        action: DevAction,
    },
    #[command(about = "Check every database and stored file in a data folder")]
    Check {
        #[arg(
            long,
            value_name = "DIR",
            help = DATA_HELP
        )]
        data: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = ENV_FILE_HELP
        )]
        env_file: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DevAction {
    #[command(about = "Add a demo company with a project, boards and cards (local addresses only)")]
    Seed {
        #[arg(
            long,
            value_name = "EMAIL",
            help = "Super admin who owns the demo company (default: the first super admin)"
        )]
        owner: Option<String>,
        #[arg(long, value_name = "PATH", help = "Settings file")]
        env_file: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum BackupAction {
    #[command(about = "Restore a snapshot into a new, empty folder and check it")]
    Restore {
        #[arg(long, value_name = "DIR", help = SNAPSHOTS_HELP)]
        from: PathBuf,
        #[arg(
            long,
            value_name = "NAME",
            help = "Snapshot to restore (default: the newest)"
        )]
        snapshot: Option<String>,
        #[arg(long, value_name = "DIR", help = "New data folder to restore into")]
        to: PathBuf,
    },
    #[command(
        about = "Restore the newest snapshot into a scratch folder, check it and record the result"
    )]
    Verify {
        #[arg(long, value_name = "DIR", help = SNAPSHOTS_HELP)]
        from: PathBuf,
        #[arg(
            long,
            value_name = "DIR",
            help = "Live data folder where the result is recorded"
        )]
        data: Option<PathBuf>,
        #[arg(
            long,
            value_name = "DIR",
            help = "Scratch folder (default: the system temporary folder)"
        )]
        scratch: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = ENV_FILE_HELP
        )]
        env_file: Option<PathBuf>,
    },
}
