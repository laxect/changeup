use std::path::PathBuf;

use changeup::{moniter, station, ChangeUp};

use clap::Parser;
use color_eyre::eyre;
use futures::FutureExt;
use tracing::error;

#[derive(Parser)]
struct Opts {
    // load config from targrt file
    #[clap(short, long)]
    config: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

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
            error!("E: {}", &e);
        }
    });

    Ok(())
}
