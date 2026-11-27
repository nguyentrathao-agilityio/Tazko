use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use tazko::admin::cli::Granted;
use tazko::backup;
use tazko::cli::{BackupAction, Cli, Command, DevAction, TemplatesAction};
use tazko::config::{self, DataLayout};
use tazko::db::Databases;
use tazko::state::AppState;

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Client(c) => tazko::client::run(c),
        server => match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(run_server(server)),
            Err(e) => {
                eprintln!("tazko: cannot start: {e}");
                ExitCode::FAILURE
            }
        },
    }
}

async fn run_server(command: Command) -> ExitCode {
    match command {
        Command::Client(_) => unreachable!("client commands run without the server runtime"),
        Command::Serve { env_file } => serve(env_file.as_deref()).await,
        Command::Admin {
            email,
            data,
            env_file,
        } => admin(&email, data, env_file.as_deref()).await,
        Command::Backup {
            action,
            data,
            to,
            keep_days,
            env_file,
        } => run_backup(action, data, to, keep_days, env_file),
        Command::Check { data, env_file } => run_check(data, env_file),
        Command::Templates {
            action:
                TemplatesAction::CleanRestricted {
                    apply,
                    data,
                    env_file,
                },
        } => clean_templates(apply, data, env_file.as_deref()).await,
        Command::Dev {
            action: DevAction::Seed { owner, env_file },
        } => dev_seed(owner.as_deref(), env_file.as_deref()).await,
        Command::Dev {
            action: DevAction::Mail { port, dir },
        } => dev_mail(port, dir),
    }
}

fn dev_mail(port: u16, dir: PathBuf) -> ExitCode {
    let listener = match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("tazko dev mail: cannot listen on 127.0.0.1:{port}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "tazko dev mail: listening on 127.0.0.1:{port}, saving emails to {}",
        dir.display()
    );
    println!(
        "Set TAZKO_SMTP_HOST=127.0.0.1, TAZKO_SMTP_PORT={port} and TAZKO_MAIL_FROM in the settings file."
    );
    tazko::mail::stub::serve(listener, dir);
    ExitCode::SUCCESS
}

async fn dev_seed(owner: Option<&str>, env_file: Option<&Path>) -> ExitCode {
    let settings = match config::load(env_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tazko dev seed: {e}");
            return ExitCode::FAILURE;
        }
    };
    if !tazko::devseed::local(&settings.public_url) {
        eprintln!("tazko dev seed: refusing, TAZKO_PUBLIC_URL is not a local address");
        return ExitCode::FAILURE;
    }

    let layout = DataLayout::new(&settings.data_dir);
    if let Err(e) = layout.create() {
        eprintln!("tazko dev seed: cannot prepare the data folder: {e}");
        return ExitCode::FAILURE;
    }

    let secret = match config::server_secret(&settings, &layout) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tazko dev seed: cannot load the server secret: {e}");
            return ExitCode::FAILURE;
        }
    };
    let dbs = match Databases::open(layout) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("tazko dev seed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let public = settings.public_url.trim_end_matches('/').to_string();
    let state = AppState::new(settings, dbs, secret);
    let outcome = tazko::devseed::seed(&state, owner).await;
    state.dbs.close();
    match outcome {
        Ok(tazko::devseed::Seeded::Created { board, .. }) => {
            println!(
                "Seeded the Tazko Demo company. Open {public}/c/{}/b/{board}",
                tazko::devseed::SLUG
            );
            ExitCode::SUCCESS
        }
        Ok(tazko::devseed::Seeded::AlreadyThere) => {
            println!("The Tazko Demo company is already there; nothing changed.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tazko dev seed: {e}");
            ExitCode::FAILURE
        }
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

    let secret = match config::server_secret(&settings, &layout) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tazko: cannot load the server secret: {e}");
            return ExitCode::FAILURE;
        }
    };

    let dbs = match Databases::open(layout) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("tazko: cannot open control.db: {e}");
            return ExitCode::FAILURE;
        }
    };

    let addr = SocketAddr::new(settings.host, settings.port);
    let banner = config::startup_message(&settings);
    let state = AppState::new(settings, dbs, secret);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("tazko: cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("{banner}");
    tokio::spawn(purge_trash_daily(state.clone()));
    tokio::spawn(tazko::notifications::due_soon_job(state.clone()));
    tokio::spawn(tazko::webhooks::worker::job(state.clone()));
    tokio::spawn(tazko::mail::job(state.clone()));
    tokio::spawn(tazko::notifications::email::job(state.clone()));
    if let Err(e) = axum::serve(listener, tazko::http::router(state.clone()))
        .with_graceful_shutdown(shutdown())
        .await
    {
        eprintln!("tazko: server error: {e}");
        state.dbs.close();
        return ExitCode::FAILURE;
    }

    state.dbs.close();
    ExitCode::SUCCESS
}

