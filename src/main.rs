use futures::FutureExt;

use changeup::{dbus_station, moniter, ChangeUp};

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let change_up = ChangeUp::last().await.unwrap();

        let end = futures::select! {
            x = dbus_station(change_up.clone()).fuse() => x,
            y = moniter(change_up).fuse() => y,
        };
        if let Err(e) = end {
            log::error!("E: {}", &e);
        }
    });

    Ok(())
}
