use super::install_from::{PackDependency, PackFormat};
use crate::state::ModLoader;
use crate::state::content_store::input;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub(crate) const INDEX_NAME: &str = "orbiont.index.json";

/// Orbpack v1 is a self-contained ZIP: this manifest and selected files in overrides/.
/// It has no provider project IDs or external download references.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OrbpackManifest {
    pub format: String,
    pub format_version: u32,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optifine: Option<crate::optifine::OptifineReference>,
}

impl OrbpackManifest {
    pub(crate) fn new(
        game_version: String,
        loader: ModLoader,
        loader_version: Option<String>,
        name: String,
        version: String,
        description: Option<String>,
    ) -> crate::Result<Self> {
        let manifest = Self {
            format: "orbiont".into(),
            format_version: 1,
            name,
            version,
            description,
            game_version,
            loader,
            loader_version,
            optifine: None,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub(crate) fn validate(&self) -> crate::Result<()> {
        if let Some(reference) = &self.optifine {
            reference.check_compatibility(&self.game_version, self.loader)?;
        }
        if self.format != "orbiont" || self.format_version != 1 {
            return Err(input("Unsupported Orbpack format or version"));
        }
        if self.name.trim().is_empty()
            || self.version.trim().is_empty()
            || self.game_version.trim().is_empty()
        {
            return Err(input("Orbpack metadata is incomplete"));
        }
        if self.loader != ModLoader::Vanilla
            && self
                .loader_version
                .as_ref()
                .is_none_or(|v| v.trim().is_empty())
        {
            return Err(input("Orbpack loader version is missing"));
        }
        Ok(())
    }

    pub(crate) fn into_pack_format(self) -> crate::Result<PackFormat> {
        self.validate()?;
        let mut dependencies =
            HashMap::from([(PackDependency::Minecraft, self.game_version)]);
        let loader = match self.loader {
            ModLoader::Vanilla => None,
            ModLoader::Forge => Some(PackDependency::Forge),
            ModLoader::NeoForge => Some(PackDependency::NeoForge),
            ModLoader::Fabric => Some(PackDependency::FabricLoader),
            ModLoader::Quilt => Some(PackDependency::QuiltLoader),
        };
        if let Some(loader) = loader {
            dependencies.insert(loader, self.loader_version.unwrap());
        }
        Ok(PackFormat {
            game: "minecraft".into(),
            format_version: 1,
            version_id: self.version,
            name: self.name,
            summary: self.description,
            files: Vec::new(),
            dependencies,
            optifine: self.optifine,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbpack_roundtrip_preserves_metadata_for_every_loader() {
        for loader in [
            ModLoader::Vanilla,
            ModLoader::Forge,
            ModLoader::NeoForge,
            ModLoader::Fabric,
            ModLoader::Quilt,
        ] {
            let manifest = OrbpackManifest::new(
                "1.21.1".into(),
                loader,
                Some("0.16.0".into()),
                "Pack".into(),
                "2.0".into(),
                Some("Description".into()),
            )
            .unwrap();
            let bytes = serde_json::to_vec(&manifest).unwrap();
            let decoded: OrbpackManifest =
                serde_json::from_slice(&bytes).unwrap();
            let pack = decoded.into_pack_format().unwrap();
            assert_eq!(pack.name, "Pack");
            assert_eq!(pack.version_id, "2.0");
            assert_eq!(pack.summary.as_deref(), Some("Description"));
            assert_eq!(pack.dependencies[&PackDependency::Minecraft], "1.21.1");
            assert_eq!(
                pack.dependencies.len(),
                if loader == ModLoader::Vanilla { 1 } else { 2 }
            );
            assert!(pack.files.is_empty());
        }
    }

    #[test]
    fn orbpack_rejects_incompatible_or_incomplete_metadata() {
        let mut manifest = OrbpackManifest::new(
            "1.21.1".into(),
            ModLoader::Vanilla,
            None,
            "Pack".into(),
            "1.0".into(),
            None,
        )
        .unwrap();
        manifest.format_version = 2;
        assert!(manifest.into_pack_format().is_err());
        assert!(
            OrbpackManifest::new(
                "1.21.1".into(),
                ModLoader::Fabric,
                None,
                "Pack".into(),
                "1.0".into(),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn orbpack_preserves_exact_optifine_reference_and_rejects_incompatible_instances()
     {
        let mut manifest = OrbpackManifest::new(
            "1.21.1".into(),
            ModLoader::Vanilla,
            None,
            "Pack".into(),
            "1".into(),
            None,
        )
        .unwrap();
        assert!(
            serde_json::to_value(&manifest)
                .unwrap()
                .get("optifine")
                .is_none()
        );
        let reference = crate::optifine::OptifineReference {
            minecraft_version: "1.21.1".into(),
            version: "HD_U_J1".into(),
            installer_sha256: "a".repeat(64),
        };
        manifest.optifine = Some(reference.clone());
        let mut decoded: OrbpackManifest =
            serde_json::from_slice(&serde_json::to_vec(&manifest).unwrap())
                .unwrap();
        assert_eq!(decoded.optifine, Some(reference.clone()));
        decoded.loader = ModLoader::Fabric;
        decoded.loader_version = Some("0.16.0".into());
        assert!(decoded.into_pack_format().is_err());
        manifest.game_version = "1.20.1".into();
        assert!(manifest.validate().is_err());
        manifest.game_version = "1.21.1".into();
        assert_eq!(
            manifest.into_pack_format().unwrap().optifine,
            Some(reference)
        );
    }
}
