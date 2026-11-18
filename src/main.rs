use std::net::SocketAddr;
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use tazko::cli::{Cli, Command};
use tazko::config::{self, DataLayout};
use tazko::db::Databases;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { env_file } => serve(env_file.as_deref()).await,
        Command::Admin { .. } => not_yet("admin"),
        Command::Backup => not_yet("backup"),
        Command::Check => not_yet("check"),
    }
}

async fn serve(env_file: Option<&Path>) -> ExitCode {
    let settings = match config::load(env_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tazko: {e}");
            eprintln!("tazko: see docs/dev.md for the settings file");
            return ExitCode::FAILURE;
        }
    };
    let layout = DataLayout::new(&settings.data_dir);
    if let Err(e) = layout.create() {
        eprintln!(
            "tazko: cannot prepare the data folder {}: {e}",
            layout.root.display()
        );
        return ExitCode::FAILURE;
    }

    if let Err(e) = config::server_secret(&settings, &layout) {
        eprintln!("tazko: cannot load the server secret: {e}");
        return ExitCode::FAILURE;
    }

    let dbs = match Databases::open(layout) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("tazko: cannot open control.db: {e}");
            return ExitCode::FAILURE;
        }
    };

    let addr = SocketAddr::new(settings.host, settings.port);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("tazko: cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("{}", config::startup_message(&settings));
    if let Err(e) = axum::serve(listener, tazko::http::router())
        .with_graceful_shutdown(shutdown())
        .await
    {
        eprintln!("tazko: server error: {e}");
        dbs.close();
        return ExitCode::FAILURE;
    }

    dbs.close();
    ExitCode::SUCCESS
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
}

fn not_yet(name: &str) -> ExitCode {
    eprintln!("tazko {name}: not available in this version yet");
    ExitCode::FAILURE
}
