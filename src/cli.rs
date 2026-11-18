use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
            help = "Data folder (default: TAZKO_DATA_DIR from the settings file)"
        )]
        data: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Settings file, used when --data is not given"
        )]
        env_file: Option<PathBuf>,
    },
    #[command(about = "Back up or restore the data folder")]
    Backup,
    #[command(about = "Check every database and file")]
    Check,
}
