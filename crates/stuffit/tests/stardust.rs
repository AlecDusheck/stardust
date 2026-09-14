//! End-to-end check against the Stardust archive. Set `STARDUST_SIT` to the
//! .sit path and `STARDUST_RSRC_TRUTH` to the AppleDouble file holding the
//! expected "Stardust 1.1" resource fork; the test skips when unset.

use std::fs;
use std::path::Path;

/// Resource fork data begins after the AppleDouble header in the ground
/// truth file produced by macOS.
const APPLEDOUBLE_RSRC_OFFSET: usize = 82;

fn env_path(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            eprintln!("{name} not set; skipping");
            None
        }
    }
}

#[test]
fn extracts_stardust_archive() {
    let (Some(sit_path), Some(truth_path)) =
        (env_path("STARDUST_SIT"), env_path("STARDUST_RSRC_TRUTH"))
    else {
        return;
    };
    let bytes = fs::read(&sit_path).expect("read archive");
    let archive = stuffit::parse(&bytes).expect("parse archive");

    let paths: Vec<&str> = archive.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains(&"stardust Folder"), "paths: {paths:?}");
    assert!(
        paths.contains(&"stardust Folder/Stardust 1.1"),
        "paths: {paths:?}"
    );
    assert!(
        paths.contains(&"stardust Folder/Instructions:Read Me"),
        "paths: {paths:?}"
    );

    // Every fork in the archive must decompress and pass its checksum.
    for entry in &archive.entries {
        for fork in entry.data_fork.iter().chain(entry.resource_fork.iter()) {
            let data = archive
                .read_fork(&bytes, fork)
                .unwrap_or_else(|e| panic!("{}: {e}", entry.path));
            assert_eq!(data.len(), fork.uncompressed_len as usize, "{}", entry.path);
        }
    }

    let app = archive
        .entries
        .iter()
        .find(|e| e.path == "stardust Folder/Stardust 1.1")
        .unwrap();
    assert!(!app.is_dir);
    assert_eq!(&app.finder_type, b"APPL");
    let rsrc = app.resource_fork.as_ref().expect("resource fork");
    assert_eq!(rsrc.uncompressed_len, 1_148_223);
    let actual = archive
        .read_fork(&bytes, rsrc)
        .expect("decompress resource fork");
    let truth = fs::read(&truth_path).expect("read ground truth");
    assert_eq!(actual, &truth[APPLEDOUBLE_RSRC_OFFSET..]);

    // The Read Me sits next to the truth file with the same naming scheme.
    let readme = archive
        .entries
        .iter()
        .find(|e| e.path == "stardust Folder/Instructions:Read Me")
        .unwrap();
    assert_eq!(
        readme.data_fork.as_ref().map(|f| f.uncompressed_len),
        Some(11_422)
    );
    assert_eq!(
        readme.resource_fork.as_ref().map(|f| f.uncompressed_len),
        Some(1_332)
    );
    let readme_truth = Path::new(&truth_path).with_file_name("Instructions:Read Me");
    if let Ok(expected) = fs::read(&readme_truth) {
        let data = archive
            .read_fork(&bytes, readme.data_fork.as_ref().unwrap())
            .unwrap();
        assert_eq!(data, expected);
    }
}

#[test]
fn rejects_non_archive() {
    assert_eq!(
        stuffit::parse(b"hello").unwrap_err(),
        stuffit::Error::NotStuffIt5
    );
}
