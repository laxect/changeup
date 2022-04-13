use std::path::PathBuf;

use clap::Parser;
use futures::FutureExt;

use changeup::{moniter, station, ChangeUp};

#[derive(Parser)]
struct Opts {
    // load config from targrt file
    #[clap(short, long)]
    config: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let Opts { config } = Opts::parse();

    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let change_up = if let Some(config_path) = config {
            ChangeUp::last_with_config(&config_path).await
        } else {
            ChangeUp::last().await
        }
        .unwrap();

        let end = futures::select! {
            x = station(change_up.clone()).fuse() => x,
            y = moniter(change_up).fuse() => y,
        };
        if let Err(e) = end {
            log::error!("E: {}", &e);
        }
    });

    Ok(())
}
