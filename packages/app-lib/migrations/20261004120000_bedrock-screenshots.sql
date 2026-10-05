-- Extend screenshot sources to Bedrock profiles while preserving Java data.
CREATE TABLE screenshot_sources (id TEXT NOT NULL PRIMARY KEY);
INSERT INTO screenshot_sources SELECT id FROM instances;
CREATE TRIGGER screenshot_source_instance_insert AFTER INSERT ON instances
BEGIN INSERT INTO screenshot_sources(id) VALUES (NEW.id) ON CONFLICT(id) DO NOTHING; END;
CREATE TRIGGER screenshot_source_instance_delete AFTER DELETE ON instances
BEGIN DELETE FROM screenshot_sources WHERE id = OLD.id; END;

CREATE TEMP TABLE saved_screenshot_memberships AS SELECT * FROM screenshot_group_memberships;
DROP TABLE screenshot_group_memberships;
CREATE TABLE screenshots_extended (
    id TEXT NOT NULL PRIMARY KEY,
    instance_id TEXT NOT NULL,
    file_name TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    editor_state TEXT,
    UNIQUE (instance_id, file_name),
    FOREIGN KEY (instance_id) REFERENCES screenshot_sources(id) ON DELETE CASCADE
);
INSERT INTO screenshots_extended SELECT id, instance_id, file_name, content_hash, file_size, modified_at, created_at, editor_state FROM screenshots;
DROP TABLE screenshots;
ALTER TABLE screenshots_extended RENAME TO screenshots;
CREATE INDEX screenshots_instance_id ON screenshots(instance_id);
CREATE INDEX screenshots_instance_hash ON screenshots(instance_id, content_hash, file_size);
CREATE TABLE screenshot_group_memberships (
    screenshot_id TEXT NOT NULL PRIMARY KEY,
    group_id TEXT NOT NULL,
    FOREIGN KEY (screenshot_id) REFERENCES screenshots(id) ON DELETE CASCADE,
    FOREIGN KEY (group_id) REFERENCES screenshot_groups(id) ON DELETE CASCADE
);
INSERT INTO screenshot_group_memberships SELECT * FROM saved_screenshot_memberships;
DROP TABLE saved_screenshot_memberships;
CREATE INDEX screenshot_group_memberships_group_id ON screenshot_group_memberships(group_id);
