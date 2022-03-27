use std::time::Duration;

use redis::Commands;
use swayipc::{Connection, EventType, WindowChange};

const SUBS: [swayipc::EventType; 1] = [EventType::Window];

fn main() -> anyhow::Result<()> {
    let conn = Connection::new()?;
    let redis = redis::Client::open("redis://127.0.0.1")?;
    let mut redis_con = redis.get_connection()?;
    redis_con.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut last = None;
    for event in conn.subscribe(SUBS)? {
        let event = match event? {
            swayipc::Event::Window(win_event) => *win_event,
            _ => unreachable!(),
        };
        if !matches!(event.change, WindowChange::Focus) {
            continue;
        }
        let node_id = event.container.id;
        log::info!("focus: {}", node_id);
        if let Some(last) = last {
            let _: () = redis_con.set("sway:last_focus", last)?;
        }
        last = Some(node_id);
    }
    Ok(())
}
