-- Orbiont does not support shared instances or hosted servers. Existing
-- instances of those kinds become regular local instances.
UPDATE instance_links
SET link_kind = 'unmanaged'
WHERE link_kind IN ('shared_instance', 'modrinth_hosting');

UPDATE instance_content_sets
SET source_kind = 'local'
WHERE source_kind IN ('shared_instance', 'modrinth_hosting');

UPDATE instance_content_entries
SET source_kind = 'local'
WHERE source_kind IN ('shared_instance', 'modrinth_hosting');

DROP INDEX IF EXISTS instance_links_hosting_server_id;
DROP INDEX IF EXISTS instance_links_hosting_active_instance_id;
DROP INDEX IF EXISTS instance_links_shared_instance_id;

ALTER TABLE instance_links DROP COLUMN hosting_server_id;
ALTER TABLE instance_links DROP COLUMN hosting_instance_ids;
ALTER TABLE instance_links DROP COLUMN hosting_active_instance_id;
ALTER TABLE instance_links DROP COLUMN shared_instance_id;
ALTER TABLE instance_links DROP COLUMN shared_instance_role;
ALTER TABLE instance_links DROP COLUMN shared_instance_manager_id;
ALTER TABLE instance_links DROP COLUMN shared_instance_linked_user_id;
ALTER TABLE instance_links DROP COLUMN shared_instance_server_manager_name;
ALTER TABLE instance_links DROP COLUMN shared_instance_server_manager_icon_url;

DROP TABLE IF EXISTS instance_content_set_sync_state;
DROP TABLE IF EXISTS instance_content_set_remote_refs;
