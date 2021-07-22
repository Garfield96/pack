use deb_version::compare_versions;
use log::warn;
use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, Result};
use std::cmp::Ordering;
use std::io::Error;
use std::path::Path;
use std::process::Command;

pub fn execute_script(desc: &str, pre_install_script: &Path) -> Result<(), Error> {
    println!("Execute {} script", desc);
    let out = Command::new("sh")
        .arg("-c")
        .arg(pre_install_script.to_str().unwrap())
        .output()?;
    let stderr = out.stderr;
    if !stderr.is_empty() {
        warn!(
            "{} script warnings: {}",
            desc,
            String::from_utf8(stderr).unwrap()
        );
    }
    Ok(())
}

pub fn add_version_compare(db: &Connection) -> Result<()> {
    db.create_scalar_function(
        "cmpversion",
        3,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        move |ctx| {
            assert_eq!(ctx.len(), 3, "Wrong number of arguments");
            let l = ctx.get_raw(0).as_str().unwrap();
            let cmp = ctx.get_raw(1).as_str().unwrap();
            let r = ctx.get_raw(2).as_str().unwrap();
            match compare_versions(l, r) {
                Ordering::Less => Ok(cmp.starts_with('<')),
                Ordering::Equal => Ok(cmp.ends_with('=')),
                Ordering::Greater => Ok(cmp.starts_with('>')),
            }
        },
    )
}
