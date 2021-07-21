use log::warn;
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
