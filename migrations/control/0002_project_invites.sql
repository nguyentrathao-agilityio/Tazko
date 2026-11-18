ALTER TABLE invites ADD COLUMN project_applied_at INTEGER;

CREATE INDEX invites_project ON invites (org_id, project_id) WHERE project_id IS NOT NULL;
