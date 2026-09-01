// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Direct construction of the minimal Git state used by test fixtures.

use std::fs;
use std::path::{Path, PathBuf};

/// Initialise the repository metadata a fixture needs without invoking Git.
///
/// # Panics
///
/// Panics if the metadata directories or control files cannot be written.
pub fn initialise_repository(root: &Path) {
    let store = root.join(".git");
    fs::create_dir_all(store.join("objects")).expect("fixture object store");
    fs::create_dir_all(store.join("refs/heads")).expect("fixture reference store");
    fs::write(store.join("HEAD"), "ref: refs/heads/main\n").expect("fixture HEAD");
    fs::write(
        store.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n",
    )
    .expect("fixture repository configuration");
}

/// Track every file currently present outside the fixture's metadata store.
///
/// # Panics
///
/// Panics if the fixture tree cannot be read or its index cannot be written.
pub fn track_all(root: &Path) {
    let mut paths = Vec::new();
    collect_files(root, root, &mut paths);
    write_index(root, &paths);
}

/// Track exactly the relative paths supplied by a fixture.
///
/// # Panics
///
/// Panics if a listed file cannot be read or the fixture index cannot be written.
#[allow(dead_code)] // Justified: crate tests use this through the shared file's second inclusion.
pub fn track_paths(root: &Path, paths: &[PathBuf]) {
    write_index(root, paths);
}

fn collect_files(root: &Path, directory: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("fixture directory") {
        let entry = entry.expect("fixture entry");
        if directory == root && entry.file_name() == ".git" {
            continue;
        }

        let path = entry.path();
        let file_type = entry.file_type().expect("fixture file type");
        if file_type.is_dir() {
            collect_files(root, &path, paths);
        } else {
            paths.push(
                path.strip_prefix(root)
                    .expect("path below fixture root")
                    .to_path_buf(),
            );
        }
    }
}

fn write_index(root: &Path, paths: &[PathBuf]) {
    initialise_repository(root);

    let mut entries: Vec<_> = paths.iter().map(|path| (path_bytes(path), path)).collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let mut index = Vec::new();
    index.extend_from_slice(b"DIRC");
    index.extend_from_slice(&2_u32.to_be_bytes());
    index.extend_from_slice(
        &u32::try_from(entries.len())
            .expect("fixture entry count")
            .to_be_bytes(),
    );

    for (display, path) in entries {
        let bytes = fs::read(root.join(path)).expect("tracked fixture file");
        let entry_start = index.len();
        for value in [
            0,
            0,
            0,
            0,
            0,
            0,
            0o100_644,
            0,
            0,
            u32::try_from(bytes.len()).expect("fixture file size"),
        ] {
            index.extend_from_slice(&value.to_be_bytes());
        }
        index.extend_from_slice(&blob_id(&bytes));
        let flags = u16::try_from(display.len().min(0x0fff)).expect("fixture path length");
        index.extend_from_slice(&flags.to_be_bytes());
        index.extend_from_slice(&display);
        index.push(0);
        while !(index.len() - entry_start).is_multiple_of(8) {
            index.push(0);
        }
    }

    let checksum = sha1(&index);
    index.extend_from_slice(&checksum);
    fs::write(root.join(".git/index"), index).expect("fixture index");
}

fn blob_id(bytes: &[u8]) -> [u8; 20] {
    let mut object = format!("blob {}\0", bytes.len()).into_bytes();
    object.extend_from_slice(bytes);
    sha1(&object)
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt as _;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().replace('\\', "/").into_bytes()
}

fn sha1(bytes: &[u8]) -> [u8; 20] {
    let bit_length = u64::try_from(bytes.len()).expect("fixture byte length") * 8;
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = [
        0x6745_2301_u32,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    for block in message.as_chunks::<64>().0 {
        let mut schedule = [0_u32; 80];
        for (slot, word) in schedule[..16].iter_mut().zip(block.as_chunks::<4>().0) {
            *slot = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..80 {
            schedule[index] = (schedule[index - 3]
                ^ schedule[index - 8]
                ^ schedule[index - 14]
                ^ schedule[index - 16])
                .rotate_left(1);
        }

        let [mut hash_a, mut hash_b, mut hash_c, mut hash_d, mut hash_e] = state;
        for (index, word) in schedule.into_iter().enumerate() {
            let (choice, constant) = match index {
                0..=19 => ((hash_b & hash_c) | ((!hash_b) & hash_d), 0x5a82_7999),
                20..=39 => (hash_b ^ hash_c ^ hash_d, 0x6ed9_eba1),
                40..=59 => (
                    (hash_b & hash_c) | (hash_b & hash_d) | (hash_c & hash_d),
                    0x8f1b_bcdc,
                ),
                _ => (hash_b ^ hash_c ^ hash_d, 0xca62_c1d6),
            };
            let next = hash_a
                .rotate_left(5)
                .wrapping_add(choice)
                .wrapping_add(hash_e)
                .wrapping_add(constant)
                .wrapping_add(word);
            hash_e = hash_d;
            hash_d = hash_c;
            hash_c = hash_b.rotate_left(30);
            hash_b = hash_a;
            hash_a = next;
        }

        for (slot, value) in state
            .iter_mut()
            .zip([hash_a, hash_b, hash_c, hash_d, hash_e])
        {
            *slot = slot.wrapping_add(value);
        }
    }

    let mut digest = [0_u8; 20];
    for (chunk, value) in digest.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        chunk.copy_from_slice(&value.to_be_bytes());
    }
    digest
}
