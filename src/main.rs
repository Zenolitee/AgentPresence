mod commands;
mod config;
mod detect;
mod discord;
mod format;
mod fsutil;
mod harnesses;
mod presence;
mod runtime;
mod tui;
mod window;

use std::env;
use std::io;

use crate::commands::{print_debug, print_status, publish_once};
use crate::config::Config;
use crate::discord::clear_all_presences;
use crate::runtime::{run_loop, run_tui_loop};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let mut config = Config::load()?;
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--ignore-opencode") {
        config.detect_opencode = false;
    }
    if args.iter().any(|a| a == "--ignore-claude") {
        config.detect_claude = false;
    }
    let command = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "tui".to_string());

    match command.as_str() {
        "status" => print_status(&config),
        "debug" => print_debug(&config),
        "once" => publish_once(&config),
        "clear" => clear_all_presences(&config),
        "run" => run_loop(&config),
        "tui" => run_tui_loop(config),
        _ => {
            eprintln!("usage: multi-agent-presence [--ignore-opencode] [--ignore-claude] [run|tui|once|status|debug|clear]");
            Ok(())
        }
    }
}
