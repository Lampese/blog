use anyhow::{Context, Result};
use blog::{
    build::{self, BuildOptions},
    project, server, state,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Rust + Typst static blog")]
struct Args {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Inspect or recover managed reference state
    State {
        #[command(subcommand)]
        command: StateCommand,
    },
    /// Generate static pages, preserving previous output on compilation or reference errors
    Build {
        #[arg(long)]
        drafts: bool,
        #[arg(long)]
        check: bool,
    },
    /// Preview locally with automatic rebuilding and selection-based references
    Serve {
        #[arg(long, default_value_t = 7878)]
        port: u16,
    },
    /// Create a Typst article
    New {
        slug: String,
        #[arg(long)]
        title: Option<String>,
    },
    /// Initialize a site for this generator
    Init { directory: PathBuf },
    /// Capture a rendered content range using UTF-16 offsets
    Capture {
        article: String,
        start: usize,
        end: usize,
    },
}

#[derive(Subcommand)]
enum StateCommand {
    /// Decode managed snapshots, including footer conflicts, without compiling
    Inspect { file: PathBuf },
    /// Reconcile snapshots against resolved source; save only with --write
    Recover {
        file: PathBuf,
        /// Additional article snapshots, e.g. files exported from Git revisions
        #[arg(long)]
        from: Vec<PathBuf>,
        #[arg(long)]
        write: bool,
    },
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    let args = Args::parse();
    if let Command::Init { directory } = &args.command {
        return project::init(directory);
    }
    let root = args
        .root
        .canonicalize()
        .context("Project directory not found")?;
    match args.command {
        Command::State { command } => match command {
            StateCommand::Inspect { file } => println!(
                "{}",
                serde_json::to_string_pretty(&state::inspect(&root.join(file))?)?
            ),
            StateCommand::Recover { file, from, write } => println!(
                "{}",
                serde_json::to_string_pretty(&build::recover_state(&root, &file, &from, write)?)?
            ),
        },
        Command::Build { drafts, check } => {
            let result = build::build(
                &root,
                BuildOptions {
                    drafts,
                    mode: if check {
                        build::BuildMode::Check
                    } else {
                        build::BuildMode::Publish
                    },
                },
            )?;
            eprintln!(
                "{} {} articles{}",
                if check { "Checked" } else { "Generated" },
                result.posts.len(),
                if check {
                    String::new()
                } else {
                    format!(" → {}", root.join(result.config.output).display())
                }
            );
        }
        Command::Serve { port } => server::serve(root, port).await?,
        Command::New { slug, title } => {
            project::new_post(&root, &slug, title)?;
        }
        Command::Capture {
            article,
            start,
            end,
        } => {
            let mut result = build::build(
                &root,
                BuildOptions {
                    drafts: true,
                    ..Default::default()
                },
            )?;
            let revision = result
                .posts
                .get(&article)
                .context("Article not found")?
                .body
                .revision()
                .to_owned();
            let include = build::register(
                &root,
                &mut result,
                &article,
                &revision,
                blog_ranges::TextRange::new(start..end),
            )?;
            println!("{}", include.code());
        }
        Command::Init { .. } => unreachable!(),
    }
    Ok(())
}
