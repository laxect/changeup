use std::{collections::VecDeque, future, sync::Arc};

use dbus::{channel::MatchingReceiver, message::MatchRule};
use dbus_crossroads::Crossroads;
use futures::{lock::Mutex, FutureExt, StreamExt};
use swayipc_async::{Event, EventType, Node, WindowChange};

const SUBS: [swayipc_async::EventType; 1] = [EventType::Window];

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

async fn moniter(last: Arc<Mutex<Option<i64>>>) -> anyhow::Result<()> {
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
                focus(event.container, &mut visited_list)?;
            }
            WindowChange::Close => {
                close(event.container, &mut visited_list)?;
            }
            _ => continue,
        }
        let len = visited_list.len();
        if len >= 2 {
            let mut last = last.lock().await;
            *last = Some(visited_list[len - 2]);
            log::debug!("last: {:?}", *last);
        }
    }
    Ok(())
}

async fn dbus_station(last: Arc<Mutex<Option<i64>>>) -> anyhow::Result<()> {
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

    let ifact_token = road.register(changeup::NAME, |b| {
        b.method_with_cr_async(changeup::METHOD, (), ("node_id",), move |mut ctx, cr, _: ()| {
            let last: &mut Arc<Mutex<Option<i64>>> = cr.data_mut(ctx.path()).unwrap();
            let last = last.clone();
            async move {
                let last = last.lock().await.to_owned();
                let last = last.unwrap_or(-1);
                ctx.reply(Ok((last.to_string(),)))
            }
        });
    });

    road.insert("/", &[ifact_token], last.clone());
    log::info!("station set up");

    conn.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |msg, conn| {
            road.handle_message(msg, conn).unwrap();
            true
        }),
    );

    future::pending::<()>().await;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let last = Arc::new(Mutex::new(None));
    let moniter_last = last.clone();

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let end = futures::select! {
            x = dbus_station(last).fuse() => x,
            y = moniter(moniter_last).fuse() => y,
        };
        if let Err(e) = end {
            log::error!("E: {}", &e);
        }
    });

    Ok(())
}
