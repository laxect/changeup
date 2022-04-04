use std::{collections::VecDeque, time::Duration};

use redis::Commands;
use swayipc::{Connection, EventType, Node, WindowChange};

const SUBS: [swayipc::EventType; 1] = [EventType::Window];

fn focus(eve: Node, last: &mut VecDeque<i64>) -> anyhow::Result<()> {
    let node_id = eve.id;
    let _ = last.retain(|id| *id != node_id);
    last.push_back(node_id);
    if last.len() > changeup::LEN {
        last.pop_front();
    }
    Ok(())
}

fn close(eve: Node, last: &mut VecDeque<i64>) -> anyhow::Result<()> {
    let node_id = eve.id;
    if !last.contains(&node_id) {
        return Ok(());
    }
    let _ = last.retain(|id| *id != node_id);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let conn = Connection::new()?;
    let redis = redis::Client::open("redis://127.0.0.1")?;
    let mut redis_con = redis.get_connection()?;
    redis_con.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut last = VecDeque::with_capacity(changeup::LEN);
    for event in conn.subscribe(SUBS)? {
        let event = match event? {
            swayipc::Event::Window(win_event) => *win_event,
            _ => unreachable!(),
        };
        match event.change {
            WindowChange::Focus => {
                focus(event.container, &mut last)?;
            }
            WindowChange::Close => {
                close(event.container, &mut last)?;
            }
            _ => continue,
        }
        dbg!(&last);
        let len = last.len();
        if len > 2 {
            let last = last[len - 2];
            let _: () = redis_con.set(changeup::KEY, last)?;
        }
    }
    Ok(())
}
