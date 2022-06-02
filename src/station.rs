use std::{fs::File, future, io::Read, path::Path};

use anyhow::anyhow;
use dbus::{channel::MatchingReceiver, message::MatchRule, MethodErr};
use dbus_crossroads::{Crossroads, IfaceBuilder};

use crate::{ChangeUpConfig, ConId, Criteria, Last};

mod map_manager;
mod rule;

pub use map_manager::{Actions, MapManager};
pub use rule::{Rule, RuleSet};

fn basic(b: &mut IfaceBuilder<Last>) {
    b.property("version").get(|_, _| {
        let version = std::env!("CARGO_PKG_VERSION");
        Ok(version.to_owned())
    });
    b.method("Ping", (), ("pong",), |_, _, _: ()| Ok(("pong".to_owned(),)));
}

fn load_config<P: AsRef<Path>>(path: P) -> anyhow::Result<ChangeUpConfig> {
    let mut file = File::open(path)?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;
    let config = toml::from_str(&buffer)?;
    Ok(config)
}

fn config(b: &mut IfaceBuilder<Last>) {
    b.property("ruleset").get_async(move |mut ctx, change_up| {
        let change_up = change_up.clone();
        async move {
            let ruleset = &change_up.lock().await.ruleset;
            let reply = toml::to_string_pretty(ruleset).unwrap();
            ctx.reply(Ok(reply))
        }
    });
    b.property("actions").get_async(move |mut ctx, change_up| {
        let change_up = change_up.clone();
        async move {
            let reply = change_up.lock().await.map_manager.to_toml();
            ctx.reply(Ok(reply))
        }
    });
    b.method_with_cr_async(crate::LOAD_CONFIG_METHOD, ("path",), (), move |mut ctx, cr, (path,): (String,)| {
        let config = load_config(path);
        let change_up: &Last = cr.data_mut(ctx.path()).unwrap();
        let change_up = change_up.clone();
        async move {
            match config {
                Ok(c) => {
                    let mut change_up = change_up.lock().await;
                    change_up.ruleset = c.ruleset;
                    let new_as = c.actions;
                    change_up.map_manager.replace_actions(new_as);
                    ctx.reply(Ok(()))
                }
                Err(e) => ctx.reply(Err(MethodErr::failed(&e))),
            }
        }
    });
}

fn last_viewed(b: &mut IfaceBuilder<Last>) {
    b.property("last_viewed_exist").get_async(move |mut ctx, change_up| {
        let change_up = change_up.clone();
        async move { ctx.reply(Ok(change_up.lock().await.last.is_some())) }
    });
    b.property("last_viewed").get_async(move |mut ctx, change_up| {
        let change_up = change_up.clone();
        async move {
            let last = change_up.lock().await.last.to_owned().unwrap_or(-1);
            ctx.reply(Ok(last))
        }
    });
    b.method_with_cr_async(crate::JUMP_BACK_METHOD, (), (), move |mut ctx, cr, _: ()| {
        let change_up: &Last = cr.data_mut(ctx.path()).unwrap();
        let change_up = change_up.clone();
        async move {
            let mut change_up = change_up.lock().await;
            if let Some(last) = change_up.last {
                change_up.conn.run_command(last.focus()).await.map_err(|e| log::error!("cmd: {}", e)).ok();
            }
            ctx.reply(Ok(()))
        }
    });
}

enum FocusMode {
    JumpBack,
    Focus(i64),
    Exec(Option<String>),
}

fn focus(b: &mut IfaceBuilder<Last>) {
    async fn fo(change_up: Last, target: String) -> anyhow::Result<()> {
        let key = ConId::Wayland(target);
        let mut change_up = change_up.lock().await;
        // the id (i64) of con
        let con = *change_up
            .index
            .get(&key)
            .ok_or_else(|| anyhow!("node not found"))?
            .iter()
            .next()
            .ok_or_else(|| anyhow!("node not found"))?;
        change_up.conn.run_command(con.focus()).await?;
        Ok(())
    }

    b.method_with_cr_async(crate::FOCUS_METHOD, ("target",), (), move |mut ctx, cr, (target,): (String,)| {
        let change_up: &Last = cr.data_mut(ctx.path()).unwrap();
        let change_up = change_up.clone();
        async move {
            let res = fo(change_up, target).await.map_err(|e| MethodErr::failed(&e));
            ctx.reply(res)
        }
    });
}

fn rule_focus(b: &mut IfaceBuilder<Last>) {
    async fn logic(change_up: Last, app_kind: String) -> anyhow::Result<()> {
        let mut change_up = change_up.lock().await;
        let now_on = change_up.now_on().await?;

        let rule = change_up.ruleset.get(&app_kind).ok_or_else(|| anyhow!("No such rule set"))?;
        let mut links = rule.links().iter();

        let mode = loop {
            // found nothing, exec it
            let link = if let Some(n) = links.next() {
                n
            } else {
                break FocusMode::Exec(rule.exec());
            };
            // the type of link is not important
            let con_id = ConId::Wayland(link.to_owned());
            let set = if let Some(set) = change_up.index.get(&con_id) {
                set
            } else {
                continue;
            };
            // already focus one, jump back
            if let Some(now) = now_on {
                if set.contains(&now) {
                    break FocusMode::JumpBack;
                }
            }
            // have one, and not focused
            if !set.is_empty() {
                let target = set.iter().next().ok_or_else(|| anyhow!("nothing in a box which is not empty"))?;
                break FocusMode::Focus(*target);
            }
        };

        match mode {
            FocusMode::JumpBack => {
                if let Some(last) = change_up.last {
                    change_up.conn.run_command(last.focus()).await.map_err(|e| log::error!("cmd: {}", e)).ok();
                }
            }
            FocusMode::Focus(con) => {
                change_up.conn.run_command(con.focus()).await.map_err(|e| log::error!("cmd: {}", e)).ok();
            }
            FocusMode::Exec(Some(cmd)) => {
                change_up.conn.run_command(cmd).await.map_err(|e| log::error!("exec: {}", &e)).ok();
            }
            FocusMode::Exec(None) => {
                log::info!("no exec seted");
            }
        }
        Ok(())
    }

    b.method_with_cr_async(
        crate::FOCUS_CREATE_OR_JUMPBACK_METHOD,
        ("app_kind",),
        (),
        move |mut ctx, cr, (app_kind,): (String,)| {
            let change_up: &Last = cr.data_mut(ctx.path()).unwrap();
            let change_up = change_up.clone();
            async move {
                let reply = logic(change_up, app_kind).await.map_err(|e| MethodErr::failed(&e));
                ctx.reply(reply)
            }
        },
    );
}

// dbus station
pub async fn station(change_up: Last) -> anyhow::Result<()> {
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

    let token = road.register(crate::NAME, |b: &mut IfaceBuilder<Last>| {
        basic(b);
        focus(b);
        config(b);
        rule_focus(b);
        last_viewed(b);
    });
    road.insert(crate::PATH, &[token], change_up);

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