async fn admin(email: &str, data: Option<PathBuf>, env_file: Option<&Path>) -> ExitCode {
    let data = match data_dir(data, env_file) {
        Ok(d) => d,
        Err(code) => return code,
    };
    match tazko::admin::cli::grant_super_admin(&data, email).await {
        Ok((email, Granted::Already)) => println!("{email} is already a super admin."),
        Ok((email, Granted::Promoted)) => println!("{email} is now a super admin."),
        Ok((email, Granted::Added)) => {
            println!(
                "{email} is now a super admin. Sign in with that Google account and open /admin."
            )
        }
        Err(e) => {
            eprintln!("tazko admin: {e}");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

async fn clean_templates(apply: bool, data: Option<PathBuf>, env_file: Option<&Path>) -> ExitCode {
    let data = match data_dir(data, env_file) {
        Ok(d) => d,
        Err(code) => return code,
    };
    let dbs = match Databases::open(DataLayout::new(&data)) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("tazko templates: cannot open {}: {e}", data.display());
            return ExitCode::FAILURE;
        }
    };
    let result = tazko::templates::clean::restricted(&dbs, apply).await;
    dbs.close();
    let found = match result {
        Ok(f) => f,
        Err(e) => {
            eprintln!("tazko templates: {e}");
            return ExitCode::FAILURE;
        }
    };
    for f in &found {
        println!(
            "{}: template \"{}\" (from restricted board {}): {} cards, {} shared copies",
            f.company, f.name, f.board, f.cards, f.shared_copies
        );
    }
    let copies: usize = found.iter().map(|f| f.shared_copies).sum();
    match (found.is_empty(), apply) {
        (true, _) => println!("No template from a restricted board has cards."),
        (false, true) => println!(
            "Removed the cards from {} templates and {copies} shared copies.",
            found.len()
        ),
        (false, false) => println!(
            "{} templates and {copies} shared copies would lose their cards. Run again with --apply to remove them.",
            found.len()
        ),
    }

    ExitCode::SUCCESS
}

async fn purge_trash_daily(state: AppState) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
    loop {
        tick.tick().await;
        let now = tazko::session::now();
        match tazko::admin::trash::purge(state.dbs.layout(), state.dbs.control(), now).await {
            Ok(removed) if !removed.is_empty() => {
                println!("tazko: removed from trash: {}", removed.join(", "))
            }
            Ok(_) => {}
            Err(e) => eprintln!("tazko: trash clean-up failed: {e}"),
        }
        purge_projects(&state, now).await;
    }
}

async fn purge_projects(state: &AppState, now: i64) {
    for (slug, db) in tazko::tenant::active_tenants(state).await {
        if let Err(e) = tazko::projects::purge(&db, now).await {
            eprintln!("tazko: project clean-up failed for {slug}: {e:?}");
        }
        let dir = state.dbs.layout().orgs.join(&slug);
        if let Err(e) = tazko::files::sweep(&db, dir).await {
            eprintln!("tazko: file clean-up failed for {slug}: {e:?}");
        }
    }
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

fn data_dir(data: Option<PathBuf>, env_file: Option<&Path>) -> Result<PathBuf, ExitCode> {
    if let Some(d) = data {
        return Ok(d);
    }

    match config::load(env_file) {
        Ok(s) => Ok(s.data_dir),
        Err(e) => {
            eprintln!("tazko: {e}");
            eprintln!("tazko: or pass --data <folder>");
            Err(ExitCode::FAILURE)
        }
    }
}

fn report(kind: &str, s: &backup::Status) -> ExitCode {
    let name = s.snapshot.as_deref().unwrap_or("-");
    if s.ok {
        println!(
            "{kind} ok: {name}, {} databases, {} files, {} bytes",
            s.databases, s.files, s.bytes
        );
        return ExitCode::SUCCESS;
    }

    eprintln!("{kind} FAILED: {name}");
    for p in &s.problems {
        eprintln!("  - {p}");
    }

    ExitCode::FAILURE
}

fn run_backup(
    action: Option<BackupAction>,
    data: Option<PathBuf>,
    to: Option<PathBuf>,
    keep_days: i64,
    env_file: Option<PathBuf>,
) -> ExitCode {
    match action {
        Some(BackupAction::Restore { from, snapshot, to }) => {
            match backup::restore(&from, snapshot.as_deref(), &to) {
                Ok(name) => {
                    let checked = backup::check(&to);
                    let status = backup::Status {
                        at: tazko::session::now(),
                        ok: checked.ok(),
                        snapshot: Some(name),
                        databases: checked.databases,
                        files: checked.files,
                        bytes: 0,
                        problems: checked.problems,
                    };
                    report("restore", &status)
                }
                Err(e) => {
                    eprintln!("tazko backup restore: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some(BackupAction::Verify {
            from,
            data,
            scratch,
            env_file,
        }) => {
            let data = match data_dir(data, env_file.as_deref()) {
                Ok(d) => d,
                Err(code) => return code,
            };
            let scratch = scratch.unwrap_or_else(std::env::temp_dir);
            let status = backup::run_restore_test(&from, &scratch);
            if let Err(e) = backup::write_status(&data, "restore-test", &status) {
                eprintln!("tazko: cannot record the restore test result: {e}");
            }

            report("restore test", &status)
        }
        None => {
            let Some(to) = to else {
                eprintln!("tazko backup: --to <folder> is required");
                return ExitCode::from(2);
            };
            let data = match data_dir(data, env_file.as_deref()) {
                Ok(d) => d,
                Err(code) => return code,
            };
            let status = backup::run_backup(&data, &to, keep_days);
            if let Err(e) = backup::write_status(&data, "backup", &status) {
                eprintln!("tazko: cannot record the backup result: {e}");
            }

            report("backup", &status)
        }
    }
}

fn run_check(data: Option<PathBuf>, env_file: Option<PathBuf>) -> ExitCode {
    let data = match data_dir(data, env_file.as_deref()) {
        Ok(d) => d,
        Err(code) => return code,
    };
    let r = backup::check(&data);
    if r.ok() {
        println!("check ok: {} databases, {} files", r.databases, r.files);
        return ExitCode::SUCCESS;
    }

    eprintln!("check FAILED: {} problems", r.problems.len());
    for p in &r.problems {
        eprintln!("  - {p}");
    }

    ExitCode::FAILURE
}
