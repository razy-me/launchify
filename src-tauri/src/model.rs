use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const INDEX_SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    App,
    StoreApp,
    SystemSetting,
    PowerAction,
    Folder,
    File,
    Workflow,
    WebShortcut,
    Window,
    InlineResult,
    ShellCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub keywords: Vec<String>,
    pub kind: ItemKind,
    pub target: String,
    pub icon_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexSnapshot {
    pub version: u32,
    pub built_at_unix: i64,
    pub items: Vec<IndexItem>,
}

impl Default for IndexSnapshot {
    fn default() -> Self {
        Self {
            version: INDEX_SNAPSHOT_VERSION,
            built_at_unix: 0,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrecencyEntry {
    pub count: u32,
    pub last_used_unix: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrecencyStore {
    pub entries: HashMap<String, FrecencyEntry>,
    /// Normalized prefix -> item_id -> selection count
    pub prefix_memory: HashMap<String, HashMap<String, u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrefixMode {
    Global,
    Shell,
    Calc,
    Path,
    Window,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlinePreview {
    pub display: String,
    pub copy_value: String,
    pub kind_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: ItemKind,
    pub icon_path: Option<String>,
    pub secondary_hint: Option<String>,
    pub score: f64,
    pub autocomplete_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResponse {
    pub mode: PrefixMode,
    pub inline_preview: Option<InlinePreview>,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ExecuteOutcome {
    Launched,
    CopiedToClipboard { value: String },
    WindowFocused,
}
