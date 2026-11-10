use anyhow::Result;
use bestquark_paintings::{Paths, assets, config, install, rotation};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "paintings",
    version,
    about = "Existing art for desktops, terminals, and web apps"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Tick {
        #[arg(long)]
        stream: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },
    Next {
        #[arg(long)]
        stream: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },
    Current,
    Status,
    Streams,
    Configure {
        #[arg(long)]
        stream: Option<String>,
        #[arg(long)]
        fit: Option<String>,
        #[arg(long)]
        apply: bool,
    },
    Install {
        #[arg(long, default_value = "desktop")]
        adapters: String,
        #[arg(long)]
        config_url: Option<String>,
        #[arg(long)]
        stream: Option<String>,
        #[arg(long)]
        no_schedule: bool,
    },
    InstallWeb {
        #[arg(long)]
        root: PathBuf,
    },
    Uninstall,
    Download {
        #[arg(long)]
        stream: Option<String>,
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    Gallery {
        #[arg(long, default_value = "dist/gallery")]
        output: PathBuf,
    },
}
fn execute(command: Commands) -> Result<Value> {
    let paths = Paths::discover()?;
    Ok(match command {
        Commands::Tick { stream, id } => {
            rotation::tick(&paths, stream.as_deref(), id.as_deref(), false)?
        }
        Commands::Next { stream, id } => {
            rotation::tick(&paths, stream.as_deref(), id.as_deref(), true)?
        }
        Commands::Current => rotation::current(&paths)?,
        Commands::Status => config::status(&paths)?,
        Commands::Streams => {
            let mut d = json!({});
            for s in assets::STREAMS {
                d[s] = json!(assets::catalog(s)?.len());
            }
            d
        }
        Commands::Configure { stream, fit, apply } => {
            config::configure(&paths, stream.as_deref(), fit.as_deref(), apply)?
        }
        Commands::Install {
            adapters,
            config_url,
            stream,
            no_schedule,
        } => install::install(
            &paths,
            &adapters,
            stream.as_deref(),
            config_url.as_deref(),
            !no_schedule,
        )?,
        Commands::InstallWeb { root } => install::install_web(&paths, &root)?,
        Commands::Uninstall => install::uninstall(&paths)?,
        Commands::Gallery { output } => {
            install::build_gallery(&output)?;
            json!({"gallery":output})
        }
        Commands::Download { stream, limit } => {
            let mut count = 0;
            let mut failed = vec![];
            for s in stream
                .map(|s| vec![s])
                .unwrap_or_else(|| assets::STREAMS.iter().map(|s| s.to_string()).collect())
            {
                for a in assets::catalog(&s)?.into_iter().take(if limit == 0 {
                    usize::MAX
                } else {
                    limit
                }) {
                    match rotation::download(&paths, &a) {
                        Ok(_) => count += 1,
                        Err(e) => failed.push(json!({"id":a["id"],"error":e.to_string()})),
                    }
                }
            }
            json!({"downloaded":count,"failed":failed})
        }
    })
}
fn main() {
    match execute(Cli::parse().command) {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("paintings: {e:#}");
            std::process::exit(1);
        }
    }
}
