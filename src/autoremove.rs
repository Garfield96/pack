use crate::db_backend::SQLite;
use crate::purge::purge;
use log::debug;

pub fn autoremove(db_name: &str) {
    let mut conn = SQLite::init(db_name);
    let tx = conn.transaction().unwrap();
    let mut get_removeable_stmt = tx
        .prepare(
            "SELECT DISTINCT s.package \
            FROM status as s \
            WHERE s.auto_installed = 1 AND \
            NOT EXISTS (SELECT * FROM dependencies as d WHERE s.package = d.dependency)",
        )
        .unwrap();

    let deps = get_removeable_stmt
        .query_map([], |p| p.get::<_, String>(0))
        .unwrap();
    for d in deps {
        debug!("Remove {}", d.as_ref().unwrap());
        // purge(db_name, &d.unwrap());
    }
    get_removeable_stmt.finalize().unwrap();
    tx.commit().unwrap();
}
