use std::{path::PathBuf, time::Duration};

use clap::{Parser, Subcommand};
use dbus::blocking::{LocalConnection, Proxy};

#[derive(Subcommand)]
enum Command {
    Init,
    RuleFocus { target: String },
    Focus { target: String },
    Config { path: PathBuf },
}

#[derive(Parser)]
struct Opts {
    #[clap(subcommand)]
    cmd: Command,
}

fn init(proxy: &Proxy<&LocalConnection>) -> anyhow::Result<()> {
    proxy.method_call(changeup::NAME, "Ping", ())?;
    Ok(())
}

fn focus(proxy: &Proxy<&LocalConnection>, target: String) -> anyhow::Result<()> {
    proxy.method_call(changeup::NAME, changeup::FOCUS_METHOD, (target,))?;
    Ok(())
}

fn load_config(proxy: &Proxy<&LocalConnection>, path: PathBuf) -> anyhow::Result<()> {
    proxy.method_call(changeup::NAME, changeup::LOAD_CONFIG_METHOD, (path.to_string_lossy().to_string(),))?;
    Ok(())
}

fn focus_create_or_exec(proxy: &Proxy<&LocalConnection>, target: String) -> anyhow::Result<()> {
    proxy.method_call(changeup::NAME, changeup::FOCUS_CREATE_OR_JUMPBACK_METHOD, (target,))?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let Opts { cmd } = Opts::parse();

    let conn = LocalConnection::new_session()?;
    let proxy = conn.with_proxy(changeup::NAME, changeup::PATH, Duration::from_millis(200));

    match cmd {
        Command::Config { path } => load_config(&proxy, path)?,
        Command::Focus { target } => focus(&proxy, target)?,
        Command::RuleFocus { target } => focus_create_or_exec(&proxy, target)?,
        Command::Init => init(&proxy)?,
    }

    Ok(())
}
