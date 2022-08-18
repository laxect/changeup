use std::{fmt::Display, path::PathBuf};

use changeup::ChangeUpEPProxyBlocking;
use clap::{Parser, Subcommand};
use color_eyre::eyre;
use zbus::blocking::Connection;

#[derive(Subcommand)]
enum Command {
    Init,
    Last,
    Config { path: PathBuf },
    Focus { target: String },
    RuleFocus { target: String },
}

#[derive(Parser)]
struct Opts {
    #[clap(subcommand)]
    cmd: Command,
}

fn print<T: Display>(item: T) {
    print!("{}", item)
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let Opts { cmd } = Opts::parse();

    let conn = Connection::session()?;
    let mut proxy = ChangeUpEPProxyBlocking::builder(&conn)
        .cache_properties(zbus::CacheProperties::No)
        .build()?;

    match cmd {
        Command::Init => proxy.ping().map(print)?,
        Command::Last => proxy.jump_to_last_viewed()?,
        Command::Config { path } => proxy.reload_config(&path.to_string_lossy()).map(print)?,
        Command::Focus { target } => proxy.focus(target)?,
        Command::RuleFocus { target } => proxy.rule_focus(target)?,
    }
    Ok(())
}
