pub const LEN: usize = 32;
pub const KEY: &str = "sway:last_focus";

#[cfg(not(debug_assertions))]
pub const NAME: &str = "moe.gyara.changeup";
#[cfg(debug_assertions)]
pub const NAME: &str = "moe.gyara.changeupd";

pub const LAST_VIEWED: &str = "/last_viewed";
pub const LAST_VIEWED_METHOD: &str = "LastViewed";
pub const JUMP_BACK_METHOD: &str = "Jump";
