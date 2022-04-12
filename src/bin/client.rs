use std::time::Duration;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let conn = dbus::blocking::LocalConnection::new_session()?;
    let proxy = conn.with_proxy(changeup::NAME, "/", Duration::from_millis(200));
    let (last,): (String,) = proxy.method_call(changeup::NAME, changeup::METHOD, ())?;
    let last: i64 = last.parse()?;
    if last == -1 {
        log::warn!("no last viewed window avaliable yet!");
        return Ok(());
    }

    let mut swayc = swayipc::Connection::new()?;
    let cmd = format!("[con_id={}] focus", last);
    swayc.run_command(cmd).map(|_| ()).map_err(|e| {
        log::error!("sway cmd error: {}", &e);
        anyhow::anyhow!(e)
    })
}
