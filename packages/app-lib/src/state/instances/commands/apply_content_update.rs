use crate::install::{
    ContentUpdateSelection, InstallErrorContext, InstallPhaseDetails,
    InstallPhaseId, InstallProgress, InstallProgressReporter,
    InstallProgressSecondary,
};
use crate::state::instances::{
    ContentEntry, ContentSet, ContentSourceKind, InstanceFile,
    adapters::sqlite::{content_rows, instance_rows},
};
use crate::state::{
    CacheBehaviour, CachedEntry, CachedFile, Dependency, DependencyType, State,
    Version,
};
use crate::util::fetch::DownloadReason;
use futures::stream::{self, StreamExt};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::apply_content_install::{
    DownloadedProjectVersion, add_downloaded_project_version,
    add_downloaded_project_version_with_enabled, download_project_version,
    download_project_version_with_progress, rename_project_companion_file,
};
use super::check_content_updates::{ContentUpdate, check_content_updates};

#[derive(Clone, Debug)]
pub(crate) struct BulkUpdatePlan {
    project_updates: Vec<PlannedProjectUpdate>,
    dependency_additions: Vec<PlannedDependencyInstall>,
}

#[derive(Clone, Debug)]
struct PlannedProjectUpdate {
    project_id: String,
    relative_path: String,
    current_version_id: String,
    update_version_id: String,
    file_size: u64,
    duplicate_paths: Vec<String>,
}

#[derive(Clone, Debug)]
struct PlannedDependencyInstall {
    project_id: String,
    version_id: String,
    parent_version_id: String,
    file_size: u64,
}

#[derive(Clone, Debug)]
enum PlannedDownload {
    ProjectUpdate(PlannedProjectUpdate),
    DependencyAddition(PlannedDependencyInstall),
}

impl PlannedDownload {
    fn file_size(&self) -> u64 {
        match self {
            Self::ProjectUpdate(update) => update.file_size,
            Self::DependencyAddition(dependency) => dependency.file_size,
        }
    }
}

enum DownloadedBulkProject {
    ProjectUpdate(PlannedProjectUpdate, DownloadedProjectVersion),
    DependencyAddition(DownloadedProjectVersion),
}

#[derive(Clone, Debug)]
struct InstalledProject {
    relative_path: String,
    project_id: Option<String>,
    version_id: Option<String>,
    enabled: bool,
}

#[derive(Clone, Debug)]
struct ResolvedDependency {
    project_id: String,
    version_id: String,
    parent_version_id: String,
    file_size: u64,
}

pub(crate) async fn update_project(
    instance_id: &str,
    project_path: &str,
    state: &State,
) -> crate::Result<String> {
    let updates = check_content_updates(
        instance_id,
        Some(CacheBehaviour::MustRevalidate),
        state,
    )
    .await?;
    let update = updates
        .into_iter()
        .find(|update| update.relative_path == project_path)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "This project cannot be updated!".to_string(),
            )
        })?;

    apply_content_update(instance_id, project_path, &update, state).await
}

async fn apply_content_update(
    instance_id: &str,
    project_path: &str,
    update: &ContentUpdate,
    state: &State,
) -> crate::Result<String> {
    let enabled = content_rows::get_instance_file_by_relative_path(
        instance_id,
        project_path,
        &state.pool,
    )
    .await?
    .is_none_or(|file| file.enabled);
    let downloaded = download_project_version(
        instance_id,
        &update.update_version_id,
        DownloadReason::Update,
        Some(update.current_version_id.clone()),
        state,
    )
    .await?;

    validate_update_project(
        &downloaded,
        &update.project_id,
        &update.update_version_id,
    )?;

    let new_path = add_downloaded_project_version_with_enabled(
        instance_id,
        downloaded,
        ContentSourceKind::Local,
        Some(enabled),
        Some(project_path),
        state,
    )
    .await?;

    if new_path != project_path {
        rename_project_companion_file(
            instance_id,
            project_path,
            &new_path,
            state,
        )
        .await?;
    }

    Ok(new_path)
}

pub(crate) async fn apply_bulk_update(
    instance_id: &str,
    plan: BulkUpdatePlan,
    reporter: InstallProgressReporter,
    state: &State,
) -> crate::Result<()> {
    let download_total =
        plan.project_updates.len() + plan.dependency_additions.len();
    let downloads =
        download_planned_projects(instance_id, &plan, &reporter, state).await?;

    reporter
        .update(InstallPhaseId::Finalizing, None, InstallPhaseDetails::Empty)
        .await?;
    for (index, download) in downloads.into_iter().enumerate() {
        reporter
            .update(
                InstallPhaseId::Finalizing,
                Some(InstallProgress {
                    current: index as u64,
                    total: download_total as u64,
                    secondary: None,
                }),
                InstallPhaseDetails::Empty,
            )
            .await?;
        match download {
            DownloadedBulkProject::ProjectUpdate(update, downloaded) => {
                for path in &update.duplicate_paths {
                    super::content_mutation::remove_project(
                        instance_id,
                        path,
                        state,
                    )
                    .await?;
                }
                let enabled = content_rows::get_instance_file_by_relative_path(
                    instance_id,
                    &update.relative_path,
                    &state.pool,
                )
                .await?
                .is_none_or(|file| file.enabled);
                let new_path = add_downloaded_project_version_with_enabled(
                    instance_id,
                    downloaded,
                    ContentSourceKind::Local,
                    Some(enabled),
                    Some(&update.relative_path),
                    state,
                )
                .await?;

                if new_path != update.relative_path {
                    rename_project_companion_file(
                        instance_id,
                        &update.relative_path,
                        &new_path,
                        state,
                    )
                    .await?;
                }
            }
            DownloadedBulkProject::DependencyAddition(downloaded) => {
                add_downloaded_project_version(
                    instance_id,
                    downloaded,
                    ContentSourceKind::Local,
                    state,
                )
                .await?;
            }
        }
    }

    reporter.clear_context().await?;
    reporter.persist().await?;
    Ok(())
}

