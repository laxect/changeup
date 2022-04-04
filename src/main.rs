#![feature(drain_filter)]
use std::{collections::LinkedList, time::Duration};

use changeup::LEN;
use redis::Commands;
use swayipc::{Connection, EventType, Node, WindowChange};

const SUBS: [swayipc::EventType; 1] = [EventType::Window];

fn focus(eve: Node, last: &mut LinkedList<i64>) -> anyhow::Result<()> {
    let node_id = eve.id;
    last.push_back(node_id);
    if last.len() > LEN {
        last.pop_front();
    }
    Ok(())
}

fn close(eve: Node, last: &mut LinkedList<i64>) -> anyhow::Result<()> {
    let node_id = eve.id;
    if !last.contains(&node_id) {
        return Ok(());
    }
    let items = last.drain_filter(|id| *id == node_id);
    log::info!("remove nodes: {:?}", items);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let conn = Connection::new()?;
    let redis = redis::Client::open("redis://127.0.0.1")?;
    let mut redis_con = redis.get_connection()?;
    redis_con.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut last = LinkedList::new();
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
        if let Some(last) = last.front() {
            let _: () = redis_con.set(changeup::KEY, last)?;
        }
    }
    Ok(())
}
