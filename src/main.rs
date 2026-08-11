use clap::Parser;
use ocoger::{core, ui};

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
    // Default to DEBUG so the key/action diagnostics land in the log file.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
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

    // Scan project + global agent dirs, project shadowing global by filename.
    let mut agents = Vec::new();
    match core::agent_scanner::scan_agents_cascaded(&project) {
        Ok(entries) => {
            for e in &entries {
                match core::agent_parser::load_agent(&e.path) {
                    Ok(a) => {
                        let mut a = a;
                        a.origin = match e.origin {
                            core::agent_scanner::AgentOrigin::Project => {
                                core::agent_parser::AgentOrigin::Project
                            }
                            core::agent_scanner::AgentOrigin::Global => {
                                core::agent_parser::AgentOrigin::Global
                            }
                        };
                        agents.push(a);
                    }
                    Err(err) => {
                        tracing::warn!(path = %e.path.display(), error = %err, "skipping bad agent");
                    }
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "agent scan failed"),
    }
    tracing::info!(count = agents.len(), "agents loaded");
    for a in &agents {
        tracing::info!(
            agent = %a.path.display(),
            model = %a.frontmatter.model,
            selected = a.is_selected,
            "loaded agent"
        );
    }

    let app = ui::app::App::new(agents, project);
    ui::event_handler::run(app).await?;
    Ok(())
}
