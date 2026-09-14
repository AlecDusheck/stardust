//! Extract executable resources for the binary fidelity audit.

use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let input = PathBuf::from(
        args.next()
            .ok_or("usage: dump_code <archive.sit> <output-dir>")?,
    );
    let output = PathBuf::from(args.next().ok_or("missing output directory")?);
    let bytes = fs::read(input)?;
    let archive = stuffit::parse(&bytes)?;
    let application = archive
        .entries
        .iter()
        .find(|e| &e.finder_type == b"APPL")
        .ok_or("no application")?;
    let data = archive.read_fork(
        &bytes,
        application
            .resource_fork
            .as_ref()
            .ok_or("no resource fork")?,
    )?;
    let fork = macrsrc::fork::ResourceFork::parse(&data)?;
    fs::create_dir_all(&output)?;
    fs::write(output.join("Stardust.rsrc"), &data)?;
    for resource in &fork.resources {
        println!(
            "{} {:4} {:6} {:?}",
            resource.kind_str(),
            resource.id,
            resource.data.len(),
            resource.name
        );
        if matches!(&resource.kind, b"CODE" | b"TEXT") {
            fs::write(
                output.join(format!("{}-{}.bin", resource.kind_str(), resource.id)),
                &resource.data,
            )?;
        }
    }
    Ok(())
}
