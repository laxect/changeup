use std::{
    collections::{HashMap, HashSet},
    fs::File,
    hash::Hash,
    io::Read,
    path::Path,
    sync::Arc,
};

use futures::lock::Mutex;
use serde::Deserialize;
use swayipc_async::{Connection, Node, NodeType};

pub const LEN: usize = 32;
pub const KEY: &str = "sway:last_focus";

mod moniter;
mod station;

pub use moniter::moniter;
pub use station::station;
use station::{Actions, MapManager, RuleSet};

#[cfg(not(debug_assertions))]
pub const NAME: &str = "moe.gyara.changeup";
#[cfg(debug_assertions)]
pub const NAME: &str = "moe.gyara.changeupd";

pub const PATH: &str = "/";
pub const FOCUS_METHOD: &str = "Focus";
pub const JUMP_BACK_METHOD: &str = "JumpBack";
pub const LOAD_CONFIG_METHOD: &str = "LoadConfig";
pub const FOCUS_CREATE_OR_JUMPBACK_METHOD: &str = "FCoJ";

#[derive(Clone, Debug)]
pub enum ConId {
    Wayland(String),
    X(String),
}

impl ConId {
    pub fn take_from_node(node: &Node) -> Self {
        if let Some(app_id) = &node.app_id {
            return Self::Wayland(app_id.to_owned());
        }
        if let Some(win) = node.window_properties.as_ref() {
            let class = win.class.as_ref();
            class.map_or_else(|| Self::X("".to_owned()), |s| Self::X(s.to_owned()))
        } else {
            Self::X(node.name.clone().unwrap_or_else(|| node.id.to_string()))
        }
    }

    pub fn id(&self) -> &String {
        match self {
            Self::Wayland(app_id) => app_id,
            Self::X(class) => class,
        }
    }
}

impl PartialEq for ConId {
    fn eq(&self, other: &Self) -> bool {
        self.id().eq(other.id())
    }
}

impl Eq for ConId {}

impl Hash for ConId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}

pub trait Criteria {
    fn criteria(&self) -> String;

    fn focus(&self) -> String {
        format!("{} focus", self.criteria())
    }
}

impl Criteria for i64 {
    #[inline]
    fn criteria(&self) -> String {
        format!("[con_id={self}]")
    }
}

#[derive(Deserialize)]
pub struct ChangeUpConfig {
    pub ruleset: RuleSet,
    pub actions: Actions,
}

pub struct ChangeUp {
    pub last: Option<i64>,
    pub index: HashMap<ConId, HashSet<i64>>,
    pub ruleset: RuleSet,
    pub map_manager: MapManager,
    pub conn: Connection,
}

const DEFAULT_CONFIG: &str = "changeup/config.toml";

impl ChangeUp {
    pub async fn last() -> anyhow::Result<Last> {
        let mut config = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("?"))?;
        config.push(DEFAULT_CONFIG);
        Self::last_with_config(config).await
    }

    pub async fn last_with_config<P: AsRef<Path>>(config_path: P) -> anyhow::Result<Last> {
        let mut config_file = File::open(config_path)?;
        let mut config = String::new();
        config_file.read_to_string(&mut config)?;
        let ChangeUpConfig { ruleset, actions } = toml::from_str(&config)?;
        let mut change_up = Self {
            last: None,
            index: HashMap::default(),
            conn: Connection::new().await?,
            map_manager: MapManager::default(),
            ruleset,
        };
        change_up.map_manager.replace_actions(actions);
        Ok(Arc::new(Mutex::new(change_up)))
    }

    async fn now_on(&mut self) -> anyhow::Result<Option<i64>> {
        let mut list = vec![self.conn.get_tree().await?];
        while let Some(mut node) = list.pop() {
            if matches!(node.node_type, NodeType::Con | NodeType::FloatingCon) && node.nodes.is_empty() && node.focused {
                return Ok(Some(node.id));
            }
            list.append(&mut node.nodes);
        }
        Ok(None)
    }
}

pub type Last = Arc<Mutex<ChangeUp>>;
