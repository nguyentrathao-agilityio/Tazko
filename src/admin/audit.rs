use rusqlite::{Connection, params};
use serde_json::Value;

pub fn record(
    c: &Connection,
    actor: Option<i64>,
    action: &str,
    org_id: Option<i64>,
    target: &str,
    detail: Value,
) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO audit_log (actor_id, action, org_id, target, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![actor, action, org_id, target, detail.to_string()],
    )?;
    Ok(())
}

pub fn kind(action: &str) -> &'static str {
    match action {
        "sign_in_refused" => "refused",
        a if a.starts_with("company_") => "company",
        a if a.starts_with("project_") || a == "board_imported" => "projects",
        a if a.starts_with("invite_") => "invites",
        _ => "people",
    }
}

pub fn actions_of(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "refused" => Some(&["sign_in_refused"]),
        "company" => Some(&[
            "company_created",
            "company_updated",
            "company_suspended",
            "company_resumed",
            "company_deleted",
        ]),
        "projects" => Some(&[
            "project_created",
            "project_archived",
            "project_restored",
            "project_deleted",
            "project_owner_added",
            "project_joined",
            "board_imported",
        ]),
        "invites" => Some(&["invite_created", "invite_revoked"]),
        "people" => Some(&[
            "role_changed",
            "member_removed",
            "super_admin_granted",
            "super_admin_revoked",
            "person_disabled",
            "person_enabled",
        ]),
        _ => None,
    }
}
