//! Lists the entries of a StuffIt 5 archive given on the command line.

fn main() {
    let path = std::env::args().nth(1).expect("usage: list <archive.sit>");
    let bytes = std::fs::read(path).expect("read archive");
    let archive = stuffit::parse(&bytes).expect("parse archive");
    for entry in &archive.entries {
        let kind = if entry.is_dir { "dir " } else { "file" };
        println!("{kind} {}", entry.path);
        for (label, fork) in [("data", &entry.data_fork), ("rsrc", &entry.resource_fork)] {
            if let Some(fork) = fork {
                println!(
                    "     {label}: method {} {} -> {} bytes",
                    fork.method, fork.compressed_len, fork.uncompressed_len
                );
            }
        }
    }
}
