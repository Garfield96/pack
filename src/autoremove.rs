use crate::db_backend::SQLite;
use crate::purge::purge;

pub fn autoremove(db_name: &str) {
    let mut conn = SQLite::init(db_name);
    let tx = conn.transaction().unwrap();
    let mut get_removeable_stmt = tx
        .prepare(
            "WITH RECURSIVE auto_installed_packages as (
                SELECT * FROM status as s WHERE s.auto_installed = 1
            ), removable_packages as (
                SELECT DISTINCT s.package
                FROM auto_installed_packages as s LEFT JOIN dependencies as d ON s.package = d.dependency
                WHERE d.dependency IS NULL
                UNION
                SELECT d.dependency
                FROM dependencies as d, removable_packages as rd, auto_installed_packages as s
                WHERE s.package = d.dependency AND d.package = rd.package
            )
            SELECT * FROM removable_packages",
        )
        .unwrap();

    let deps = get_removeable_stmt
        .query_map([], |p| p.get::<_, String>(0))
        .unwrap()
        .map(|p| p.unwrap())
        .collect::<Vec<String>>();
    get_removeable_stmt.finalize().unwrap();
    tx.commit().unwrap();

    for d in deps {
        println!("Remove {}", d);
        purge(db_name, d.as_str());
    }
}
