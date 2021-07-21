use crate::db_backend::SQLite;
use crate::populate::populate_db;
use crate::MIRROR;
use flate2::read::GzDecoder;
use reqwest::Url;
use std::io::Cursor;
use tempfile::NamedTempFile;

pub fn update(db_name: &str) {
    let filename = "dists/bullseye/main/binary-all/Packages.gz";
    // Download metadata
    let url = Url::parse(MIRROR).unwrap().join(filename).unwrap();
    let p = reqwest::blocking::get(url).unwrap();
    if !p.status().is_success() {
        panic!("Download of package failed. Status: {}", p.status());
    }

    let mut conn = SQLite::init(db_name);
    let tx = conn.transaction().unwrap();
    // Drop old data
    tx.execute("DELETE FROM status_available", []).unwrap();
    tx.commit().unwrap();

    // Unpack data
    let content = Cursor::new(p.bytes().unwrap());
    let mut gz = GzDecoder::new(content);
    let mut writer = NamedTempFile::new().unwrap();
    std::io::copy(&mut gz, &mut writer).unwrap();

    populate_db(db_name, writer.path(), "_available");
}
