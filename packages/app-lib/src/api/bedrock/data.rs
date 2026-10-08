use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct Document {
    pub text: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recovery {
    pub id: String,
    pub root_id: String,
    pub path: String,
    pub saved_at: u64,
    pub operation: String,
    #[serde(default = "default_state")]
    pub state: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub size_limited: bool,
    #[serde(default)]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WorldTarget {
    pub root_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RootKind {
    Shared,
    User,
    Legacy,
    Logs,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataRoot {
    pub id: String,
    pub path: std::path::PathBuf,
    pub kind: RootKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    ResourcePack,
    SkinPack,
    BehaviorPack,
    World,
    Log,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceItem {
    pub root_id: String,
    pub path: String,
    pub name: String,
    pub kind: ItemKind,
    pub version: Option<String>,
    pub description: Option<String>,
    pub development: bool,
    pub icon_path: Option<std::path::PathBuf>,
    pub pack_id: Option<String>,
    pub pack_version: Option<Vec<u32>>,
    pub dependencies: Vec<PackDependency>,
    pub activations: Vec<PackActivation>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Workspace {
    pub roots: Vec<DataRoot>,
    pub items: Vec<WorkspaceItem>,
    pub incomplete: bool,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub created: Option<u64>,
    pub modified: Option<u64>,
    pub count: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct DirectoryListing {
    pub entries: Vec<FileEntry>,
    pub limited: bool,
}

#[derive(Debug, Serialize)]
pub struct LogText {
    pub text: String,
    pub truncated: bool,
}

fn default_state() -> String {
    "applied".into()
}
fn default_source() -> String {
    "management".into()
}
#[derive(Debug, Clone, Serialize)]
pub struct PackDependency {
    pub pack_id: String,
    pub version: Vec<u32>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PackActivation {
    pub root_id: String,
    pub world_path: String,
    pub version: Vec<u32>,
}
#[derive(Debug, Serialize)]
pub struct RecoveryPage {
    pub items: Vec<Recovery>,
    pub total: usize,
    pub limited: bool,
}
#[derive(Debug, Serialize)]
pub struct RecoveryPreview {
    pub recovery: Recovery,
    pub can_restore: bool,
    pub conflicts: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct StorageEntry {
    pub id: String,
    pub root_id: String,
    pub path: String,
    pub category: String,
    pub size_bytes: u64,
    pub size_limited: bool,
    pub removable: bool,
}
#[derive(Debug, Serialize)]
pub struct StorageSummary {
    pub entries: Vec<StorageEntry>,
    pub limited: bool,
}
