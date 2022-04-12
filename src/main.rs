use std::{
    collections::{HashMap, VecDeque},
    future,
    sync::Arc,
};

use dbus::{channel::MatchingReceiver, message::MatchRule};
use dbus_crossroads::{Crossroads, IfaceBuilder, IfaceToken};
use futures::{lock::Mutex, FutureExt, StreamExt};
use swayipc_async::{Event, EventType, Node, WindowChange};

const SUBS: [swayipc_async::EventType; 1] = [EventType::Window];

#[derive(Hash, Clone, Debug, PartialEq, Eq)]
pub enum ConId {
    Wayland(String),
    X11(String),
}

impl ConId {
    #[inline]
    fn criteria(&self) -> String {
        match self {
            Self::Wayland(id) => format!("[app_id={}]", id),
            Self::X11(class) => format!(r#"[class="{}"]"#, class),
        }
    }

    fn take_from_node(node: &Node) -> Option<Self> {
        if let Some(app_id) = &node.app_id {
            return Some(Self::Wayland(app_id.to_owned()));
        }
        let class = node.window_properties.as_ref()?.class.as_ref()?;
        Some(Self::X11(class.to_owned()))
    }
}

#[inline]
fn criteria(node_id: i64) -> String {
    format!("[con_id={}]", node_id)
}

pub struct ChangeUp {
    last: Option<i64>,
    now_on: Option<i64>,
    index: HashMap<ConId, Vec<i64>>,
    conn: swayipc_async::Connection,
}

type Last = Arc<Mutex<ChangeUp>>;

async fn focus(eve: Node, _change_up: &Last, visited_list: &mut VecDeque<i64>) -> anyhow::Result<()> {
    let node_id = eve.id;
    let _ = visited_list.retain(|id| *id != node_id);
    visited_list.push_back(node_id);
    if visited_list.len() > changeup::LEN {
        visited_list.pop_front();
    }
    Ok(())
}

async fn close(eve: Node, change_up: &Last, visited_list: &mut VecDeque<i64>) -> anyhow::Result<()> {
    let con_id = ConId::take_from_node(&eve).ok_or_else(|| anyhow::anyhow!("parser con id failed"))?;
    let node_id = eve.id;
    if !visited_list.contains(&node_id) {
        return Ok(());
    }
    let _ = visited_list.retain(|id| *id != node_id);
    let mut change_up = change_up.lock().await;
    if let Some(wins) = change_up.index.get_mut(&con_id) {
        wins.retain(|node| *node != node_id);
    }
    Ok(())
}

async fn moniter(change_up: Last) -> anyhow::Result<()> {
    log::info!("moniter up");

    let conn = swayipc_async::Connection::new().await?;
    let mut visited_list = VecDeque::with_capacity(changeup::LEN);
    let mut stream = conn.subscribe(&SUBS).await?;
    while let Some(event) = stream.next().await {
        let event = match event? {
            Event::Window(win_event) => *win_event,
            _ => unreachable!(),
        };
        match event.change {
            WindowChange::Focus => {
                focus(event.container, &change_up, &mut visited_list).await?;
            }
            WindowChange::Close => {
                close(event.container, &change_up, &mut visited_list).await?;
            }
            _ => continue,
        }
        let len = visited_list.len();
        let mut change_up = change_up.lock().await;
        if len >= 2 {
            change_up.last = Some(visited_list[len - 2]);
        } else {
            change_up.last = None;
        }
        if len >= 1 {
            change_up.now_on = Some(visited_list[len - 1]);
        } else {
            change_up.now_on = None;
        }
        log::debug!("last: {:?}", change_up.last);
        log::debug!("on: {:?}", change_up.now_on);
    }
    Ok(())
}

fn last_viewed(road: &mut Crossroads) -> IfaceToken<Last> {
    road.register(changeup::NAME, |b: &mut IfaceBuilder<Last>| {
        b.property("online").get_async(move |mut ctx, change_up| {
            let change_up = change_up.clone();
            async move { ctx.reply(Ok(change_up.lock().await.last.is_some())) }
        });
        b.method_with_cr_async(
            changeup::LAST_VIEWED_METHOD,
            (),
            ("node_id",),
            move |mut ctx, cr, _: ()| {
                let change_up: &mut Last = cr.data_mut(ctx.path()).unwrap();
                let change_up = change_up.clone();
                async move {
                    let last = change_up.lock().await.last.to_owned().unwrap_or(-1);
                    ctx.reply(Ok((last.to_string(),)))
                }
            },
        );
        b.method_with_cr_async(changeup::JUMP_BACK_METHOD, (), (), move |mut ctx, cr, _: ()| {
            let change_up: &mut Last = cr.data_mut(ctx.path()).unwrap();
            let change_up = change_up.clone();
            async move {
                let mut change_up = change_up.lock().await;
                if let Some(last) = change_up.last {
                    let cmd = format!("{} focus", criteria(last));
                    change_up
                        .conn
                        .run_command(cmd)
                        .await
                        .map_err(|e| {
                            log::error!("cmd: {}", &e);
                            e
                        })
                        .ok();
                }
                ctx.reply(Ok(()))
            }
        });
    })
}

async fn dbus_station(change_up: Last) -> anyhow::Result<()> {
    log::info!("station up");

    let (resource, conn) = dbus_tokio::connection::new_session_sync()?;
    let _handle = tokio::spawn(async {
        let err = resource.await;
        log::error!("Lost connection to D-Bus: {}", err);
    });

    conn.request_name(changeup::NAME, true, true, false).await?;

    let mut road = Crossroads::new();
    road.set_async_support(Some((
        conn.clone(),
        Box::new(|x| {
            tokio::spawn(x);
        }),
    )));

    let last_viewed_token = last_viewed(&mut road);
    road.insert(changeup::LAST_VIEWED, &[last_viewed_token], change_up);

    conn.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |msg, conn| {
            road.handle_message(msg, conn).unwrap();
            true
        }),
    );

    log::info!("station set up");
    future::pending::<()>().await;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let change_up = ChangeUp {
            last: None,
            now_on: None,
            conn: swayipc_async::Connection::new().await.unwrap(),
            index: HashMap::new(),
        };
        let change_up = Arc::new(Mutex::new(change_up));

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
