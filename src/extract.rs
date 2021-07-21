use debpkg::DebPkg;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn extract_archive(out_dir: &Path, archive: String) {
    let deb_file = File::open(archive).expect("File not found");
    let mut package = DebPkg::parse(deb_file).expect("Parsing failed");
    package
        .control()
        .unwrap()
        .unpack(out_dir.join("control"))
        .unwrap();
    package
        .data()
        .unwrap()
        .unpack(out_dir.join("data"))
        .unwrap();
    let mut version_file = File::create(out_dir.join("debian-binary")).unwrap();
    let v = package.format_version();
    version_file
        .write_all(format!("{}.{}\n", v.0, v.1).as_bytes())
        .unwrap();
}
