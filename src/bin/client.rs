use std::time::Duration;

struct Opts {}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let conn = dbus::blocking::LocalConnection::new_session()?;
    let proxy = conn.with_proxy(changeup::NAME, changeup::LAST_VIEWED, Duration::from_millis(200));
    proxy.method_call(changeup::NAME, changeup::JUMP_BACK_METHOD, ())?;
    Ok(())
}
