use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize)]
pub struct WorkspaceItem {
    pub root_id: String,
    pub path: String,
    pub name: String,
    pub kind: ItemKind,
    pub version: Option<String>,
    pub description: Option<String>,
    pub development: bool,
    pub icon_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Serialize)]
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
