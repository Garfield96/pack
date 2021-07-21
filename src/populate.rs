use crate::db_backend::SQLite;
use debcontrol::{BufParse, Streaming};
use rusqlite::{params, Statement};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

pub fn populate_db_auto_installed(db_name: &str, file: String) {
    let mut conn = SQLite::init(db_name);

    let status_file = File::open(file).unwrap();
    let mut buf_parse = BufParse::new(status_file, 4096);
    let tx = conn.transaction().unwrap();
    let mut status_stmt = tx
        .prepare("UPDATE status SET auto_installed = ?2 WHERE package = ?1")
        .unwrap();

    while let Some(entry) = buf_parse.try_next().unwrap() {
        match entry {
            Streaming::Item(paragraph) => {
                let mut fields = HashMap::new();
                for field in paragraph.fields {
                    fields.insert(field.name, field.value);
                }
                status_stmt
                    .execute(params![fields.get("Package"), fields.get("Auto-Installed")])
                    .unwrap();
            }
            Streaming::Incomplete => {
                buf_parse.buffer().unwrap();
            }
        }
    }
    status_stmt.finalize().unwrap();
    tx.commit().unwrap();
}

fn setup_db(db_name: &str, suffix: &str) {
    let mut conn = SQLite::init(db_name);
    let tx = conn.transaction().unwrap();
    tx.execute(
        "CREATE TABLE IF NOT EXISTS priorities (\
            id INT PRIMARY KEY,\
            priority TEXT NOT NULL)",
        [],
    )
    .unwrap();

    tx.execute(
        "REPLACE INTO priorities (id, priority) VALUES \
                (0,'required'),\
                (1,'important'),\
                (2,'standard'),\
                (3,'optional'),\
                (4,'extra')",
        [],
    )
    .unwrap();

    tx.execute(
        &*format!(
            "CREATE TABLE IF NOT EXISTS status{} (\
            package TEXT PRIMARY KEY, \
            status TEXT, \
            priority INT, \
            section TEXT, \
            source TEXT, \
            version TEXT NOT NULL, \
            maintainer_name TEXT, \
            maintainer_mail TEXT, \
            architecture TEXT, \
            multi_arch TEXT, \
            installed_size INT,\
            description TEXT,\
            homepage TEXT,\
            auto_installed INT,\
            filename TEXT, \
            prerm TEXT, \
            postrm TEXT, \
            md5 TEXT CHECK(LENGTH(md5) = 32), \
            sha256 TEXT CHECK(LENGTH(sha256) = 64), \
            FOREIGN KEY(priority) REFERENCES priorities(id) \
            )",
            suffix
        ),
        [],
    )
    .unwrap();

    tx.execute(
        &*format!(
            "CREATE TABLE IF NOT EXISTS dependencies{0} (\
            package TEXT NOT NULL,\
            type TEXT NOT NULL,\
            dependency TEXT NOT NULL,\
            version TEXT,\
            FOREIGN KEY(package) REFERENCES status(package{0})
            )",
            suffix
        ),
        [],
    )
    .unwrap();

    tx.execute(
        "CREATE TABLE IF NOT EXISTS installed_files (\
            package TEXT NOT NULL,\
            file TEXT NOT NULL,\
            FOREIGN KEY(package) REFERENCES status(package)
            )",
        [],
    )
    .unwrap();

    tx.execute(
        &*format!(
            "CREATE TABLE IF NOT EXISTS conffiles{0} (\
            package TEXT NOT NULL,\
            conffile TEXT NOT NULL,\
            hash TEXT,\
            FOREIGN KEY(package) REFERENCES status(package{0})
            )",
            suffix
        ),
        [],
    )
    .unwrap();

    tx.commit().unwrap();
}

