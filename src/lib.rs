use std::{
    collections::{HashMap, HashSet},
    fs::File,
    hash::Hash,
    io::Read,
    path::Path,
    sync::Arc,
};

use futures::lock::Mutex;
use serde::{Deserialize, Serialize};
use swayipc_async::{Connection, Node, NodeType};

pub const LEN: usize = 32;
pub const KEY: &str = "sway:last_focus";

mod moniter;
mod station;

pub use moniter::moniter;
pub use station::station;
use station::RuleSet;

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
    X11(String),
}

impl ConId {
    pub fn take_from_node(node: &Node) -> Option<Self> {
        if let Some(app_id) = &node.app_id {
            return Some(Self::Wayland(app_id.to_owned()));
        }
        let class = node.window_properties.as_ref()?.class.as_ref()?;
        Some(Self::X11(class.to_owned()))
    }

    pub fn id(&self) -> &String {
        match self {
            Self::Wayland(app_id) => app_id,
            Self::X11(class) => class,
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

impl Criteria for ConId {
    #[inline]
    fn criteria(&self) -> String {
        match self {
            Self::Wayland(id) => format!("[app_id={}]", id),
            Self::X11(class) => format!(r#"[class="{}"]"#, class),
        }
    }
}

impl Criteria for i64 {
    #[inline]
    fn criteria(&self) -> String {
        format!("[con_id={}]", self)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChangeUpConfig {
    pub ruleset: RuleSet,
}

pub struct ChangeUp {
    pub last: Option<i64>,
    pub index: HashMap<ConId, HashSet<i64>>,
    pub ruleset: RuleSet,
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
        let ChangeUpConfig { ruleset } = toml::from_str(&config)?;
        let change_up = Self {
            last: None,
            index: HashMap::default(),
            conn: Connection::new().await?,
            ruleset,
        };
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
