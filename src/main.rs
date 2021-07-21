mod autoremove;
mod db_backend;
mod extract;
mod install;
mod populate;
mod purge;
mod update;
mod utils;

use crate::autoremove::autoremove;
use crate::extract::extract_archive;
use crate::install::install;
use crate::populate::{populate_db, populate_db_auto_installed};
use crate::purge::purge;
use crate::update::update;
use clap::Clap;
use std::path::Path;

const MIRROR: &str = "http://debian.mirror.lrz.de/debian/";

#[derive(Clap)]
#[clap(name = "pack")]
struct Cmd {
    #[clap(subcommand)]
    sub_command: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    Extract(Extract),
    Install(Install),
    Purge(Purge),
    Populate(Populate),
    Autoremove(Autoremove),
    Update(Update),
}

#[derive(Clap)]
#[clap(about = "Extracts package archive")]
struct Extract {
    #[clap(short, long, about = "Target directory", default_value = ".")]
    out: String,

    #[clap(about = "Package to extract")]
    archive: String,
}

#[derive(Clap)]
#[clap(about = "Installs package")]
struct Install {
    #[clap(about = "Package to install")]
    package: String,
}

#[derive(Clap)]
#[clap(about = "Purges installed package")]
struct Purge {
    #[clap(about = "Package to purge")]
    package: String,
}

#[derive(Clap)]
#[clap(about = "Update package metadata")]
struct Update {}

#[derive(Clap)]
#[clap(about = "Autoremove")]
struct Autoremove {}

#[derive(Clap)]
#[clap(about = "Populates DB")]
struct Populate {
    #[clap(short, long, about = "Import available packages")]
    available: bool,

    #[clap(short, long, about = "Add information about auto-installed packages")]
    installed: bool,

    #[clap(about = "Input file")]
    status_file: String,
}

fn main() {
    env_logger::init();
    let db_name = "packages.db";
    let cmd = Cmd::parse();
    match cmd.sub_command {
        SubCommand::Extract(e) => {
            println!("Extract {}", e.archive);
            let out_dir = Path::new(&e.out);
            extract_archive(out_dir, e.archive);
        }
        SubCommand::Install(i) => {
            println!("Installing {}", i.package);
            install(db_name, i.package, false);
        }
        SubCommand::Purge(p) => {
            println!("Purge {}", p.package);
            purge(db_name, &p.package);
        }
        SubCommand::Autoremove(_) => {
            autoremove(db_name);
        }
        SubCommand::Update(_) => {
            update(db_name);
        }
        SubCommand::Populate(p) => {
            println!("Read data from {}", p.status_file);
            if p.installed {
                populate_db_auto_installed(db_name, p.status_file)
            } else {
                let suffix = if p.available { "_available" } else { "" };
                populate_db(db_name, Path::new(&p.status_file), suffix);
            }
        }
    }
}
