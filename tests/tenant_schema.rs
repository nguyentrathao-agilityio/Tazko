use rusqlite::Connection;
use tazko::db::{TENANT, migrate, open_connection};

const COMPANY_WORDS: [&str; 6] = ["org", "orgs", "company", "companies", "tenant", "tenants"];
const KEY_WORDS: [&str; 4] = ["id", "slug", "key", "uuid"];

fn names_a_company(column: &str) -> bool {
    let parts: Vec<String> = column
        .to_lowercase()
        .split('_')
        .map(str::to_owned)
        .collect();
    parts.windows(2).any(|pair| {
        COMPANY_WORDS.contains(&pair[0].as_str()) && KEY_WORDS.contains(&pair[1].as_str())
    })
}

fn columns(conn: &Connection) -> Vec<(String, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT m.name, c.name FROM sqlite_master m, pragma_table_info(m.name) c \
             WHERE m.type IN ('table', 'view') ORDER BY m.name, c.cid",
        )
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn a_migrated_company_file_has_no_column_that_names_a_company() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = open_connection(&dir.path().join("tazko.db")).unwrap();
    assert_eq!(migrate(&mut conn, &TENANT).unwrap(), TENANT.latest());

    let all = columns(&conn);
    assert!(
        all.len() > 20,
        "expected the company schema, found {} columns",
        all.len()
    );
    let offending: Vec<String> = all
        .iter()
        .filter(|(_, column)| names_a_company(column))
        .map(|(table, column)| format!("{table}.{column}"))
        .collect();
    assert!(
        offending.is_empty(),
        "each company has its own tazko.db, so open that company's file instead of adding a company column: {offending:?}"
    );
}

#[test]
fn company_column_names_are_recognised() {
    for name in [
        "org_id",
        "company_id",
        "parent_org_id",
        "Tenant_Slug",
        "companies_key",
    ] {
        assert!(names_a_company(name), "{name}");
    }
    for name in ["project_id", "board_id", "organiser", "company_name", "id"] {
        assert!(!names_a_company(name), "{name}");
    }
}

#[test]
fn the_template_tables_are_part_of_the_company_schema() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = open_connection(&dir.path().join("tazko.db")).unwrap();
    migrate(&mut conn, &TENANT).unwrap();
    let all = columns(&conn);
    for (table, column) in [
        ("board_templates", "data"),
        ("board_templates", "background"),
        ("cards", "is_template"),
    ] {
        assert!(
            all.iter().any(|(t, c)| t == table && c == column),
            "{table}.{column} missing"
        );
    }
}
