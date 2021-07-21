use crate::db_backend::SQLite;
use crate::populate::populate_db;
use crate::utils::execute_script;
use crate::MIRROR;
use debpkg::DebPkg;
use log::warn;
use reqwest::Url;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use tar::EntryType;
use tempfile::{tempdir, NamedTempFile};

fn get_package(db_name: &str, package: &str) -> Result<DebPkg<File>, std::io::ErrorKind> {
    if package.ends_with(".deb") {
        let source = File::open(package).expect("File not found");
        Ok(DebPkg::parse(source).expect("Parsing failed"))
    } else {
        let mut conn = SQLite::init(db_name);
        let tx = conn.transaction().unwrap();
        let (filename, _expected_md5, expected_sha256) = tx
            .query_row(
                "SELECT filename, md5, sha256 FROM status_available WHERE package = ?1",
                params![package.trim()],
                |r| {
                    Ok((
                        r.get::<_, String>(0),
                        r.get::<_, String>(1),
                        r.get::<_, String>(2),
                    ))
                },
            )
            .unwrap();
        tx.commit().unwrap();
        conn.close();

        if filename.is_err() {
            warn!("{}", filename.unwrap_err());
            return Err(std::io::ErrorKind::NotFound);
        }

        let url = Url::parse(MIRROR)
            .unwrap()
            .join(&filename.unwrap())
            .unwrap();

        let p = reqwest::blocking::get(url).unwrap();
        if !p.status().is_success() {
            warn!("Download of package failed. Status: {}", p.status());
            return Err(std::io::ErrorKind::NotFound);
        }

        // Check hash
        let mut sha256 = Sha256::new();
        let content = p.bytes().unwrap();
        sha256.update(content.as_ref());
        let hash = sha256.finalize();
        let mut hash_str = String::new();
        hash.as_slice()
            .iter()
            .for_each(|c| hash_str.push_str(&*format!("{:02x}", c)));
        if hash_str == expected_sha256.unwrap() {
            println!("Hashes match");
        } else {
            panic!("The hash value of the downloaded package is different. Abort");
        }

        let mut writer = NamedTempFile::new().unwrap();
        let reader = writer.reopen().unwrap();

        writer.write_all(content.as_ref()).unwrap();

        Ok(DebPkg::parse(reader).expect("Parsing failed"))
    }
}

pub fn install(db_name: &str, package_name: String, automatic_install: bool) {
    let mut conn = SQLite::init(db_name);

    // Check whether package is already installed
    let mut installed_stmt = conn
        .prepare("SELECT count(*) FROM status WHERE package = ?1".to_string())
        .unwrap();

    let installed = installed_stmt
        .query_row(params![package_name.trim()], |r| r.get::<_, u64>(0))
        .unwrap();

    installed_stmt.finalize().unwrap();

    if installed > 0 {
        println!("Package {} already installed", package_name);
        return;
    }

    let package = get_package(db_name, package_name.as_str());
    if package.is_err() {
        return;
    }
    let mut package = package.unwrap();
    let control_dir = tempdir().unwrap();
    package
        .control()
        .unwrap()
        .unpack(control_dir.path())
        .unwrap();

    populate_db(
        db_name,
        control_dir.path().join("control").as_path(),
        "_temp",
    );

    // Check which dependencies need to get installed
    if !automatic_install {
        let mut get_depends_stmt = conn
            .prepare(
                "WITH RECURSIVE deps as (
                    SELECT TRIM(dependency) as dependency
                    FROM dependencies_temp
                    WHERE type = 'depends'
                    UNION
                    SELECT TRIM(d.dependency) as dependency
                    FROM dependencies_available as d, deps as dr
                    WHERE TRIM(d.package) = dr.dependency AND
                          d.type = 'depends'
                )
                SELECT DISTINCT *
                FROM deps as d
                WHERE NOT EXISTS (SELECT * FROM status as s WHERE TRIM(s.package) = d.dependency);"
                    .to_string(),
            )
            .unwrap();

        let deps: Vec<String> = get_depends_stmt
            .query_map(params![], |p| p.get::<_, String>(0))
            .unwrap()
            .map(|p| p.unwrap())
            .collect();

        get_depends_stmt.finalize().unwrap();

        for d in deps {
            println!("{}", d);
            install(db_name, d, true);
        }
    }

    let tx = conn.transaction().unwrap();

    // Install dependencies and package in topological order

    // Run pre-install script
    let pre_install_script = control_dir.path().join("preinst");
    if pre_install_script.exists() {
        execute_script("pre-install", pre_install_script.as_path()).unwrap();
    }

    let mut file_stmt = tx
        .prepare("INSERT INTO installed_files (package, file) VALUES (?1, ?2)")
        .unwrap();

    // Copy files
    ////////////////////////////////////////////////////////////////////////////////////////////////
    // adapted from tar/src/archive.rs

    // Delay any directory entries until the end (they will be created if needed by
    // descendants), to ensure that directory permissions do not interfer with descendant
    // extraction.
    let mut directories = Vec::new();
    let mut data = package.data().unwrap();
    for entry in data.entries().unwrap() {
        let mut file = entry.unwrap();
        if file.header().entry_type() == EntryType::Directory {
            directories.push(file);
        } else {
            file_stmt
                .execute(params![
                    package_name,
                    file.path().unwrap().to_str().unwrap()
                ])
                .unwrap();
            file.unpack_in("/").unwrap();
        }
    }
    for mut dir in directories {
        dir.unpack_in("/").unwrap();
    }

    // end from archive.rs
    ////////////////////////////////////////////////////////////////////////////////////////////////

    file_stmt.finalize().unwrap();

    // Run post-install script
    let post_install_script = control_dir.path().join("postinst");
    if post_install_script.exists() {
        execute_script("post-install", post_install_script.as_path()).unwrap();
    }

    // Store pre- and post-remove scripts
    let pre_remove_script_path = control_dir.path().join("prerm");
    let mut pre_remove_script = String::new();
    if pre_remove_script_path.exists() {
        File::open(pre_remove_script_path)
            .unwrap()
            .read_to_string(&mut pre_remove_script)
            .unwrap();
    }

    let post_remove_script_path = control_dir.path().join("postrm");
    let mut post_remove_script = String::new();
    if post_remove_script_path.exists() {
        File::open(post_remove_script_path)
            .unwrap()
            .read_to_string(&mut post_remove_script)
            .unwrap();
    }

    tx.execute(
        "UPDATE status_temp SET auto_installed = ?2, status = 'install ok installed', prerm = ?3, postrm = ?4 WHERE package = ?1",
        params![package_name, automatic_install as i32, pre_remove_script, post_remove_script],
    ).unwrap();

    control_dir.close().unwrap();

    if !automatic_install {
        // Persist info in database
        tx.execute("INSERT INTO status SELECT * FROM status_temp", [])
            .unwrap();
        tx.execute(
            "INSERT INTO dependencies SELECT * FROM dependencies_temp",
            [],
        )
        .unwrap();

        // Remove temporary data
        tx.execute("DROP TABLE status_temp", []).unwrap();
        tx.execute("DROP TABLE dependencies_temp", []).unwrap();
        tx.execute("DROP TABLE conffiles_temp", []).unwrap();
    }

    tx.commit().unwrap();
}
