//! `stardust-extract`: pulls the original game's art, sounds and levels out
//! of `Stardust_Mac_EN.sit` (or an already-unstuffed resource fork).
//!
//! `vendor/` receives faithful exports of every resource; `assets/` receives
//! the sprite sheets, sounds and level files the game loads.

mod export;

use clap::Parser;
use macrsrc::fork::ResourceFork;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// StuffIt archive, AppleDouble `.rsrc` file, or raw resource fork.
    input: PathBuf,
    /// Where raw exports go.
    #[arg(long, default_value = "vendor")]
    vendor: PathBuf,
    /// Where game-ready assets go.
    #[arg(long, default_value = "assets")]
    assets: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let bytes = fs::read(&args.input)?;
    let fork_bytes = match stuffit::parse(&bytes) {
        Ok(archive) => {
            export_read_me(&archive, &bytes, &args.vendor)?;
            application_resource_fork(&archive, &bytes)?
        }
        Err(stuffit::Error::NotStuffIt5) => bytes,
        Err(e) => return Err(e.into()),
    };
    let fork = ResourceFork::parse(&fork_bytes)?;
    let counts = export::vendor(&fork, &args.vendor)?;
    println!("vendor: {counts}");
    let counts = export::assets(&fork, &args.assets)?;
    println!("assets: {counts}");
    Ok(())
}

/// Resource fork of the `APPL` entry, the only one that holds game data.
fn application_resource_fork(
    archive: &stuffit::Archive,
    bytes: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let entry = archive
        .entries
        .iter()
        .find(|e| &e.finder_type == b"APPL")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no application in archive"))?;
    let fork = entry.resource_fork.as_ref().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "application has no resource fork")
    })?;
    Ok(archive.read_fork(bytes, fork)?)
}

/// The manual ships as a plain-text data fork next to the application.
fn export_read_me(
    archive: &stuffit::Archive,
    bytes: &[u8],
    vendor: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let read_me = archive
        .entries
        .iter()
        .filter(|e| !e.is_dir)
        .find(|e| e.path.contains("Read Me"));
    let Some(fork) = read_me.and_then(|e| e.data_fork.as_ref()) else {
        return Ok(());
    };
    let text = macrsrc::mac_roman(&archive.read_fork(bytes, fork)?).replace('\r', "\n");
    Ok(write(&vendor.join("instructions.txt"), text.as_bytes())?)
}

fn write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, data)
}
