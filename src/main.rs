use std::net::SocketAddr;
use std::process::ExitCode;

use clap::Parser;
use tazko::cli::{Cli, Command};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { host, port } => serve(SocketAddr::new(host, port)).await,
        Command::Admin { .. } => not_yet("admin"),
        Command::Backup => not_yet("backup"),
        Command::Check => not_yet("check"),
    }
}

async fn serve(addr: SocketAddr) -> ExitCode {
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("tazko: cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "tazko {} listening on http://{addr}",
        env!("CARGO_PKG_VERSION")
    );
    if let Err(e) = axum::serve(listener, tazko::http::router())
        .with_graceful_shutdown(shutdown())
        .await
    {
        eprintln!("tazko: server error: {e}");
        return ExitCode::FAILURE;
    }

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
