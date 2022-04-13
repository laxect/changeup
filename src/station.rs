use std::future;

use dbus::{channel::MatchingReceiver, message::MatchRule};
use dbus_crossroads::{Crossroads, IfaceBuilder, IfaceToken};

use crate::{Criteria, Last};

fn last_viewed(road: &mut Crossroads) -> IfaceToken<Last> {
    road.register(crate::NAME, |b: &mut IfaceBuilder<Last>| {
        b.property("useable").get_async(move |mut ctx, change_up| {
            let change_up = change_up.clone();
            async move { ctx.reply(Ok(change_up.lock().await.last.is_some())) }
        });
        b.property("node_id").get_async(move |mut ctx, change_up| {
            let change_up = change_up.clone();
            async move {
                let last = change_up.lock().await.last.to_owned().unwrap_or(-1);
                ctx.reply(Ok(last))
            }
        });
        b.method_with_cr_async(crate::JUMP_BACK_METHOD, (), (), move |mut ctx, cr, _: ()| {
            let change_up: &mut Last = cr.data_mut(ctx.path()).unwrap();
            let change_up = change_up.clone();
            async move {
                let mut change_up = change_up.lock().await;
                if let Some(last) = change_up.last {
                    let conn = &mut change_up.conn;
                    conn.run_command(last.focus()).await.map_err(|e| log::error!("cmd: {}", e)).ok();
                }
                ctx.reply(Ok(()))
            }
        });
    })
}

pub async fn dbus_station(change_up: Last) -> anyhow::Result<()> {
    log::info!("station up");

    let (resource, conn) = dbus_tokio::connection::new_session_sync()?;
    let _handle = tokio::spawn(async {
        let err = resource.await;
        log::error!("Lost connection to D-Bus: {}", err);
    });

    conn.request_name(crate::NAME, true, true, false).await?;

    let mut road = Crossroads::new();
    road.set_async_support(Some((
        conn.clone(),
        Box::new(|x| {
            tokio::spawn(x);
        }),
    )));

    let last_viewed_token = last_viewed(&mut road);
    road.insert(crate::LAST_VIEWED, &[last_viewed_token], change_up);

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
