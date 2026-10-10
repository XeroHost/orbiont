use crate::state::DirectoryInfo;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::{Pool, Sqlite};
use std::path::Path;
use std::time::Duration;

pub(crate) async fn connect(
    app_identifier: &str,
) -> crate::Result<Pool<Sqlite>> {
    let settings_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;

    crate::util::io::create_dir_all(&settings_dir).await?;

    let db_path = settings_dir.join("app.db");

    connect_app_db(&db_path, app_identifier).await
}

async fn connect_app_db(
    db_path: &Path,
    app_identifier: &str,
) -> crate::Result<Pool<Sqlite>> {
    super::db_backup::maybe_backup_existing_app_db(db_path, app_identifier)
        .await?;
    open_migrated_app_db(db_path).await
}

async fn open_migrated_app_db(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    let pool = open_app_db_pool(db_path).await?;

    if let Err(err) = stale_data_cleanup(&pool).await {
        tracing::warn!(
            "Failed to clean up stale data from state database before migrations: {err}"
        );
    }

    sqlx::migrate!().run(&pool).await?;
    record_current_app_version(&pool).await?;

    if let Err(err) = stale_data_cleanup(&pool).await {
        tracing::warn!(
            "Failed to clean up stale data from state database: {err}"
        );
    }

    Ok(pool)
}

async fn open_app_db_pool(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    let conn_options = SqliteConnectOptions::new()
        .filename(db_path)
        .busy_timeout(Duration::from_secs(30))
        .journal_mode(SqliteJournalMode::Wal)
        .optimize_on_close(true, None)
        .create_if_missing(true);

    Ok(SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .idle_timeout(Duration::from_secs(120))
        .connect_with(conn_options)
        .await?)
}

async fn record_current_app_version(pool: &Pool<Sqlite>) -> crate::Result<()> {
    let previous_version = sqlx::query_scalar!(
        "SELECT value FROM app_metadata WHERE key = 'app_version'"
    )
    .fetch_optional(pool)
    .await?;
    let already_used_sync_update = previous_version
        .as_deref()
        .and_then(|version| {
            let mut parts = version.split('.');
            Some((
                parts.next()?.parse::<u64>().ok()?,
                parts.next()?.parse::<u64>().ok()?,
            ))
        })
        .is_some_and(|version| version >= (0, 20));

    if env!("CARGO_PKG_VERSION").starts_with("0.20.")
        && already_used_sync_update
    {
        let mut settings = super::Settings::get(pool).await?;
        if settings.pending_update_toast_for_version.is_some() {
            settings.pending_update_toast_for_version = None;
            settings.update(pool).await?;
        }
    }

    sqlx::query!(
        "
		INSERT INTO app_metadata (key, value, updated_at)
		VALUES ('app_version', ?, unixepoch())
		ON CONFLICT(key) DO UPDATE SET
			value = excluded.value,
			updated_at = excluded.updated_at
		",
        env!("CARGO_PKG_VERSION"),
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Cleans up data from the database that is no longer referenced, but must be
/// kept around for a little while to allow users to recover from accidental
/// deletions.
async fn stale_data_cleanup(pool: &Pool<Sqlite>) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

    let has_skin_tables = sqlx::query!(
		"SELECT COUNT(*) AS \"count!: i64\" FROM sqlite_master WHERE type = 'table' AND name IN ('custom_minecraft_skins', 'minecraft_users')",
	)
	.fetch_one(&mut *tx)
	.await?
	.count == 2;

    if has_skin_tables {
        sqlx::query!(
			"DELETE FROM custom_minecraft_skins WHERE minecraft_user_uuid NOT IN (SELECT uuid FROM minecraft_users)"
		)
		.execute(&mut *tx)
		.await?;
    }

    tx.commit().await?;

    Ok(())
}

#[cfg(test)]
mod upstream_regression_tests {
    use super::*;
    #[tokio::test]
    async fn concurrent_immediate_writers_do_not_upgrade_stale_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let pool = open_app_db_pool(&dir.path().join("concurrent.db"))
            .await
            .unwrap();
        sqlx::query("CREATE TABLE counter (value INTEGER NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO counter VALUES (0)")
            .execute(&pool)
            .await
            .unwrap();
        let mut writers = Vec::new();
        for _ in 0..8 {
            let pool = pool.clone();
            writers.push(tokio::spawn(async move {
                let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
                let value: i64 =
                    sqlx::query_scalar("SELECT value FROM counter")
                        .fetch_one(&mut *tx)
                        .await
                        .unwrap();
                tokio::task::yield_now().await;
                sqlx::query("UPDATE counter SET value = ?")
                    .bind(value + 1)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
            }));
        }
        for writer in writers {
            writer.await.unwrap();
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT value FROM counter")
                .fetch_one(&pool)
                .await
                .unwrap(),
            8
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn json_reads_accept_text_and_binary_recovery_state() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE jobs (state BLOB)")
            .execute(&pool)
            .await
            .unwrap();
        let original = crate::install::model::InstallJobState::new(
            crate::install::InstallRequest::BulkUpdateContent {
                instance_id: "legacy".into(),
                updates: vec![],
            },
        );
        let json = serde_json::to_string(&original).unwrap();
        sqlx::query("INSERT INTO jobs VALUES (?), (jsonb(?))")
            .bind(&json)
            .bind(&json)
            .execute(&pool)
            .await
            .unwrap();
        let states: Vec<String> =
            sqlx::query_scalar("SELECT json(state) FROM jobs")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(states.len(), 2);
        for state in states {
            let recovered: crate::install::model::InstallJobState =
                serde_json::from_str(&state).unwrap();
            assert_eq!(recovered.schema_version, original.schema_version);
            assert!(matches!(recovered.request,
                crate::install::InstallRequest::BulkUpdateContent { instance_id, .. }
                if instance_id == "legacy"));
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&state).unwrap(),
                serde_json::from_str::<serde_json::Value>(&json).unwrap()
            );
        }
    }
}
