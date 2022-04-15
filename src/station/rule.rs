use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// # Rule
/// app_kind ("browser") => app_list ["firefox", "google-chrome", "tor"]
/// Then if any exist (check in order) just jump to.

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Rule {
    link: Vec<String>,
    exec: Option<String>,
}

pub type RuleSet = HashMap<String, Rule>;

impl Rule {
    pub fn links(&self) -> &[String] {
        &self.link
    }

    pub fn exec(&self) -> Option<String> {
        self.exec.as_ref().map(|exec| format!("exec {exec}"))
    }
}
