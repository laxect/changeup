use std::collections::VecDeque;

use futures::StreamExt;
use swayipc_async::{Connection, Event, EventType, WindowChange};

use crate::{ConId, Last, Node};

const SUBS: [swayipc_async::EventType; 1] = [EventType::Window];

#[inline]
async fn focus(eve: Node, _change_up: &Last, visited_list: &mut VecDeque<i64>) -> anyhow::Result<()> {
    let node_id = eve.id;
    visited_list.retain(|id| *id != node_id);
    visited_list.push_back(node_id);
    if visited_list.len() > crate::LEN {
        visited_list.pop_front();
    }
    Ok(())
}

#[inline]
async fn close(eve: Node, change_up: &Last, visited_list: &mut VecDeque<i64>) -> anyhow::Result<()> {
    let con_id = ConId::take_from_node(&eve);
    let node_id = eve.id;
    if !visited_list.contains(&node_id) {
        return Ok(());
    }
    visited_list.retain(|id| *id != node_id);
    let mut change_up = change_up.lock().await;
    let mut remove_con = false;
    if let Some(wins) = change_up.index.get_mut(&con_id) {
        wins.remove(&node_id);
        if wins.is_empty() {
            remove_con = true;
        }
    }
    if remove_con {
        change_up.index.remove(&con_id);
    }
    Ok(())
}

#[inline]
async fn new(eve: Node, change_up: &Last) -> anyhow::Result<()> {
    let con_id = ConId::take_from_node(&eve);
    let node_id = eve.id;
    let mut change_up = change_up.lock().await;
    let entry = change_up.index.entry(con_id).or_default();
    entry.insert(node_id);
    Ok(())
}

async fn scan(conn: &mut Connection, change_up: &Last) -> anyhow::Result<()> {
    use swayipc_async::NodeType;

    let tree = conn.get_tree().await?;
    let index = &mut change_up.lock().await.index;
    let mut next = VecDeque::new();
    next.push_back(tree);
    while let Some(node) = next.pop_front() {
        if matches!(node.node_type, NodeType::Con | NodeType::FloatingCon) && node.nodes.is_empty() {
            // only use valid one
            let con_id = ConId::take_from_node(&node);
            let node_id = node.id;
            index.entry(con_id).or_default().insert(node_id);
        }
        for item in node.nodes.into_iter() {
            next.push_back(item);
        }
    }
    Ok(())
}

pub async fn moniter(change_up: Last) -> anyhow::Result<()> {
    log::info!("moniter up");

    let mut conn = Connection::new().await?;
    scan(&mut conn, &change_up).await?;

    let mut visited_list = VecDeque::with_capacity(crate::LEN);
    let mut stream = conn.subscribe(&SUBS).await?;
    while let Some(event) = stream.next().await {
        let event = match event? {
            Event::Window(win_event) => *win_event,
            _ => unreachable!(),
        };
        match event.change {
            WindowChange::Focus => focus(event.container, &change_up, &mut visited_list).await?,
            WindowChange::Close => close(event.container, &change_up, &mut visited_list).await?,
            WindowChange::New => new(event.container, &change_up).await?,
            _ => continue,
        };
        let len = visited_list.len();
        let mut change_up = change_up.lock().await;
        if len >= 2 {
            change_up.last = Some(visited_list[len - 2]);
        } else {
            change_up.last = None;
        }
        log::debug!("last: {:?}", change_up.last);
        log::debug!("index: {:?}", change_up.index);
        log::debug!("ruleset: {:?}", change_up.ruleset);
    }
    Ok(())
}