const BULK_DOWNLOAD_CONCURRENCY: usize = 4;

struct BulkDownloadProgress {
    bytes: Vec<u64>,
    completed: u64,
    total_bytes: u64,
}

impl BulkDownloadProgress {
    async fn report(
        &self,
        reporter: &InstallProgressReporter,
    ) -> crate::Result<()> {
        reporter
            .update(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: self.completed,
                    total: self.bytes.len() as u64,
                    secondary: Some(InstallProgressSecondary {
                        current: self.bytes.iter().sum(),
                        total: self.total_bytes,
                    }),
                }),
                InstallPhaseDetails::Empty,
            )
            .await
    }
}

async fn download_planned_projects(
    instance_id: &str,
    plan: &BulkUpdatePlan,
    reporter: &InstallProgressReporter,
    state: &State,
) -> crate::Result<Vec<DownloadedBulkProject>> {
    let planned = plan
        .project_updates
        .iter()
        .cloned()
        .map(PlannedDownload::ProjectUpdate)
        .chain(
            plan.dependency_additions
                .iter()
                .cloned()
                .map(PlannedDownload::DependencyAddition),
        )
        .collect::<Vec<_>>();
    let progress = Arc::new(Mutex::new(BulkDownloadProgress {
        bytes: vec![0; planned.len()],
        completed: 0,
        total_bytes: planned.iter().map(PlannedDownload::file_size).sum(),
    }));
    progress.lock().await.report(reporter).await?;
    let mut downloads = stream::iter(planned.into_iter().enumerate())
        .map(|(index, download)| {
            let progress = progress.clone();
            let reporter = reporter.clone();
            async move {
                let size = download.file_size();
                let (version_id, reason, dependent_on) = match &download {
                    PlannedDownload::ProjectUpdate(update) => (
                        &update.update_version_id,
                        DownloadReason::Update,
                        update.current_version_id.clone(),
                    ),
                    PlannedDownload::DependencyAddition(dependency) => (
                        &dependency.version_id,
                        DownloadReason::Dependency,
                        dependency.parent_version_id.clone(),
                    ),
                };
                let context =
                    InstallErrorContext::new("download content update")
                        .version_id(version_id.clone())
                        .build();
                let progress_callback = progress.clone();
                let reporter_callback = reporter.clone();
                let mut on_progress = move |current: u64,
                                            _total: u64|
                      -> Pin<
                    Box<dyn Future<Output = crate::Result<()>> + Send>,
                > {
                    let progress = progress_callback.clone();
                    let reporter = reporter_callback.clone();
                    Box::pin(async move {
                        let mut progress = progress.lock().await;
                        progress.bytes[index] =
                            progress.bytes[index].max(current.min(size));
                        progress.report(&reporter).await
                    })
                };
                let result = download_project_version_with_progress(
                    instance_id,
                    version_id,
                    reason,
                    Some(dependent_on),
                    state,
                    Some(&mut on_progress),
                )
                .await;
                let downloaded =
                    reporter.preserve_failure_context(context, result).await?;
                let downloaded = match download {
                    PlannedDownload::ProjectUpdate(update) => {
                        validate_update_project(
                            &downloaded,
                            &update.project_id,
                            &update.update_version_id,
                        )?;
                        DownloadedBulkProject::ProjectUpdate(update, downloaded)
                    }
                    PlannedDownload::DependencyAddition(dependency) => {
                        validate_update_project(
                            &downloaded,
                            &dependency.project_id,
                            &dependency.version_id,
                        )?;
                        DownloadedBulkProject::DependencyAddition(downloaded)
                    }
                };
                let mut progress = progress.lock().await;
                progress.bytes[index] = size;
                progress.completed += 1;
                progress.report(&reporter).await?;
                Ok::<_, crate::Error>(downloaded)
            }
        })
        .buffer_unordered(BULK_DOWNLOAD_CONCURRENCY);
    let mut output = Vec::with_capacity(
        plan.project_updates.len() + plan.dependency_additions.len(),
    );
    while let Some(download) = downloads.next().await {
        output.push(download?);
    }
    Ok(output)
}

