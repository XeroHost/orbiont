use super::{DataRoot, RootKind};
use crate::state::instances::adapters::sqlite::instance_rows::InstanceScreenshotSource;
use std::{fs, io};

pub(crate) const SOURCE_PREFIX: &str = "bedrock:";

pub(super) fn discover_sources(
    roots: &[DataRoot],
) -> io::Result<Vec<InstanceScreenshotSource>> {
    let mut sources = Vec::new();
    for root in roots.iter().filter(|root| root.kind != RootKind::Logs) {
        let mut pending = vec![("Screenshots".to_string(), 0)];
        while let Some((relative, depth)) = pending.pop() {
            if sources.len() >= 2048 {
                return Err(io::Error::other("Too many screenshot folders"));
            }
            let path = match super::workspace::resolve(root, &relative) {
                Ok(path) if path.is_dir() => path,
                Ok(_) => continue,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::InvalidInput
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            sources.push(InstanceScreenshotSource {
                id: format!("{SOURCE_PREFIX}{}:{relative}", root.id),
                name: if relative == "Screenshots" {
                    "Minecraft Bedrock".into()
                } else {
                    format!(
                        "Minecraft Bedrock · {}",
                        relative
                            .strip_prefix("Screenshots/")
                            .unwrap_or(&relative)
                    )
                },
                path: path.to_string_lossy().into_owned(),
            });
            if depth < 3 {
                let mut children = Vec::new();
                for entry in fs::read_dir(path)?.take(10_000) {
                    let entry = entry?;
                    if entry.file_type()?.is_dir()
                        && let Some(name) = entry.file_name().to_str()
                    {
                        children
                            .push((format!("{relative}/{name}"), depth + 1));
                    }
                }
                children.sort();
                pending.extend(children.into_iter().rev());
            }
        }
    }
    Ok(sources)
}

pub(crate) async fn sources() -> crate::Result<Vec<InstanceScreenshotSource>> {
    #[cfg(windows)]
    {
        // No folders are created and no installation is required to view existing captures.
        super::on_windows(|| {
            let (roaming, local) = super::user_data_dirs()?;
            let roots = super::workspace::discover(&roaming, &local)
                .map_err(super::data_error)?;
            discover_sources(&roots).map_err(super::data_error)
        })
        .await
        .map_err(|error| crate::ErrorKind::InputError(error.to_string()).into())
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

pub(crate) async fn source(
    id: &str,
) -> crate::Result<InstanceScreenshotSource> {
    #[cfg(windows)]
    {
        let id = id.to_owned();
        super::on_windows(move || {
            let (roaming, local) = super::user_data_dirs()?;
            let roots = super::workspace::discover(&roaming, &local)
                .map_err(super::data_error)?;
            resolve_source(&roots, &id).map_err(super::data_error)
        })
        .await
        .map_err(|error| crate::ErrorKind::InputError(error.to_string()).into())
    }
    #[cfg(not(windows))]
    {
        let _ = id;
        Err(
            crate::ErrorKind::InputError("Bedrock requires Windows".into())
                .into(),
        )
    }
}

fn resolve_source(
    roots: &[DataRoot],
    id: &str,
) -> io::Result<InstanceScreenshotSource> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unknown Bedrock screenshot folder",
        )
    };
    let (root_id, relative) = id
        .strip_prefix(SOURCE_PREFIX)
        .and_then(|id| id.split_once(':'))
        .ok_or_else(invalid)?;
    let parts: Vec<_> = relative.split('/').collect();
    if parts[0] != "Screenshots" || parts.len() > 4 {
        return Err(invalid());
    }
    let root = roots
        .iter()
        .find(|root| root.id == root_id && root.kind != RootKind::Logs)
        .ok_or_else(invalid)?;
    let path = super::workspace::resolve(root, relative)?;
    if !path.is_dir() {
        return Err(invalid());
    }
    Ok(InstanceScreenshotSource {
        id: id.into(),
        name: if relative == "Screenshots" {
            "Minecraft Bedrock".into()
        } else {
            format!("Minecraft Bedrock · {}", &relative[12..])
        },
        path: path.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::RootKind;
    use super::*;
    use std::fs;

    #[test]
    fn finds_direct_and_nested_profile_images_without_packs_worlds_or_logs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("com.mojang");
        fs::create_dir_all(root.join("Screenshots/12345")).unwrap();
        fs::create_dir_all(root.join("resource_packs/example")).unwrap();
        fs::write(root.join("Screenshots/direct.png"), b"png").unwrap();
        fs::write(root.join("Screenshots/12345/photo.jpeg"), b"jpeg").unwrap();
        fs::write(root.join("resource_packs/example/pack_icon.png"), b"png")
            .unwrap();
        let roots = vec![DataRoot {
            id: "gdk-user-42".into(),
            path: root,
            kind: RootKind::User,
        }];
        let sources = discover_sources(&roots).unwrap();
        assert_eq!(sources.len(), 2);
        assert!(
            sources
                .iter()
                .any(|s| s.id == "bedrock:gdk-user-42:Screenshots")
        );
        assert!(
            sources
                .iter()
                .any(|s| s.id == "bedrock:gdk-user-42:Screenshots/12345")
        );
        assert!(
            sources
                .iter()
                .all(|s| std::path::Path::new(&s.path).is_absolute())
        );
        let logs = vec![DataRoot {
            id: "logs".into(),
            path: roots[0].path.clone(),
            kind: RootKind::Logs,
        }];
        assert!(discover_sources(&logs).unwrap().is_empty());
        assert!(
            resolve_source(&roots, "bedrock:gdk-user-42:Screenshots/12345")
                .is_ok()
        );
        for id in [
            "java-1",
            "bedrock:unknown:Screenshots",
            "bedrock:gdk-user-42:resource_packs",
            "bedrock:gdk-user-42:Screenshots/../resource_packs",
            "bedrock:gdk-user-42:Screenshots/12345/photo.jpeg",
        ] {
            assert!(resolve_source(&roots, id).is_err(), "{id}");
        }
    }

    #[tokio::test]
    async fn migration_preserves_java_groups_and_edits_and_keeps_instance_cleanup()
     {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql("PRAGMA foreign_keys = ON; CREATE TABLE instances(id TEXT PRIMARY KEY); INSERT INTO instances VALUES ('java-1');")
            .execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!(
            "../../../migrations/20260819130000_screenshot-groups.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../migrations/20260824120000_screenshot-editor-state.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql("INSERT INTO screenshots VALUES ('shot-1','java-1','a.png','hash',3,1,1,'edit'); INSERT INTO screenshot_groups VALUES ('group-1','Favorites',0); INSERT INTO screenshot_group_memberships VALUES ('shot-1','group-1');")
            .execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(include_str!(
            "../../../migrations/20261004120000_bedrock-screenshots.sql"
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT editor_state FROM screenshots WHERE id='shot-1'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "edit"
        );
        assert_eq!(sqlx::query_scalar::<_, String>("SELECT group_id FROM screenshot_group_memberships WHERE screenshot_id='shot-1'").fetch_one(&pool).await.unwrap(), "group-1");
        sqlx::query("INSERT INTO screenshot_sources(id) VALUES ('bedrock:gdk-user-42:Screenshots')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO screenshots (id,instance_id,file_name,content_hash,file_size,modified_at,created_at) VALUES ('bedrock-shot','bedrock:gdk-user-42:Screenshots','b.jpeg','hash2',4,2,2)").execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM instances WHERE id='java-1'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM screenshots")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM screenshot_group_memberships"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        sqlx::query("INSERT INTO instances VALUES ('java-2')")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM screenshot_sources WHERE id='java-2'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }
}
