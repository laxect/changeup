use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};

use futures::lock::Mutex;
use swayipc_async::{Connection, Node};

pub const LEN: usize = 32;
pub const KEY: &str = "sway:last_focus";

mod moniter;
mod station;

pub use moniter::moniter;
pub use station::dbus_station;

#[cfg(not(debug_assertions))]
pub const NAME: &str = "moe.gyara.changeup";
#[cfg(debug_assertions)]
pub const NAME: &str = "moe.gyara.changeupd";

pub const LAST_VIEWED: &str = "/last_viewed";
pub const JUMP_BACK_METHOD: &str = "Jump";

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

pub struct ChangeUp {
    pub last: Option<i64>,
    pub now_on: Option<i64>,
    pub index: HashMap<ConId, HashSet<i64>>,
    pub conn: Connection,
}

impl ChangeUp {
    pub async fn last() -> anyhow::Result<Last> {
        let change_up = Self {
            last: None,
            now_on: None,
            index: HashMap::default(),
            conn: Connection::new().await?,
        };
        Ok(Arc::new(Mutex::new(change_up)))
    }
}

pub type Last = Arc<Mutex<ChangeUp>>;
