use std::{borrow::Cow, fs::File, future, io::Read, path::Path};

use color_eyre::eyre::{self, eyre};
use tracing::{error, info};
use zbus::{dbus_interface, fdo, ConnectionBuilder};

use crate::{ChangeUpConfig, ConId, Criteria, Last};

mod map_manager;
mod rule;

pub use map_manager::{Actions, MapManager};
pub use rule::{Rule, RuleSet};

fn load_config<P: AsRef<Path>>(path: P) -> eyre::Result<ChangeUpConfig> {
    let mut file = File::open(path)?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;
    let config = toml::from_str(&buffer)?;
    Ok(config)
}

enum FocusMode {
    JumpBack,
    Focus(i64),
    Exec(Option<String>),
}

struct Station {
    change_up: Last,
}

impl Station {
    async fn fo(&mut self, target: String) -> eyre::Result<()> {
        let key = ConId::Wayland(target);
        let mut change_up = self.change_up.lock().await;
        // the id (i64) of con
        let con = *change_up
            .index
            .get(&key)
            .ok_or_else(|| eyre!("node not found"))?
            .iter()
            .next()
            .ok_or_else(|| eyre!("node not found"))?;
        change_up.conn.run_command(con.focus()).await?;
        Ok(())
    }

    async fn rule_fo(&mut self, app_kind: String) -> eyre::Result<()> {
        let mut change_up = self.change_up.lock().await;

        let now_on = change_up.now_on().await?;
        let rule = change_up.ruleset.get(&app_kind).ok_or_else(|| eyre!("No such rule set"))?;

        // if find nothing, then exec
        let mut mode = FocusMode::Exec(rule.exec());
        for link in rule.links().iter() {
            let con_id = ConId::Wayland(link.to_owned());
            if let Some(set) = change_up.index.get(&con_id) {
                if let Some(now) = now_on {
                    // if already focus one, then jump back
                    if set.contains(&now) {
                        mode = FocusMode::JumpBack;
                        break;
                    }
                    // if already one and not focus, then focus it
                    if let Some(target) = set.iter().next() {
                        mode = FocusMode::Focus(*target);
                        break;
                    }
                }
            }
        }

        match mode {
            FocusMode::JumpBack => {
                if let Some(last) = change_up.last {
                    change_up.conn.run_command(last.focus()).await.map_err(|e| error!("cmd: {}", e)).ok();
                }
            }
            FocusMode::Focus(con) => {
                change_up.conn.run_command(con.focus()).await.map_err(|e| error!("cmd: {}", e)).ok();
            }
            FocusMode::Exec(Some(cmd)) => {
                change_up.conn.run_command(cmd).await.map_err(|e| error!("exec: {}", &e)).ok();
            }
            FocusMode::Exec(None) => {
                info!("no exec seted");
            }
        }
        Ok(())
    }
}

#[dbus_interface(name = "moe.gyara.changeup")]
impl Station {
    fn version(&self) -> Cow<str> {
        env!("CARGO_PKG_VERSION").into()
    }

    async fn ping(&self) -> Cow<str> {
        let change_up = self.change_up.lock().await;
        change_up.map_manager.reload_actions();
        "pong".into()
    }

    #[dbus_interface(property, name = "Ruleset")]
    async fn ruleset(&self) -> String {
        let ruleset = &self.change_up.lock().await.ruleset;
        toml::to_string_pretty(ruleset).unwrap()
    }

    async fn actions(&self) -> String {
        self.change_up.lock().await.map_manager.to_toml()
    }

    async fn reload_config(&mut self, path: &str) -> fdo::Result<Cow<str>> {
        let config = load_config(path).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut change_up = self.change_up.lock().await;
        change_up.ruleset = config.ruleset;
        let new_as = config.actions;
        change_up.map_manager.replace_actions(new_as);
        Ok("done".into())
    }

    #[dbus_interface(property, name = "LastViewedExist")]
    async fn last_viewed_exist(&self) -> bool {
        let change_up = self.change_up.lock().await;
        change_up.last.is_some()
    }

    #[dbus_interface(property, name = "LastViewed")]
    async fn last_viewed(&self) -> i64 {
        self.change_up.lock().await.last.unwrap_or(-1)
    }

    async fn jump_to_last_viewed(&mut self) {
        let mut change_up = self.change_up.lock().await;
        if let Some(last) = change_up.last {
            change_up.conn.run_command(last.focus()).await.map_err(|e| error!("cmd: {}", e)).ok();
        } else {
            info!("no last yet");
        }
    }

    async fn focus(&mut self, target: String) -> fdo::Result<()> {
        self.fo(target).await.map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn rule_focus(&mut self, app_kind: String) -> fdo::Result<()> {
        self.rule_fo(app_kind).await.map_err(|e| fdo::Error::Failed(e.to_string()))
    }
}

// dbus station
pub async fn station(change_up: Last) -> eyre::Result<()> {
    info!("station up");
    let station = Station { change_up };

    let _handler = ConnectionBuilder::session()?
        .name("moe.gyara.changeup")?
        .serve_at("/", station)?
        .build()
        .await?;

    info!("station set up");
    future::pending::<()>().await;
    Ok(())
}
