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
    #[command(about = "Manage super admins")]
    Admin {
        #[arg(long, help = "Email of the person to make a super admin")]
        email: Option<String>,
    },
    #[command(about = "Back up or restore the data folder")]
    Backup,
    #[command(about = "Check every database and file")]
    Check,
}
