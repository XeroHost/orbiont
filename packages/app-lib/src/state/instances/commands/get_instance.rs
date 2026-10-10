use crate::state::instances::{
    ContentSet, Instance, InstanceIconConfig, InstanceLaunchOverrides,
    InstanceLink, InstanceSyncedOptions, adapters::sqlite::instance_rows,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceMetadata {
    pub instance: Instance,
    #[serde(default)]
    pub icon_config: Option<InstanceIconConfig>,
    pub applied_content_set: ContentSet,
    pub link: InstanceLink,
    #[serde(default)]
    pub quarantined: bool,
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub synced_options: InstanceSyncedOptions,
    pub launch_overrides: InstanceLaunchOverrides,
}

pub(crate) async fn get_instance(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceMetadata>> {
    get_instance_metadata(instance_id, pool).await
}

pub(crate) async fn get_instance_metadata(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceMetadata>> {
    let Some(record) =
        instance_rows::get_instance_metadata_by_id(instance_id, pool).await?
    else {
        return Ok(None);
    };
    let quarantined =
        instance_rows::is_instance_quarantined(instance_id, pool).await?;

    Ok(Some(instance_metadata(record, quarantined)))
}

pub(crate) async fn get_instances_metadata(
    instance_ids: &[&str],
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceMetadata>> {
    let records =
        instance_rows::get_instance_metadata_many(instance_ids, pool).await?;
    let quarantined_ids =
        instance_rows::get_quarantined_instance_ids(pool).await?;

    Ok(records
        .into_iter()
        .map(|record| {
            let quarantined = quarantined_ids.contains(&record.instance.id);
            instance_metadata(record, quarantined)
        })
        .collect())
}

pub(crate) async fn list_instances(
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceMetadata>> {
    let records = instance_rows::list_instance_metadata(pool).await?;
    let quarantined_ids =
        instance_rows::get_quarantined_instance_ids(pool).await?;

    Ok(records
        .into_iter()
        .map(|record| {
            let quarantined = quarantined_ids.contains(&record.instance.id);
            instance_metadata(record, quarantined)
        })
        .collect())
}

fn instance_metadata(
    record: instance_rows::InstanceMetadataRecord,
    quarantined: bool,
) -> InstanceMetadata {
    InstanceMetadata {
        instance: record.instance,
        icon_config: record.icon_config,
        applied_content_set: record.applied_content_set,
        link: record.link,
        quarantined,
        group_ids: record.group_ids,
        synced_options: record.synced_options,
        launch_overrides: record.launch_overrides,
    }
}

#[cfg(test)]
mod upstream_legacy_tests {
    use super::*;

    #[test]
    fn recovery_metadata_without_synced_options_and_tabs_is_readable() {
        let timestamp = "2026-01-01T00:00:00Z";
        let metadata: InstanceMetadata = serde_json::from_value(serde_json::json!({
            "instance": {
                "id": "legacy", "path": "legacy", "install_stage": "installed",
                "launcher_feature_version": "none", "update_channel": "release",
                "name": "Legacy", "created": timestamp, "modified": timestamp,
                "submitted_time_played": 0, "recent_time_played": 0
            },
            "applied_content_set": {
                "id": "set", "instance_id": "legacy", "name": "Legacy",
                "source_kind": "local", "status": "available", "game_version": "1.20.1",
                "loader": "fabric", "created": timestamp, "modified": timestamp
            },
            "link": "unmanaged", "group_ids": [],
            "launch_overrides": { "instance_id": "legacy", "hooks": {} }
        })).unwrap();
        assert_eq!(metadata.synced_options, InstanceSyncedOptions::default());
        assert!(metadata.launch_overrides.visible_tabs.files);
        assert!(metadata.launch_overrides.visible_tabs.worlds);
    }
}
