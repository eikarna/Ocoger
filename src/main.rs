mod core;
mod services;
mod ui;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "ocoger",
    version,
    about = "TUI manager for OpenCode agents, models, and process supervision"
)]
struct Cli {
    /// Path to the project directory containing `.opencode/`
    #[arg(short, long, default_value = ".")]
    project: std::path::PathBuf,
}

fn init_logging() {
    let file_appender = tracing_appender::rolling::never(".", ".ocoger.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    // Guard intentionally leaked into a static-ish lifetime via forget so the
    // background writer lives for the process duration.
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();
    std::mem::forget(_guard);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();
    let cli = Cli::parse();
    let project = cli.project.canonicalize().unwrap_or(cli.project);
    tracing::info!(project = %project.display(), "ocoger starting");

    // Scan .opencode/agents/*.md then parse each one; log but continue on errors.
    let mut agents = Vec::new();
    match core::agent_scanner::scan_agents(&project) {
        Ok(paths) => {
            for p in paths {
                match core::agent_parser::load_agent(&p) {
                    Ok(a) => agents.push(a),
                    Err(e) => tracing::warn!(path = %p.display(), error = %e, "skipping bad agent"),
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "agent scan failed"),
    }
    tracing::info!(count = agents.len(), "agents loaded");

    let app = ui::app::App::new(agents, project);
    ui::event_handler::run(app).await?;
    Ok(())
}
