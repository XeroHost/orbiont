-- Orbiont has no telemetry, no ads, no cloud preference sync and no
-- Modrinth account, so these settings never do anything.
ALTER TABLE settings DROP COLUMN telemetry;
ALTER TABLE settings DROP COLUMN personalized_ads;
ALTER TABLE settings DROP COLUMN sync_theme_across_devices;
ALTER TABLE settings DROP COLUMN sync_behavior_across_devices;
ALTER TABLE settings DROP COLUMN sync_features_across_devices;

ALTER TABLE onboarding_checklist DROP COLUMN has_logged_into_modrinth;

-- The checklist only needs an instance and a Minecraft account now.
UPDATE onboarding_checklist
SET show_checklist = FALSE
WHERE has_created_instance = TRUE AND has_logged_into_minecraft = TRUE;
