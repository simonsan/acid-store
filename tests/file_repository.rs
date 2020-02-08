/*
 * Copyright 2019-2020 Wren Powell
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#![cfg(feature = "repo-file")]

use std::fs::{create_dir, File};
use std::io::{Read, Write};
use std::path::Path;

use matches::assert_matches;
use tempfile::tempdir;

use acid_store::repo::{Entry, FileRepository, NoMetadata};
use acid_store::store::MemoryStore;
use common::{assert_contains_all, ARCHIVE_CONFIG, PASSWORD};

mod common;

fn create_repo() -> acid_store::Result<FileRepository<MemoryStore, NoMetadata>> {
    FileRepository::create_repo(MemoryStore::new(), ARCHIVE_CONFIG, Some(PASSWORD))
}

#[test]
fn creating_existing_file_errs() -> anyhow::Result<()> {
    let mut repository = create_repo()?;

    repository.create("home", &Entry::directory())?;
    let result = repository.create("home", &Entry::directory());

    assert_matches!(result.unwrap_err(), acid_store::Error::AlreadyExists);
    Ok(())
}

#[test]
fn creating_file_without_parent_errs() -> anyhow::Result<()> {
    let mut repository = create_repo()?;

    // Creating a directory without a parent fails.
    let result = repository.create("home/lostatc", &Entry::directory());
    assert_matches!(result.unwrap_err(), acid_store::Error::InvalidPath);

    // Creating a directory as a child of a file fails.
    repository.create("home", &Entry::file())?;
    let result = repository.create("home/lostatc", &Entry::directory());
    assert_matches!(result.unwrap_err(), acid_store::Error::InvalidPath);

    Ok(())
}

#[test]
fn create_parents() -> anyhow::Result<()> {
    let mut repository = create_repo()?;

    repository.create_parents("home/lostatc/test", &Entry::file())?;

    assert!(repository.entry("home/lostatc/test")?.is_file());
    assert!(repository.entry("home/lostatc")?.is_directory());
    assert!(repository.entry("home")?.is_directory());

    Ok(())
}

#[test]
fn removing_nonexistent_path_errs() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    let result = repository.remove("home");

    assert_matches!(result, Err(acid_store::Error::NotFound));
    Ok(())
}

#[test]
fn removing_non_empty_directory_errs() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    repository.create_parents("home/lostatc", &Entry::directory())?;
    let result = repository.remove("home");

    assert_matches!(result, Err(acid_store::Error::NotEmpty));
    Ok(())
}

#[test]
fn remove_file() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    repository.create("home", &Entry::directory())?;
    repository.remove("home")?;

    assert!(!repository.exists("home")?);
    Ok(())
}

#[test]
fn remove_tree() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    repository.create_parents("home/lostatc/test", &Entry::file())?;
    repository.remove_tree("home")?;

    assert!(!repository.exists("home")?);
    assert!(!repository.exists("home/lostatc")?);
    assert!(!repository.exists("home/lostatc/test")?);
    Ok(())
}

#[test]
fn setting_metadata_on_nonexistent_file_errs() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    let result = repository.set_metadata("file", NoMetadata);

    assert_matches!(result, Err(acid_store::Error::NotFound));
    Ok(())
}

#[test]
fn opening_non_regular_file_errs() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    repository.create("directory", &Entry::directory())?;
    let result = repository.open("directory");

    assert_matches!(result, Err(acid_store::Error::NotFile));
    Ok(())
}

#[test]
fn opening_nonexistent_file_errs() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    let result = repository.open("nonexistent");

    assert_matches!(result, Err(acid_store::Error::NotFound));
    Ok(())
}

#[test]
fn open_file() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    repository.create("file", &Entry::file())?;
    let mut object = repository.open("file")?;

    object.write_all(b"expected data")?;
    object.flush()?;
    drop(object);

    let mut object = repository.open("file")?;
    let mut actual_data = Vec::new();
    object.read_to_end(&mut actual_data)?;

    assert_eq!(actual_data, b"expected data");
    Ok(())
}

#[test]
fn list_children() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    repository.create_parents("root/child1", &Entry::file())?;
    repository.create_parents("root/child2/descendant", &Entry::file())?;

    let actual = repository.list("root")?;
    let expected = vec![Path::new("root/child1"), Path::new("root/child2")];

    assert_contains_all(actual, expected);
    Ok(())
}

#[test]
fn walk_descendants() -> anyhow::Result<()> {
    let mut repository = create_repo()?;
    repository.create_parents("root/child1", &Entry::file())?;
    repository.create_parents("root/child2/descendant", &Entry::file())?;

    let actual = repository.walk("root")?;
    let expected = vec![
        Path::new("root/child1"),
        Path::new("root/child2"),
        Path::new("root/child2/descendant"),
    ];

    assert_contains_all(actual, expected);
    Ok(())
}

#[test]
fn archive_file() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let source_path = temp_dir.as_ref().join("source");
    let mut source_file = File::create(&source_path)?;
    source_file.write_all(b"file contents")?;
    source_file.flush()?;

    let mut repository = create_repo()?;
    repository.archive(&source_path, "dest")?;

    let mut object = repository.open("dest")?;
    let mut actual_contents = Vec::new();
    object.read_to_end(&mut actual_contents)?;

    assert_eq!(actual_contents, b"file contents");
    Ok(())
}

#[test]
fn archive_tree() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let source_path = temp_dir.as_ref().join("source");

    create_dir(&source_path)?;
    File::create(&source_path.join("file1"))?;
    create_dir(&source_path.join("directory"))?;
    File::create(&source_path.join("directory/file2"))?;

    let mut repository = create_repo()?;
    repository.archive_tree(&source_path, "dest")?;

    assert!(repository.entry("dest")?.is_directory());
    assert!(repository.entry("dest/file1")?.is_file());
    assert!(repository.entry("dest/directory")?.is_directory());
    assert!(repository.entry("dest/directory/file2")?.is_file());
    Ok(())
}

#[test]
fn extract_file() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let dest_path = temp_dir.as_ref().join("dest");

    let mut repository = create_repo()?;
    repository.create("source", &Entry::file())?;
    let mut object = repository.open("source")?;
    object.write_all(b"file contents")?;
    object.flush()?;
    drop(object);
    repository.extract("source", &dest_path)?;

    let mut actual_contents = Vec::new();
    let mut dest_file = File::open(&dest_path)?;
    dest_file.read_to_end(&mut actual_contents)?;

    assert_eq!(actual_contents, b"file contents");
    Ok(())
}

#[test]
fn extract_tree() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let dest_path = temp_dir.as_ref().join("dest");

    let mut repository = create_repo()?;
    repository.create("source", &Entry::directory())?;
    repository.create("source/file1", &Entry::file())?;
    repository.create("source/directory", &Entry::directory())?;
    repository.create("source/directory/file2", &Entry::file())?;

    repository.extract_tree("source", &dest_path)?;

    assert!(dest_path.join("file1").is_file());
    assert!(dest_path.join("directory").is_dir());
    assert!(dest_path.join("directory/file2").is_file());
    Ok(())
}