pub fn populate_db(db_name: &str, file: &Path, suffix: &str) {
    setup_db(db_name, suffix);

    let mut conn = SQLite::init(db_name);
    let status_file = File::open(file).unwrap();
    let mut buf_parse = BufParse::new(status_file, 4096);
    let tx = conn.transaction().unwrap();
    let mut status_stmt = tx
        .prepare(&*format!(
            "REPLACE INTO status{} (\
                    package, \
                    status, \
                    priority, \
                    section, \
                    source, \
                    version, \
                    maintainer_name, \
                    maintainer_mail, \
                    architecture, \
                    multi_arch, \
                    installed_size, \
                    description, \
                    homepage,\
                    auto_installed,\
                    filename,\
                    md5,\
                    sha256) \
                    VALUES (?1, ?2, \
                    (SELECT id FROM priorities WHERE priority = ?3), \
                    ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?15, ?16)",
            suffix
        ))
        .unwrap();
    let mut depends_stmt = tx
        .prepare(&*format!(
            "INSERT INTO dependencies{} (\
                    package, \
                    type, \
                    dependency, \
                    version) \
                    VALUES (?1, ?2, ?3, ?4)",
            suffix
        ))
        .unwrap();
    let mut conffiles_stmt = tx
        .prepare(&*format!(
            "INSERT INTO conffiles{} (\
                    package, \
                    conffile, \
                    hash) \
                    VALUES (?1, ?2, ?3)",
            suffix
        ))
        .unwrap();
    while let Some(entry) = buf_parse.try_next().unwrap() {
        match entry {
            Streaming::Item(paragraph) => {
                let mut fields = HashMap::new();
                for field in paragraph.fields {
                    fields.insert(field.name.trim(), field.value);
                }
                let mut maintainer_iter = fields.get("Maintainer").unwrap().split('<');
                let maintainer_name = maintainer_iter.next().unwrap();
                let maintainer_mail = maintainer_iter
                    .next()
                    .map(|maintainer_mail_value| maintainer_mail_value.replace(">", ""));
                status_stmt
                    .execute(params![
                        fields.get("Package"),
                        fields.get("Status"),
                        fields.get("Priority"),
                        fields.get("Section"),
                        fields.get("Source"),
                        fields.get("Version"),
                        maintainer_name,
                        maintainer_mail,
                        fields.get("Architecture"),
                        fields.get("Multi-Arch"),
                        fields.get("Installed-Size"),
                        fields.get("Description"),
                        fields.get("Homepage"),
                        fields.get("Filename"),
                        fields.get("MD5sum"),
                        fields.get("SHA256"),
                    ])
                    .unwrap();

                for dep_type in &[
                    "Depends",
                    "Pre-Depends",
                    "Provides",
                    "Suggests",
                    "Breaks",
                    "Replaces",
                    "Recommends",
                    "Enhances",
                    "Conflicts",
                    "Build-Using",
                ] {
                    process_dep(&mut depends_stmt, &fields, dep_type);
                }

                process_conffiles(&mut conffiles_stmt, &fields)
            }
            Streaming::Incomplete => {
                buf_parse.buffer().unwrap();
            }
        }
    }
    status_stmt.finalize().unwrap();
    depends_stmt.finalize().unwrap();
    conffiles_stmt.finalize().unwrap();
    tx.commit().unwrap();
}

fn process_conffiles(conffiles_stmt: &mut Statement, fields: &HashMap<&str, String>) {
    if let Some(conffiles) = fields.get("Conffiles") {
        let package = fields.get("Package");
        for conffile in conffiles.split('\n') {
            let mut split_iter = conffile.trim().split(' ');
            let conffile_name = split_iter.next().unwrap();
            if conffile_name.is_empty() {
                continue;
            }
            let conffile_hash = split_iter.next().unwrap_or("").trim();
            conffiles_stmt
                .execute(params![package, conffile_name, conffile_hash,])
                .unwrap();
        }
    }
}

fn process_dep(depends_stmt: &mut Statement, fields: &HashMap<&str, String>, dep_type: &str) {
    if let Some(depends) = fields.get(dep_type) {
        let package = fields.get("Package");
        for dep in depends.split(", ") {
            for alternatives in dep.split('|') {
                let mut split_iter = alternatives.split('(');
                let dep_name = split_iter.next().unwrap().trim();
                let dep_version = split_iter
                    .next()
                    .map(|dep_version_value| dep_version_value.replace(")", ""));
                depends_stmt
                    .execute(params![
                        package,
                        dep_type.to_lowercase(),
                        dep_name,
                        dep_version
                    ])
                    .unwrap();
            }
        }
    }
}
