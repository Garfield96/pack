use crate::db_backend::SQLite;
use crate::utils::execute_script;
use rusqlite::params;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

pub fn purge(db_name: &str, package: &str) {
    let mut conn = SQLite::init(db_name);
    let tx = conn.transaction().unwrap();
    let package = package.trim();

    let mut get_info_stmt = tx
        .prepare(
            "SELECT c, IFNULL(s.prerm, ''), IFNULL(s.postrm, '') FROM status as s, (
            SELECT count(*) as c
            FROM  dependencies as dep
            WHERE dep.dependency = ?1 AND dep.type = 'depends')
            WHERE s.package = ?1",
        )
        .unwrap();

    let (dep_count, prerm, postrm) = get_info_stmt
        .query_row(params![package], |e| {
            Ok((
                e.get::<_, u64>(0).unwrap(),
                e.get::<_, String>(1).unwrap(),
                e.get::<_, String>(2).unwrap(),
            ))
        })
        .unwrap();
    get_info_stmt.finalize().unwrap();

    // is this package is a dependency, it is marked as automatically installed
    if dep_count != 0 {
        println!("{} is a dependency. Setting auto_installed", package);
        tx.execute(
            "UPDATE status SET auto_installed = 1 WHERE package = ?1",
            params![package],
        )
        .unwrap();
        tx.commit().unwrap();
        return;
    }

    // Run pre-remove script
    if !prerm.is_empty() {
        let mut pre_remove_script = NamedTempFile::new().unwrap();
        pre_remove_script.write_all(prerm.as_ref()).unwrap();
        execute_script("pre-remove", pre_remove_script.path()).unwrap();
        pre_remove_script.close().unwrap();
    }

    // Remove
    let mut files_stmt = tx
        .prepare("SELECT file FROM installed_files WHERE package = ?1")
        .unwrap();
    let files = files_stmt
        .query_map(params![package], |r| r.get::<_, String>(0))
        .unwrap();
    for f in files {
        let f = f.unwrap();
        let mut f = f.chars();
        f.next();
        let f = f.as_str();
        let f_path = Path::new(f);
        let f_parent = f_path.parent().unwrap();
        println!("Remove: {}", f_path.to_str().unwrap());
        fs::remove_file(f_path);
        // Delete directory if empty
        let dir = f_parent.read_dir();
        if dir.is_ok() && dir.unwrap().next().is_none() {
            println!("Remove dir: {}", f_parent.to_str().unwrap());
            fs::remove_dir(f_parent).unwrap();
        }
    }
    files_stmt.finalize().unwrap();

    // Run post-remove script
    if !postrm.is_empty() {
        let mut post_remove_script = NamedTempFile::new().unwrap();
        post_remove_script.write_all(postrm.as_ref()).unwrap();
        execute_script("post-remove", post_remove_script.path()).unwrap();
        post_remove_script.close().unwrap();
    }

    // Remove from DB
    for table in &["status", "dependencies", "conffiles", "installed_files"] {
        tx.execute(
            format!("DELETE FROM {} WHERE package = ?1", table).as_str(),
            params![package],
        )
        .unwrap();
    }
    tx.commit().unwrap();
}