pub(crate) async fn plan_bulk_update(
    instance_id: &str,
    selections: &[ContentUpdateSelection],
    state: &State,
) -> crate::Result<BulkUpdatePlan> {
    let updateable_paths =
        bulk_updateable_project_paths(instance_id, state).await?;
    let content_set =
        content_rows::get_applied_content_set(instance_id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Instance {instance_id} has no applied content set"
                ))
            })?;
    let installed =
        installed_projects(instance_id, &content_set, state).await?;
    let mut paths = HashSet::new();
    let mut updates = Vec::with_capacity(selections.len());
    for selection in selections {
        if !updateable_paths.contains(&selection.project_path)
            || !paths.insert(&selection.project_path)
        {
            return Err(crate::state::content_store::input(
                "Selected content cannot be updated",
            ));
        }
        let project = installed
            .iter()
            .find(|project| project.relative_path == selection.project_path)
            .ok_or_else(|| {
                crate::state::content_store::input(
                    "Selected content is no longer installed",
                )
            })?;
        let project_id = project.project_id.clone().ok_or_else(|| {
            crate::state::content_store::input(
                "Selected content has no Modrinth project",
            )
        })?;
        let current_version_id =
            project.version_id.clone().ok_or_else(|| {
                crate::state::content_store::input(
                    "Selected content has no Modrinth version",
                )
            })?;
        updates.push(ContentUpdate {
            project_id,
            relative_path: selection.project_path.clone(),
            current_version_id,
            update_version_id: selection.version_id.clone(),
        });
    }
    if updates.is_empty() {
        return Ok(BulkUpdatePlan {
            project_updates: Vec::new(),
            dependency_additions: Vec::new(),
        });
    }

    let installed_by_project = installed
        .iter()
        .filter_map(|project| {
            project
                .project_id
                .as_ref()
                .map(|project_id| (project_id.clone(), project.clone()))
        })
        .collect::<HashMap<_, _>>();
    let version_ids = installed
        .iter()
        .filter_map(|project| project.version_id.clone())
        .chain(
            updates
                .iter()
                .map(|update| update.update_version_id.clone()),
        )
        .collect::<HashSet<_>>();
    let version_id_refs =
        version_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>();
    let versions = CachedEntry::get_version_many(
        &version_id_refs,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let versions_by_id = versions
        .into_iter()
        .map(|version| (version.id.clone(), version))
        .collect::<HashMap<_, _>>();
    for update in &updates {
        let version = versions_by_id
            .get(&update.update_version_id)
            .ok_or_else(|| {
                crate::state::content_store::input(
                    "Update version no longer exists",
                )
            })?;
        if !is_selected_version_compatible(
            version,
            &update.relative_path,
            &content_set,
        ) {
            return Err(crate::state::content_store::input(
                "Selected version is incompatible with this game version or loader",
            ));
        }
        if version.project_id != update.project_id {
            return Err(crate::state::content_store::input(
                "Cannot update content to a different Modrinth project",
            ));
        }
    }
    let updates_by_project =
        consolidate_selected_updates(updates, &installed, &versions_by_id);
    let updates_by_path = updates_by_project
        .values()
        .map(|(update, _)| {
            (
                update.relative_path.clone(),
                update.update_version_id.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let removed_paths = updates_by_project
        .values()
        .flat_map(|(_, duplicates)| duplicates.iter().map(String::as_str))
        .collect::<HashSet<_>>();
    let planned_versions = installed
        .iter()
        .filter(|project| {
            project.enabled
                && !removed_paths.contains(project.relative_path.as_str())
        })
        .map(|project| {
            let target = updates_by_path
                .get(&project.relative_path)
                .or(project.version_id.as_ref())
                .ok_or_else(|| {
                    crate::state::content_store::input(
                        "Installed content has no version metadata",
                    )
                })?;
            let version = versions_by_id.get(target).ok_or_else(|| {
                crate::state::content_store::input(
                    "Installed content version metadata is unavailable",
                )
            })?;
            if project.project_id.as_deref()
                != Some(version.project_id.as_str())
            {
                return Err(crate::state::content_store::input(
                    "Installed content identity does not match its version",
                ));
            }
            Ok(version.clone())
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let planned_dependencies = dependency_closure(
        planned_versions,
        &content_set,
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    for dependency in planned_dependencies.values() {
        if installed_by_project
            .get(&dependency.project_id)
            .is_some_and(|project| !project.enabled)
        {
            return Err(crate::state::content_store::input(
                "A required dependency is disabled; enable it or change the selected versions",
            ));
        }
    }
    let dependency_additions = planned_dependencies
        .values()
        .filter(|dependency| {
            !installed_by_project.contains_key(&dependency.project_id)
        })
        .map(|dependency| PlannedDependencyInstall {
            project_id: dependency.project_id.clone(),
            version_id: dependency.version_id.clone(),
            parent_version_id: dependency.parent_version_id.clone(),
            file_size: dependency.file_size,
        })
        .collect::<Vec<_>>();
    let project_updates = updates_by_project
        .into_values()
        .map(|(update, duplicate_paths)| {
            let version = versions_by_id
                .get(&update.update_version_id)
                .ok_or_else(|| {
                    crate::state::content_store::input(
                        "Update version no longer exists",
                    )
                })?;
            Ok(PlannedProjectUpdate {
                project_id: update.project_id,
                relative_path: update.relative_path,
                current_version_id: update.current_version_id,
                update_version_id: update.update_version_id,
                file_size: selected_file_size(version)?,
                duplicate_paths,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;

    Ok(BulkUpdatePlan {
        project_updates,
        dependency_additions,
    })
}

fn consolidate_selected_updates(
    updates: Vec<ContentUpdate>,
    installed: &[InstalledProject],
    versions_by_id: &HashMap<String, Version>,
) -> HashMap<String, (ContentUpdate, Vec<String>)> {
    let mut updates_by_project: HashMap<String, (ContentUpdate, Vec<String>)> =
        HashMap::new();
    let is_enabled = |path: &str| {
        installed
            .iter()
            .any(|project| project.relative_path == path && project.enabled)
    };
    for update in updates {
        if let Some((current, duplicate_paths)) =
            updates_by_project.get_mut(&update.project_id)
        {
            if versions_by_id[&update.update_version_id].date_published
                > versions_by_id[&current.update_version_id].date_published
            {
                current
                    .update_version_id
                    .clone_from(&update.update_version_id);
            }
            if is_enabled(&update.relative_path)
                && !is_enabled(&current.relative_path)
            {
                duplicate_paths.push(std::mem::replace(
                    &mut current.relative_path,
                    update.relative_path,
                ));
                current.current_version_id = update.current_version_id;
            } else {
                duplicate_paths.push(update.relative_path);
            }
        } else {
            updates_by_project
                .insert(update.project_id.clone(), (update, Vec::new()));
        }
    }
    updates_by_project
}

async fn bulk_updateable_project_paths(
    instance_id: &str,
    state: &State,
) -> crate::Result<HashSet<String>> {
    let items = super::list_content::list_indexed_content(
        instance_id,
        Some(CacheBehaviour::MustRevalidate),
        state,
    )
    .await?;

    Ok(items
        .into_iter()
        .filter(|item| !item.locked)
        .map(|item| item.file_path)
        .collect())
}

async fn installed_projects(
    instance_id: &str,
    content_set: &ContentSet,
    state: &State,
) -> crate::Result<Vec<InstalledProject>> {
    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let entries =
        content_rows::get_content_entries(&content_set.id, &state.pool).await?;
    let entries_by_file_id = entries
        .iter()
        .filter_map(|entry| {
            entry.file_id.as_deref().map(|file_id| (file_id, entry))
        })
        .collect::<HashMap<_, _>>();
    let files =
        content_rows::get_instance_files(&instance.id, &state.pool).await?;

    let hashes = files
        .iter()
        .map(|file| file.sha1.as_str())
        .collect::<Vec<_>>();
    let file_info = CachedEntry::get_file_many(
        &hashes,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let file_info_by_hash = file_info
        .into_iter()
        .map(|file| (file.hash.clone(), file))
        .collect::<HashMap<_, _>>();

    Ok(files
        .into_iter()
        .filter(|file| !file.missing)
        .filter_map(|file| {
            let entry = entries_by_file_id.get(file.id.as_str()).copied();
            let metadata = file_info_by_hash.get(&file.sha1);
            installed_project_from_row(&file, entry, metadata)
        })
        .collect())
}

fn installed_project_from_row(
    file: &InstanceFile,
    entry: Option<&ContentEntry>,
    cached: Option<&CachedFile>,
) -> Option<InstalledProject> {
    if file.missing {
        return None;
    }
    // Never pair a persisted identity with another project's hash metadata.
    let cached = cached.filter(|cached| {
        entry
            .and_then(|entry| entry.project_id.as_deref())
            .is_none_or(|id| id == cached.project_id)
    });
    let project_id = entry
        .and_then(|entry| entry.project_id.clone())
        .or_else(|| cached.map(|file| file.project_id.clone()));
    let version_id = entry
        .and_then(|entry| entry.version_id.clone())
        .or_else(|| cached.map(|file| file.version_id.clone()));
    if project_id.is_none() && version_id.is_none() {
        return None;
    }

    Some(InstalledProject {
        relative_path: file.relative_path.clone(),
        project_id,
        version_id,
        enabled: entry.is_none_or(|entry| entry.enabled) && file.enabled,
    })
}

async fn dependency_closure(
    root_versions: Vec<Version>,
    content_set: &ContentSet,
    pool: &sqlx::SqlitePool,
    semaphore: &crate::util::fetch::FetchSemaphore,
) -> crate::Result<HashMap<String, ResolvedDependency>> {
    let mut output = HashMap::new();
    let mut planned = HashMap::new();
    for version in &root_versions {
        if planned
            .insert(version.project_id.clone(), version.clone())
            .is_some_and(|previous: Version| previous.id != version.id)
        {
            return Err(crate::state::content_store::input(
                "Conflicting installed versions belong to the same project",
            ));
        }
    }
    let mut chosen = planned.clone();
    let mut stack = root_versions;
    let mut visited_versions = HashSet::new();
    let mut version_cache = HashMap::new();
    let mut project_versions_cache = HashMap::new();

    while let Some(version) = stack.pop() {
        if !visited_versions.insert(version.id.clone()) {
            continue;
        }

        for dependency in &version.dependencies {
            if !is_required_dependency(dependency, content_set) {
                continue;
            }

            let installed = dependency
                .project_id
                .as_ref()
                .and_then(|id| planned.get(id))
                .filter(|_| dependency.version_id.is_none())
                .cloned();
            let dependency_version = if let Some(installed) = installed {
                Some(installed)
            } else {
                resolve_dependency_version(
                    dependency,
                    content_set,
                    pool,
                    semaphore,
                    &mut version_cache,
                    &mut project_versions_cache,
                )
                .await?
            }
            .ok_or_else(|| {
                crate::state::content_store::input(
                    "Required dependency cannot be resolved for this instance",
                )
            })?;
            validate_resolved_dependency(
                dependency,
                &dependency_version,
                content_set,
                &planned,
                &output,
            )?;
            let project_id = dependency
                .project_id
                .clone()
                .unwrap_or_else(|| dependency_version.project_id.clone());
            let file_size = selected_file_size(&dependency_version)?;

            output.entry(project_id.clone()).or_insert_with(|| {
                ResolvedDependency {
                    project_id,
                    version_id: dependency_version.id.clone(),
                    parent_version_id: version.id.clone(),
                    file_size,
                }
            });
            chosen.insert(
                dependency_version.project_id.clone(),
                dependency_version.clone(),
            );
            stack.push(dependency_version);
        }
    }

    validate_incompatible_dependencies(&chosen)?;
    Ok(output)
}

fn validate_incompatible_dependencies(
    chosen: &HashMap<String, Version>,
) -> crate::Result<()> {
    for version in chosen.values() {
        for dependency in &version.dependencies {
            if matches!(
                dependency.dependency_type,
                DependencyType::Incompatible
            ) && chosen.values().any(|other| {
                dependency
                    .version_id
                    .as_ref()
                    .is_some_and(|id| id == &other.id)
                    || (dependency.version_id.is_none()
                        && dependency
                            .project_id
                            .as_ref()
                            .is_some_and(|id| id == &other.project_id))
            }) {
                return Err(crate::state::content_store::input(
                    "The selected update conflicts with installed content marked incompatible",
                ));
            }
        }
    }
    Ok(())
}

fn validate_resolved_dependency(
    dependency: &Dependency,
    version: &Version,
    content_set: &ContentSet,
    planned: &HashMap<String, Version>,
    resolved: &HashMap<String, ResolvedDependency>,
) -> crate::Result<()> {
    if dependency
        .project_id
        .as_deref()
        .is_some_and(|id| id != version.project_id)
        || dependency
            .version_id
            .as_deref()
            .is_some_and(|id| id != version.id)
    {
        return Err(crate::state::content_store::input(
            "Required dependency identity does not match the resolved version",
        ));
    }
    if !is_dependency_version_compatible(version, content_set) {
        return Err(crate::state::content_store::input(
            "Required dependency is incompatible with this game version or loader",
        ));
    }
    if planned
        .get(&version.project_id)
        .is_some_and(|installed| installed.id != version.id)
        || resolved
            .get(&version.project_id)
            .is_some_and(|previous| previous.version_id != version.id)
    {
        return Err(crate::state::content_store::input(
            "Required dependency conflicts with an installed or selected version; change the selection before updating",
        ));
    }
    Ok(())
}

fn is_required_dependency(
    dependency: &Dependency,
    content_set: &ContentSet,
) -> bool {
    matches!(dependency.dependency_type, DependencyType::Required)
        && !(dependency.project_id.as_deref() == Some("P7dR8mSH")
            && content_set.loader.as_str() == "quilt")
}

async fn resolve_dependency_version(
    dependency: &Dependency,
    content_set: &ContentSet,
    pool: &sqlx::SqlitePool,
    semaphore: &crate::util::fetch::FetchSemaphore,
    version_cache: &mut HashMap<String, Option<Version>>,
    project_versions_cache: &mut HashMap<String, Option<Vec<Version>>>,
) -> crate::Result<Option<Version>> {
    if let Some(version_id) = &dependency.version_id {
        return cached_version(version_id, version_cache, pool, semaphore)
            .await;
    }

    let Some(project_id) = &dependency.project_id else {
        return Ok(None);
    };
    let Some(mut versions) = cached_project_versions(
        project_id,
        project_versions_cache,
        pool,
        semaphore,
    )
    .await?
    else {
        return Ok(None);
    };

    versions.sort_by_key(|version| Reverse(version.date_published));

    Ok(find_preferred_dependency_version(&versions, content_set))
}

async fn cached_version(
    version_id: &str,
    version_cache: &mut HashMap<String, Option<Version>>,
    pool: &sqlx::SqlitePool,
    semaphore: &crate::util::fetch::FetchSemaphore,
) -> crate::Result<Option<Version>> {
    if !version_cache.contains_key(version_id) {
        let version = CachedEntry::get_version(
            version_id,
            Some(CacheBehaviour::MustRevalidate),
            pool,
            semaphore,
        )
        .await?;
        version_cache.insert(version_id.to_string(), version);
    }

    Ok(version_cache.get(version_id).cloned().flatten())
}

async fn cached_project_versions(
    project_id: &str,
    project_versions_cache: &mut HashMap<String, Option<Vec<Version>>>,
    pool: &sqlx::SqlitePool,
    semaphore: &crate::util::fetch::FetchSemaphore,
) -> crate::Result<Option<Vec<Version>>> {
    if !project_versions_cache.contains_key(project_id) {
        let versions = CachedEntry::get_project_versions(
            project_id,
            Some(CacheBehaviour::MustRevalidate),
            pool,
            semaphore,
        )
        .await?;
        project_versions_cache.insert(project_id.to_string(), versions);
    }

    Ok(project_versions_cache.get(project_id).cloned().flatten())
}

fn find_preferred_dependency_version(
    versions: &[Version],
    content_set: &ContentSet,
) -> Option<Version> {
    versions
        .iter()
        .find(|version| {
            version.game_versions.contains(&content_set.game_version)
                && version
                    .loaders
                    .iter()
                    .any(|loader| loader == content_set.loader.as_str())
        })
        .or_else(|| {
            versions.iter().find(|version| {
                is_dependency_version_compatible(version, content_set)
            })
        })
        .cloned()
}

fn is_dependency_version_compatible(
    version: &Version,
    content_set: &ContentSet,
) -> bool {
    version.game_versions.contains(&content_set.game_version)
        && (version
            .loaders
            .iter()
            .any(|loader| loader == content_set.loader.as_str())
            || version.loaders.iter().any(|loader| loader == "datapack"))
}

fn is_selected_version_compatible(
    version: &Version,
    relative_path: &str,
    content_set: &ContentSet,
) -> bool {
    let project_type =
        crate::state::ProjectType::get_from_parent_folder(relative_path);
    if !version.game_versions.contains(&content_set.game_version)
        || project_type
            != crate::state::ProjectType::get_from_loaders(
                version.loaders.clone(),
            )
    {
        return false;
    }
    match project_type {
        Some(crate::state::ProjectType::Mod) => version
            .loaders
            .iter()
            .any(|loader| loader == content_set.loader.as_str()),
        Some(_) => true,
        None => false,
    }
}

fn selected_file_size(version: &Version) -> crate::Result<u64> {
    version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .map(|file| u64::from(file.size))
        .ok_or_else(|| {
            crate::state::content_store::input("Update version has no files")
        })
}

fn validate_update_project(
    downloaded: &DownloadedProjectVersion,
    project_id: &str,
    version_id: &str,
) -> crate::Result<()> {
    if downloaded.version_id != version_id {
        return Err(crate::state::content_store::input(
            "Downloaded version differs from the selected version",
        ));
    }
    validate_project_identity(&downloaded.project_id, project_id)
}

fn validate_project_identity(
    actual: &str,
    expected: &str,
) -> crate::Result<()> {
    if actual != expected {
        return Err(crate::ErrorKind::InputError(
            "Cannot update content to a different Modrinth project".to_string(),
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod upstream_content_tests {
    use super::*;
    fn file() -> InstanceFile {
        InstanceFile {
            id: "file".into(),
            instance_id: "instance".into(),
            relative_path: "mods/test.jar".into(),
            file_name: "test.jar".into(),
            enabled: false,
            sha1: "a".repeat(40),
            size: 12,
            missing: false,
            added_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
        }
    }
    #[test]
    fn identity_requires_exact_original_project() {
        assert!(validate_project_identity("original", "original").is_ok());
        assert!(validate_project_identity("takeover", "original").is_err());
        assert!(validate_project_identity("Original", "original").is_err());
    }
    #[test]
    fn installed_cache_fallback_preserves_disabled_and_excludes_missing() {
        let mut file = file();
        let cached = CachedFile {
            hash: file.sha1.clone(),
            project_id: "original".into(),
            version_id: "v1".into(),
        };
        let installed =
            installed_project_from_row(&file, None, Some(&cached)).unwrap();
        assert_eq!(installed.project_id.as_deref(), Some("original"));
        assert!(!installed.enabled);
        file.missing = true;
        assert!(
            installed_project_from_row(&file, None, Some(&cached)).is_none()
        );
    }
    #[test]
    fn persisted_identity_is_not_combined_with_foreign_cached_version() {
        let file = file();
        let entry = ContentEntry {
            id: "entry".into(),
            instance_id: file.instance_id.clone(),
            content_set_id: "set".into(),
            file_id: Some(file.id.clone()),
            project_type: crate::state::ProjectType::Mod,
            project_id: Some("original".into()),
            version_id: None,
            source_kind: ContentSourceKind::Local,
            server_requirement: crate::state::ContentRequirement::Unknown,
            client_requirement: crate::state::ContentRequirement::Unknown,
            enabled: true,
            added_at: file.added_at,
            modified_at: file.modified_at,
        };
        let cached = CachedFile {
            hash: file.sha1.clone(),
            project_id: "takeover".into(),
            version_id: "evil-version".into(),
        };
        let installed =
            installed_project_from_row(&file, Some(&entry), Some(&cached))
                .unwrap();
        assert_eq!(installed.project_id.as_deref(), Some("original"));
        assert_eq!(installed.version_id, None);
    }
    fn version(id: &str, date: &str) -> Version {
        serde_json::from_value(serde_json::json!({"id":id,"project_id":"original","author_id":"fixture","featured":false,"name":id,"version_number":id,"changelog_url":null,"date_published":date,"downloads":0,"version_type":"release","files":[],"dependencies":[],"game_versions":["1.20.1"],"loaders":["fabric"]})).unwrap()
    }
    #[test]
    fn duplicate_selection_uses_newest_version_and_enabled_path_only() {
        let updates = vec![
            ContentUpdate {
                project_id: "original".into(),
                relative_path: "mods/disabled.jar".into(),
                current_version_id: "v0".into(),
                update_version_id: "v2".into(),
            },
            ContentUpdate {
                project_id: "original".into(),
                relative_path: "mods/enabled.jar".into(),
                current_version_id: "v1".into(),
                update_version_id: "v1".into(),
            },
        ];
        let installed = vec![
            InstalledProject {
                relative_path: "mods/disabled.jar".into(),
                project_id: Some("original".into()),
                version_id: Some("v0".into()),
                enabled: false,
            },
            InstalledProject {
                relative_path: "mods/enabled.jar".into(),
                project_id: Some("original".into()),
                version_id: Some("v1".into()),
                enabled: true,
            },
            InstalledProject {
                relative_path: "mods/user-added.jar".into(),
                project_id: Some("original".into()),
                version_id: Some("v0".into()),
                enabled: true,
            },
        ];
        let versions = HashMap::from([
            ("v1".into(), version("v1", "2025-01-01T00:00:00Z")),
            ("v2".into(), version("v2", "2026-01-01T00:00:00Z")),
        ]);
        let resolved =
            consolidate_selected_updates(updates, &installed, &versions);
        let (update, removed) = &resolved["original"];
        assert_eq!(update.update_version_id, "v2");
        assert_eq!(update.relative_path, "mods/enabled.jar");
        assert_eq!(removed, &vec!["mods/disabled.jar".to_string()]);
        assert!(!removed.contains(&"mods/user-added.jar".to_string()));
    }

    #[test]
    fn selected_versions_require_game_loader_and_content_type() {
        let mut version = version("v1", "2025-01-01T00:00:00Z");
        let set = ContentSet {
            id: "set".into(),
            instance_id: "instance".into(),
            name: "fixture".into(),
            source_kind: ContentSourceKind::Local,
            status: crate::state::ContentSetStatus::Available,
            game_version: "1.20.1".into(),
            protocol_version: None,
            loader: crate::state::ModLoader::Fabric,
            loader_version: None,
            created: chrono::Utc::now(),
            modified: chrono::Utc::now(),
        };
        assert!(is_selected_version_compatible(
            &version,
            "mods/mod.jar",
            &set
        ));
        version.loaders = vec!["forge".into()];
        assert!(!is_selected_version_compatible(
            &version,
            "mods/mod.jar",
            &set
        ));
        version.loaders = vec!["minecraft".into()];
        assert!(is_selected_version_compatible(
            &version,
            "resourcepacks/pack.zip",
            &set
        ));
        assert!(!is_selected_version_compatible(
            &version,
            "mods/mod.jar",
            &set
        ));
        version.game_versions = vec!["1.21".into()];
        assert!(!is_selected_version_compatible(
            &version,
            "resourcepacks/pack.zip",
            &set
        ));
    }
    #[test]
    fn pinned_dependency_conflicts_fail_before_content_mutation() {
        let set = ContentSet {
            id: "set".into(),
            instance_id: "instance".into(),
            name: "fixture".into(),
            source_kind: ContentSourceKind::Local,
            status: crate::state::ContentSetStatus::Available,
            game_version: "1.20.1".into(),
            protocol_version: None,
            loader: crate::state::ModLoader::Fabric,
            loader_version: None,
            created: chrono::Utc::now(),
            modified: chrono::Utc::now(),
        };
        let dependency = Dependency {
            project_id: Some("original".into()),
            version_id: Some("B2".into()),
            file_name: None,
            dependency_type: DependencyType::Required,
        };
        let b1 = version("B1", "2025-01-01T00:00:00Z");
        let mut b2 = version("B2", "2026-01-01T00:00:00Z");
        let frozen = HashMap::from([("original".into(), b1)]);
        assert!(
            validate_resolved_dependency(
                &dependency,
                &b2,
                &set,
                &frozen,
                &HashMap::new()
            )
            .is_err()
        );
        let conflict = HashMap::from([(
            "original".into(),
            ResolvedDependency {
                project_id: "original".into(),
                version_id: "B1".into(),
                parent_version_id: "A1".into(),
                file_size: 1,
            },
        )]);
        assert!(
            validate_resolved_dependency(
                &dependency,
                &b2,
                &set,
                &HashMap::new(),
                &conflict
            )
            .is_err()
        );
        assert!(
            validate_resolved_dependency(
                &dependency,
                &b2,
                &set,
                &HashMap::new(),
                &HashMap::new()
            )
            .is_ok()
        );
        b2.loaders = vec!["forge".into()];
        assert!(
            validate_resolved_dependency(
                &dependency,
                &b2,
                &set,
                &HashMap::new(),
                &HashMap::new()
            )
            .is_err()
        );
        b2.loaders = vec!["fabric".into()];
        b2.project_id = "takeover".into();
        assert!(
            validate_resolved_dependency(
                &dependency,
                &b2,
                &set,
                &HashMap::new(),
                &HashMap::new()
            )
            .is_err()
        );
    }
    #[test]
    fn explicit_incompatible_project_is_rejected() {
        let mut a = version("A2", "2026-01-01T00:00:00Z");
        a.project_id = "A".into();
        a.dependencies.push(Dependency {
            project_id: Some("original".into()),
            version_id: None,
            file_name: None,
            dependency_type: DependencyType::Incompatible,
        });
        let chosen = HashMap::from([
            ("A".into(), a),
            ("original".into(), version("B1", "2025-01-01T00:00:00Z")),
        ]);
        assert!(validate_incompatible_dependencies(&chosen).is_err());
    }
    #[tokio::test]
    async fn curseforge_prefixed_selection_reads_seeded_native_cache() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE cache (id TEXT NOT NULL, data_type TEXT NOT NULL, data BLOB, alias TEXT, expires INTEGER NOT NULL)").execute(&pool).await.unwrap();
        let mut selected = version("cf-123-456", "2026-01-01T00:00:00Z");
        selected.project_id = "cf-123".into();
        selected.files.push(crate::state::VersionFile {
            hashes: HashMap::new(),
            url: "https://fixture.invalid/content.jar".into(),
            filename: "content.jar".into(),
            primary: true,
            size: 12,
            file_type: None,
        });
        sqlx::query("INSERT INTO cache VALUES (?, 'version', ?, NULL, ?)")
            .bind(&selected.id)
            .bind(serde_json::to_string(&selected).unwrap())
            .bind(chrono::Utc::now().timestamp() + 3600)
            .execute(&pool)
            .await
            .unwrap();
        let semaphore =
            crate::util::fetch::FetchSemaphore(tokio::sync::Semaphore::new(1));
        let versions = CachedEntry::get_version_many(
            &["cf-123-456"],
            Some(CacheBehaviour::MustRevalidate),
            &pool,
            &semaphore,
        )
        .await
        .unwrap();
        assert_eq!(versions.len(), 1);
        validate_project_identity(&versions[0].project_id, "cf-123").unwrap();
        assert!(
            validate_project_identity(&versions[0].project_id, "cf-124")
                .is_err()
        );
        assert_eq!(versions[0].id, "cf-123-456");

        let set = ContentSet {
            id: "set".into(),
            instance_id: "instance".into(),
            name: "fixture".into(),
            source_kind: ContentSourceKind::Local,
            status: crate::state::ContentSetStatus::Available,
            game_version: "1.20.1".into(),
            protocol_version: None,
            loader: crate::state::ModLoader::Fabric,
            loader_version: None,
            created: chrono::Utc::now(),
            modified: chrono::Utc::now(),
        };
        let mut root = version("A2", "2026-01-01T00:00:00Z");
        root.project_id = "A".into();
        root.dependencies.push(Dependency {
            project_id: Some(selected.project_id.clone()),
            version_id: Some(selected.id.clone()),
            file_name: None,
            dependency_type: DependencyType::Required,
        });
        // Real dependency traversal consumes the seeded provider version without
        // creating launcher state or contacting a provider.
        let plan =
            dependency_closure(vec![root.clone()], &set, &pool, &semaphore)
                .await
                .unwrap();
        assert_eq!(plan["cf-123"].version_id, "cf-123-456");
        assert_eq!(plan["cf-123"].file_size, 12);

        let mut frozen = selected.clone();
        frozen.id = "cf-123-455".into();
        assert!(
            dependency_closure(
                vec![root.clone(), frozen],
                &set,
                &pool,
                &semaphore
            )
            .await
            .is_err()
        );
        assert!(
            dependency_closure(
                vec![root.clone(), selected.clone()],
                &set,
                &pool,
                &semaphore
            )
            .await
            .is_ok()
        );
        root.dependencies.push(Dependency {
            project_id: Some(selected.project_id.clone()),
            version_id: None,
            file_name: None,
            dependency_type: DependencyType::Incompatible,
        });
        assert!(
            dependency_closure(vec![root], &set, &pool, &semaphore)
                .await
                .is_err()
        );
    }
}
