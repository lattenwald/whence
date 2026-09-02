use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};

use whence::pos::Pos;
use whence::render::render_text;
use whence::server::{HostSource, replay_once, serve};
use whence::tree::Limits;

#[derive(Parser)]
#[command(name = "whence", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Speak the JSON-RPC host protocol on stdin/stdout.
    Serve,
    /// Trace against a recorded fixture directory.
    Replay {
        dir: PathBuf,
        /// `file:line:col`, 1-based, relative to the fixture directory.
        target: Option<String>,
        /// Serve the protocol on stdio, answering host requests from the fixture.
        #[arg(long)]
        serve: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        fanout: Option<u32>,
        #[arg(long)]
        depth: Option<u32>,
    },
}

fn main() -> ExitCode {
    env_logger::init();
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("whence: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Cmd::Serve => serve(
            BufReader::new(std::io::stdin()),
            std::io::stdout(),
            HostSource::Stdio,
        ),
        Cmd::Replay {
            dir,
            target,
            serve: as_server,
            json,
            fanout,
            depth,
        } => {
            if as_server {
                return serve(
                    BufReader::new(std::io::stdin()),
                    std::io::stdout(),
                    HostSource::Replay(dir),
                );
            }
            let Some(target) = target else {
                bail!("replay needs a file:line:col target (or --serve)")
            };
            let (file, pos) = parse_target(&target)?;
            let mut limits = Limits::default();
            if let Some(f) = fanout {
                limits.fanout = f;
            }
            if let Some(d) = depth {
                limits.depth = d;
            }
            let tree = replay_once(&dir, &dir.join(file), pos, limits)?;
            let text = if json {
                serde_json::to_string_pretty(&tree)?
            } else {
                render_text(&tree, &dir)
            };
            let mut out = std::io::stdout().lock();
            out.write_all(text.as_bytes())?;
            if json {
                out.write_all(b"\n")?;
            }
            Ok(())
        }
    }
}

fn parse_target(target: &str) -> anyhow::Result<(PathBuf, Pos)> {
    let (file, rest) = target
        .rsplit_once(':')
        .and_then(|(head, col)| head.rsplit_once(':').map(|(f, line)| (f, (line, col))))
        .with_context(|| format!("expected file:line:col, got {target}"))?;
    let line: u32 = rest.0.parse().context("line is not a number")?;
    let col: u32 = rest.1.parse().context("col is not a number")?;
    if line == 0 || col == 0 {
        bail!("line and col are 1-based");
    }
    Ok((
        PathBuf::from(file),
        Pos {
            line: line - 1,
            col: col - 1,
        },
    ))
}
