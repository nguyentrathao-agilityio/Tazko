use std::net::IpAddr;

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
    /// Run the web server
    Serve {
        /// Address to listen on
        #[arg(long, default_value = "127.0.0.1")]
        host: IpAddr,
        /// Port to listen on
        #[arg(long, default_value_t = 8791)]
        port: u16,
    },
    /// Manage super admins
    Admin {
        /// Email of the person to make a super admin
        #[arg(long)]
        email: Option<String>,
    },
    /// Back up or restore the data folder
    Backup,
    /// Check every database and file
    Check,
}
