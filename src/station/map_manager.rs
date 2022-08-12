use serde::{Deserialize, Serialize};
use swayipc_async::Connection;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyC {
    key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Keymap {
    Last {
        #[serde(flatten)]
        key: KeyC,
    },
    RuleFocus {
        #[serde(flatten)]
        key: KeyC,
        target: String,
    },
}

impl Keymap {
    fn key(&self) -> &str {
        match self {
            Self::Last { key } => &key.key,
            Self::RuleFocus { key, .. } => &key.key,
        }
    }

    fn load(&self) -> String {
        let key = self.key();
        let then = match self {
            Self::Last { .. } => "exec changeup-client last".to_owned(),
            Self::RuleFocus { target, .. } => format!("exec changeup-client rule-focus {}", target),
        };
        format!("bindsym {} {}", key, then)
    }

    fn unload(&self) -> String {
        let key = self.key();
        format!("unbindsym {}", key)
    }
}

pub type Actions = Vec<Keymap>;

#[derive(Default)]
pub struct MapManager {
    inner: Actions,
}

impl MapManager {
    async fn unload(inner: &Actions, conn: &mut Connection) -> anyhow::Result<()> {
        for val in inner {
            let cmd = val.unload();
            conn.run_command(cmd).await?;
        }
        Ok(())
    }

    async fn load(inner: &Actions, conn: &mut Connection) -> anyhow::Result<()> {
        for val in inner {
            let cmd = val.load();
            conn.run_command(cmd).await?;
        }
        Ok(())
    }

    async fn replace(old_as: Actions, new_as: Actions) -> anyhow::Result<()> {
        let mut conn = Connection::new().await?;
        Self::unload(&old_as, &mut conn).await?;
        Self::load(&new_as, &mut conn).await?;
        Ok(())
    }

    pub fn replace_actions(&mut self, actions: Actions) {
        let old_as = std::mem::replace(&mut self.inner, actions.clone());
        tokio::spawn(async move {
            if let Err(e) = MapManager::replace(old_as, actions).await {
                log::error!("reload failed: {}", e)
            }
        });
    }

    async fn reload(actions: Actions) -> anyhow::Result<()> {
        let mut conn = Connection::new().await?;
        Self::load(&actions, &mut conn).await?;
        Ok(())
    }

    pub fn reload_actions(&self) {
        let actions = self.inner.clone();
        tokio::spawn(async move {
            if let Err(e) = MapManager::reload(actions).await {
                log::error!("reload failed: {}", e)
            }
        });
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(&self.inner).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::Keymap;

    #[test]
    fn der() -> anyhow::Result<()> {
        let input = r#"
            type = "Last"
            key = "Alt+C"
        "#;
        let out: Keymap = toml::from_str(input)?;
        assert_eq!(out.key(), "Alt+C");
        Ok(())
    }
}
