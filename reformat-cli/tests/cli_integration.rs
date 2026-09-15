//! Integration tests for CLI functionality

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn get_binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_reformat"))
}

#[test]
fn test_cli_version() {
    let output = Command::new(get_binary_path())
        .arg("--version")
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_help() {
    let output = Command::new(get_binary_path())
        .arg("--help")
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("reformat"));
    assert!(stdout.contains("convert"));
    assert!(stdout.contains("clean"));
    assert!(stdout.contains("emojis"));
}

#[test]
fn test_cli_basic_conversion() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "myVariable = 'test'").unwrap();

    let output = Command::new(get_binary_path())
        .args(["convert", "--from-camel", "--to-snake"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("my_variable"));
}

#[test]
fn test_cli_dry_run() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    let original = "myVariable = 'test'";
    fs::write(&test_file, original).unwrap();

    let output = Command::new(get_binary_path())
        .args(["convert", "--from-camel", "--to-snake", "--dry-run"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    // File should be unchanged
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, original);

    // Output should indicate what would be converted
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would convert"));
}

#[test]
fn test_cli_recursive() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();

    let file1 = test_dir.join("file1.py");
    let file2 = sub_dir.join("file2.py");

    fs::write(&file1, "topLevel = 1").unwrap();
    fs::write(&file2, "nestedVar = 2").unwrap();

    let output = Command::new(get_binary_path())
        .args(["convert", "--from-camel", "--to-snake", "-r"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content1 = fs::read_to_string(&file1).unwrap();
    let content2 = fs::read_to_string(&file2).unwrap();

    assert!(content1.contains("top_level"));
    assert!(content2.contains("nested_var"));
}

#[test]
fn test_cli_with_prefix() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "myVariable = 'test'").unwrap();

    let output = Command::new(get_binary_path())
        .args(["convert", "--from-camel", "--to-snake", "--prefix", "old_"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("old_my_variable"));
}

#[test]
fn test_cli_with_suffix() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "myVariable = 'test'").unwrap();

    let output = Command::new(get_binary_path())
        .args(["convert", "--from-camel", "--to-snake", "--suffix", "_new"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("my_variable_new"));
}

#[test]
fn test_cli_word_filter() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "getUserName = 'alice'\nmyVariable = 123").unwrap();

    let output = Command::new(get_binary_path())
        .args([
            "convert",
            "--from-camel",
            "--to-snake",
            "--word-filter",
            "^get.*",
        ])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("get_user_name"));
    assert!(content.contains("myVariable")); // Should not be converted
}

#[test]
fn test_cli_multiple_extensions() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let py_file = test_dir.join("test.py");
    let js_file = test_dir.join("test.js");
    let txt_file = test_dir.join("test.txt");

    fs::write(&py_file, "myVariable = 1").unwrap();
    fs::write(&js_file, "myVariable = 2").unwrap();
    fs::write(&txt_file, "myVariable = 3").unwrap();

    let output = Command::new(get_binary_path())
        .args([
            "convert",
            "--from-camel",
            "--to-snake",
            "-e",
            ".py",
            "-e",
            ".js",
        ])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let py_content = fs::read_to_string(&py_file).unwrap();
    let js_content = fs::read_to_string(&js_file).unwrap();
    let txt_content = fs::read_to_string(&txt_file).unwrap();

    assert!(py_content.contains("my_variable"));
    assert!(js_content.contains("my_variable"));
    assert!(txt_content.contains("myVariable")); // Should not be converted
}

#[test]
fn test_cli_error_missing_from() {
    let output = Command::new(get_binary_path())
        .args(["convert", "--to-snake", "dummy.py"])
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("required") || stderr.contains("from"));
}

#[test]
fn test_cli_error_missing_to() {
    let output = Command::new(get_binary_path())
        .args(["convert", "--from-camel", "dummy.py"])
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("required") || stderr.contains("to"));
}

#[test]
fn test_cli_error_conflicting_from() {
    let output = Command::new(get_binary_path())
        .args([
            "convert",
            "--from-camel",
            "--from-snake",
            "--to-kebab",
            "dummy.py",
        ])
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be used with"));
}

#[test]
fn test_cli_all_format_combinations() {
    let test_cases = [
        ("--from-camel", "--to-pascal", "myName", "MyName"),
        ("--from-pascal", "--to-snake", "MyName", "my_name"),
        ("--from-snake", "--to-kebab", "my_name", "my-name"),
        ("--from-kebab", "--to-screaming-snake", "my-name", "MY_NAME"),
        ("--from-screaming-snake", "--to-camel", "MY_NAME", "myName"),
    ];

    for (from_arg, to_arg, input, expected) in test_cases.iter() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        fs::write(&test_file, input).unwrap();

        let output = Command::new(get_binary_path())
            .args(["convert", from_arg, to_arg, "-e", ".txt"])
            .arg(&test_file)
            .output()
            .expect("Failed to execute reformat");

        assert!(
            output.status.success(),
            "Failed for {} -> {}",
            from_arg,
            to_arg
        );

        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, *expected, "Failed for {} -> {}", from_arg, to_arg);
    }
}

// Whitespace cleaning tests

#[test]
fn test_cli_clean_basic() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "line1   \nline2\t\nline3\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["clean"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "line1\nline2\nline3\n");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cleaned"));
}

#[test]
fn test_cli_clean_dry_run() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.txt");
    let original = "line1   \nline2\t\nline3\n";
    fs::write(&test_file, original).unwrap();

    let output = Command::new(get_binary_path())
        .args(["clean", "--dry-run"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    // File should be unchanged
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, original);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[DRY-RUN]") || stdout.contains("Would clean"));
}

#[test]
fn test_cli_clean_recursive() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();

    let file1 = test_dir.join("file1.txt");
    let file2 = sub_dir.join("file2.txt");

    fs::write(&file1, "line1   \n").unwrap();
    fs::write(&file2, "line2\t\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["clean", "-r"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    let content1 = fs::read_to_string(&file1).unwrap();
    let content2 = fs::read_to_string(&file2).unwrap();

    assert_eq!(content1, "line1\n");
    assert_eq!(content2, "line2\n");
}

#[test]
fn test_cli_clean_extension_filtering() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let py_file = test_dir.join("test.py");
    let txt_file = test_dir.join("test.txt");

    fs::write(&py_file, "line1   \n").unwrap();
    fs::write(&txt_file, "line1   \n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["clean", "-e", ".py"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    let py_content = fs::read_to_string(&py_file).unwrap();
    let txt_content = fs::read_to_string(&txt_file).unwrap();

    assert_eq!(py_content, "line1\n"); // Should be cleaned
    assert_eq!(txt_content, "line1   \n"); // Should not be cleaned
}

#[test]
fn test_cli_clean_no_changes_needed() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "line1\nline2\nline3\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["clean"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No files needed cleaning"));
}

#[test]
fn test_cli_clean_help() {
    let output = Command::new(get_binary_path())
        .args(["clean", "--help"])
        .output()
        .expect("Failed to execute reformat clean --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Remove trailing whitespace"));
}

#[test]
fn test_cli_convert_subcommand() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "myVariable = 'test'").unwrap();

    let output = Command::new(get_binary_path())
        .args(["convert", "--from-camel", "--to-snake"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat convert");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("my_variable"));
}

// Rename tests

#[test]
fn test_cli_rename_lowercase() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("TestFile.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--to-lowercase"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());

    // Check that the file exists with the new name
    let new_file = test_dir.join("testfile.txt");
    assert!(new_file.exists());

    // Verify content is preserved
    let content = fs::read_to_string(&new_file).unwrap();
    assert_eq!(content, "content");
}

#[test]
fn test_cli_rename_uppercase() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("testfile.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--to-uppercase"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());

    // Check that the file exists with the new name
    let new_file = test_dir.join("TESTFILE.txt");
    assert!(new_file.exists());

    // Verify content is preserved
    let content = fs::read_to_string(&new_file).unwrap();
    assert_eq!(content, "content");
}

#[test]
fn test_cli_rename_capitalize() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("testFile.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--to-capitalize"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());

    // Check that the file exists with the new name
    let new_file = test_dir.join("Testfile.txt");
    assert!(new_file.exists());

    // Verify content is preserved
    let content = fs::read_to_string(&new_file).unwrap();
    assert_eq!(content, "content");
}

#[test]
fn test_cli_rename_to_underscore() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--underscored"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("test_file.txt").exists());
    assert!(!test_file.exists());
}

#[test]
fn test_cli_rename_to_hyphen() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--hyphenated"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("test-file.txt").exists());
    assert!(!test_file.exists());
}

#[test]
fn test_cli_rename_add_prefix() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--add-prefix", "new_"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("new_file.txt").exists());
    assert!(!test_file.exists());
}

#[test]
fn test_cli_rename_rm_prefix() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("old_file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--rm-prefix", "old_"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("file.txt").exists());
    assert!(!test_file.exists());
}

#[test]
fn test_cli_rename_add_suffix() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--add-suffix", "_backup"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("file_backup.txt").exists());
    assert!(!test_file.exists());
}

#[test]
fn test_cli_rename_rm_suffix() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("file_old.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--rm-suffix", "_old"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("file.txt").exists());
    assert!(!test_file.exists());
}

#[test]
fn test_cli_rename_combined() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("old_Test File.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args([
            "rename_files",
            "--rm-prefix",
            "old_",
            "--underscored",
            "--to-lowercase",
            "--add-suffix",
            "_new",
        ])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("test_file_new.txt").exists());
    assert!(!test_file.exists());
}

#[test]
fn test_cli_rename_dry_run() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("TestFile.txt");
    let original_content = "content";
    fs::write(&test_file, original_content).unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--to-lowercase", "--dry-run"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());

    // File should still exist and be unchanged
    assert!(test_file.exists());
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, original_content);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[DRY-RUN]") || stdout.contains("Would rename"));
}

#[test]
fn test_cli_rename_recursive() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();

    let file1 = test_dir.join("File1.txt");
    let file2 = sub_dir.join("File2.txt");

    fs::write(&file1, "content1").unwrap();
    fs::write(&file2, "content2").unwrap();

    let output = Command::new(get_binary_path())
        .args(["rename_files", "--to-lowercase", "-r"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("file1.txt").exists());
    assert!(sub_dir.join("file2.txt").exists());
}

#[test]
fn test_cli_rename_help() {
    let output = Command::new(get_binary_path())
        .args(["rename_files", "--help"])
        .output()
        .expect("Failed to execute reformat rename --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Rename files"));
}

// Combined default command tests
#[test]
fn test_cli_combined_default() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    // Create a file with uppercase name, emojis, and trailing whitespace
    let test_file = test_dir.join("TestFile.txt");
    fs::write(&test_file, "Line 1   \nTask ✅ done\nLine 3\t\n").unwrap();

    let output = Command::new(get_binary_path())
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat (default command)");

    assert!(output.status.success());

    // Names are left alone: lowercasing Cargo.toml or Button.tsx broke builds.
    let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "Should have exactly one file");
    let actual_name = entries[0].as_ref().unwrap().file_name();
    assert_eq!(
        actual_name.to_str().unwrap(),
        "TestFile.txt",
        "the default command must not rename files"
    );
    // Check content transformations
    let content = fs::read_to_string(&test_file).unwrap();

    // Emoji should be transformed
    assert!(content.contains("[x]"), "Emoji should be replaced with [x]");
    assert!(
        !content.contains("✅"),
        "Original emoji should not be present"
    );

    // Whitespace should be cleaned
    assert!(
        !content.contains("   \n"),
        "Trailing spaces should be removed"
    );
    assert!(!content.contains("\t\n"), "Trailing tabs should be removed");
}

#[test]
fn test_cli_combined_recursive() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();

    let file1 = test_dir.join("File1.txt");
    let file2 = sub_dir.join("File2.md");

    fs::write(&file1, "Text   \n✅ Done\n").unwrap();
    fs::write(&file2, "More text\t\n☐ Todo\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["-r"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat -r");

    assert!(output.status.success());

    // Names are left alone. Compare actual names: on a case-insensitive
    // filesystem `file1.txt` would also resolve to `File1.txt`.
    let name_in = |dir: &std::path::Path| {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(name_in(&test_dir), ["File1.txt"]);
    assert_eq!(name_in(&sub_dir), ["File2.md"]);

    // Check content transformations for file1
    let content1 = fs::read_to_string(&file1).unwrap();
    assert!(content1.contains("[x]"));
    assert!(!content1.contains("✅"));
    assert!(!content1.contains("   \n"));

    // Check content transformations for file2
    let content2 = fs::read_to_string(&file2).unwrap();
    assert!(content2.contains("[ ]"));
    assert!(!content2.contains("☐"));
    assert!(!content2.contains("\t\n"));
}

#[test]
fn test_cli_combined_dry_run() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("TestFile.txt");
    let original_content = "Line 1   \nTask ✅\n";
    fs::write(&test_file, original_content).unwrap();

    let output = Command::new(get_binary_path())
        .args(["--dry-run"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat --dry-run");

    assert!(output.status.success());

    // File should remain unchanged
    assert!(test_file.exists());
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, original_content);

    // Output should indicate dry-run mode
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[DRY-RUN]") || stdout.contains("Would"));
}

#[test]
fn test_cli_combined_no_changes_needed() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    // Create a file that already meets all criteria
    let test_file = test_dir.join("testfile.txt");
    fs::write(&test_file, "Line 1\nLine 2\n").unwrap();

    let output = Command::new(get_binary_path())
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat (default command)");

    assert!(output.status.success());

    // File should still exist with same content
    assert!(test_file.exists());
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "Line 1\nLine 2\n");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No files needed processing"));
}

// =============================================================================
// Preset tests
// =============================================================================

#[test]
fn test_preset_clean_step() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    // Create a reformat.json in the test dir
    fs::write(
        test_dir.join("reformat.json"),
        r#"{"tidy": {"steps": ["clean"]}}"#,
    )
    .unwrap();

    // Create a file with trailing whitespace
    let test_file = test_dir.join("code.py");
    fs::write(&test_file, "hello   \nworld\t\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["-p", "tidy"])
        .arg(&test_dir)
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "hello\nworld\n");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("clean:"));
    assert!(stdout.contains("Pipeline 'tidy' complete."));
}

#[test]
fn test_preset_rename_step() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(
        test_dir.join("reformat.json"),
        r#"{"lower": {"steps": ["rename"], "rename": {"case_transform": "lowercase"}}}"#,
    )
    .unwrap();

    let test_file = test_dir.join("MyFile.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(["-p", "lower"])
        .arg(&test_dir)
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // On case-insensitive filesystems, check actual filename
    let entries: Vec<_> = fs::read_dir(&test_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".txt")))
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file_name().to_str().unwrap(), "myfile.txt");
}

#[test]
fn test_preset_multi_step() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(
        test_dir.join("reformat.json"),
        r#"{"all": {"steps": ["rename", "clean"], "rename": {"case_transform": "lowercase"}}}"#,
    )
    .unwrap();

    let test_file = test_dir.join("MyCode.py");
    fs::write(&test_file, "x = 1   \ny = 2\t\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["-p", "all"])
        .arg(&test_dir)
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // File should be renamed and cleaned
    let renamed = test_dir.join("mycode.py");
    assert!(renamed.exists() || test_dir.join("MyCode.py").exists()); // case-insensitive FS
    let entries: Vec<_> = fs::read_dir(&test_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".py")))
        .collect();
    assert_eq!(entries.len(), 1);
    let actual_name = entries[0].file_name();
    assert_eq!(actual_name.to_str().unwrap(), "mycode.py");

    let content = fs::read_to_string(entries[0].path()).unwrap();
    assert_eq!(content, "x = 1\ny = 2\n");
}

#[test]
fn test_preset_dry_run_override() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(
        test_dir.join("reformat.json"),
        r#"{"tidy": {"steps": ["clean"]}}"#,
    )
    .unwrap();

    let test_file = test_dir.join("code.rs");
    let original = "let x = 1;   \n";
    fs::write(&test_file, original).unwrap();

    let output = Command::new(get_binary_path())
        .args(["-p", "tidy", "--dry-run"])
        .arg(&test_dir)
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // File should be unchanged in dry-run
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, original);
}

#[test]
fn test_preset_missing_config_file() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    // No reformat.json created
    let output = Command::new(get_binary_path())
        .args(["-p", "whatever"])
        .arg(&test_dir)
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reformat.json not found"));
}

#[test]
fn test_preset_unknown_preset_name() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(
        test_dir.join("reformat.json"),
        r#"{"code": {"steps": ["clean"]}}"#,
    )
    .unwrap();

    let output = Command::new(get_binary_path())
        .args(["-p", "nonexistent"])
        .arg(&test_dir)
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("preset 'nonexistent' not found"));
}

// ==================== Job tests ====================

#[test]
fn test_job_from_file() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(&job_file, r#"{"steps": ["clean"]}"#).unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "hello   \nworld  \n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["--job"])
        .arg(&job_file)
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "hello\nworld\n");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Pipeline"));
    assert!(stdout.contains("complete."));
}

#[test]
fn test_job_from_stdin() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "hello   \n").unwrap();

    let mut child = Command::new(get_binary_path())
        .args(["--job", "-"])
        .arg(&test_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn reformat");

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        stdin.write_all(b"{\"steps\": [\"clean\"]}").unwrap();
    }

    let output = child.wait_with_output().expect("Failed to wait on child");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "hello\n");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Pipeline 'stdin' complete."));
}

#[test]
fn test_job_multi_step() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(
        &job_file,
        r#"{
            "steps": ["replace", "clean"],
            "replace": {
                "patterns": [
                    {"find": "foo", "replace": "bar"}
                ]
            }
        }"#,
    )
    .unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "foo   \nfoo baz  \n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["--job"])
        .arg(&job_file)
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "bar\nbar baz\n");
}

#[test]
fn test_job_dry_run() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(&job_file, r#"{"steps": ["clean"]}"#).unwrap();

    let test_file = test_dir.join("test.txt");
    let original = "hello   \n";
    fs::write(&test_file, original).unwrap();

    let output = Command::new(get_binary_path())
        .args(["--job"])
        .arg(&job_file)
        .arg("--dry-run")
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // File should be unchanged
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, original);
}

#[test]
fn test_job_missing_file() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let output = Command::new(get_binary_path())
        .args(["--job", "/nonexistent/job.json"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to read job file"));
}

#[test]
fn test_job_invalid_json() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(&job_file, "not valid json {{{").unwrap();

    let output = Command::new(get_binary_path())
        .args(["--job"])
        .arg(&job_file)
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse job"));
}

#[test]
fn test_job_unknown_step() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(&job_file, r#"{"steps": ["clean", "bogus"]}"#).unwrap();

    let output = Command::new(get_binary_path())
        .args(["--job"])
        .arg(&job_file)
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown step 'bogus'"));
}

#[test]
fn test_job_conflicts_with_preset() {
    let output = Command::new(get_binary_path())
        .args(["--job", "job.json", "-p", "code", "."])
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "stderr should mention conflict: {}",
        stderr
    );
}

/// The preset path used to hardcode `None` for convert's strip/replace affix
/// settings, so these keys were silently unreachable from a preset even
/// though `ConvertConfig` defined them. Both paths now build the same config.
#[test]
fn test_preset_convert_supports_affix_options() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(test_dir.join("code.py"), "m_userName = 1\n").unwrap();
    fs::write(
        test_dir.join("reformat.json"),
        r#"{
            "strip": {
                "steps": ["convert"],
                "convert": {
                    "from_format": "camel",
                    "to_format": "snake",
                    "file_extensions": [".py"],
                    "strip_prefix": "m_"
                }
            }
        }"#,
    )
    .unwrap();

    let output = Command::new(get_binary_path())
        .args(["-p", "strip", "."])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to run preset");

    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        fs::read_to_string(test_dir.join("code.py")).unwrap(),
        "user_name = 1\n",
        "convert.strip_prefix had no effect from a preset"
    );
}

/// Acronyms survive conversion end to end.
#[test]
fn test_cli_convert_preserves_acronyms() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(test_dir.join("a.py"), "parseHTTPResponse()\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["convert", "--from-camel", "--to-snake", "-r", "."])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to run convert");

    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(test_dir.join("a.py")).unwrap(),
        "parse_http_response()\n"
    );
}

/// --quiet must silence the per-file reporting that used to be printed
/// straight from the library.
#[test]
fn test_cli_quiet_suppresses_output() {
    // A unique directory per test: these run in parallel, and a shared
    // fixture path lets them clobber each other. TempDir also cleans up
    // when a test panics, which explicit teardown at the end does not.
    let _tmp = tempfile::tempdir().unwrap();
    let test_dir = _tmp.path().to_path_buf();
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(test_dir.join("a.txt"), "line  \n").unwrap();

    let output = Command::new(get_binary_path())
        .args(["-q", "clean", "-r", "."])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to run clean");

    assert!(output.status.success());
    assert!(
        output.stdout.is_empty(),
        "--quiet still produced output: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    // The work itself still happened.
    assert_eq!(
        fs::read_to_string(test_dir.join("a.txt")).unwrap(),
        "line\n"
    );
}

/// A path that does not exist is an error, not a silent success.
#[test]
fn test_cli_missing_path_is_an_error() {
    for args in [
        vec!["clean", "/nonexistent/reformat/zzz"],
        vec![
            "convert",
            "--from-camel",
            "--to-snake",
            "/nonexistent/reformat/zzz",
        ],
    ] {
        let output = Command::new(get_binary_path())
            .args(&args)
            .output()
            .expect("Failed to run");
        assert!(
            !output.status.success(),
            "{:?} exited 0 on a missing path",
            args
        );
    }
}

// ---------------------------------------------------------------------------
// Subcommands that previously had no CLI coverage at all: group, endings,
// indent and header. `group` is the most destructive of the nine -- it moves
// files and rewrites references -- and had none.
// ---------------------------------------------------------------------------

fn fixture() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("reformat-cli-")
        .tempdir()
        .unwrap()
}

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(get_binary_path())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run reformat")
}

#[test]
fn test_cli_group_moves_files_into_prefix_directories() {
    let tmp = fixture();
    let dir = tmp.path();
    for name in ["wbs_create.tmpl", "wbs_delete.tmpl", "other.tmpl"] {
        fs::write(dir.join(name), "x").unwrap();
    }

    let output = run(dir, &["group", "--no-interactive", "."]);
    assert!(output.status.success(), "{:?}", output);

    assert!(dir.join("wbs/wbs_create.tmpl").exists());
    assert!(dir.join("wbs/wbs_delete.tmpl").exists());
    assert!(dir.join("other.tmpl").exists(), "ungrouped file was moved");
    assert!(dir.join("changes.json").exists());
}

#[test]
fn test_cli_group_strip_prefix_and_preview() {
    let tmp = fixture();
    let dir = tmp.path();
    for name in ["wbs_create.tmpl", "wbs_delete.tmpl"] {
        fs::write(dir.join(name), "x").unwrap();
    }

    // --preview must not touch anything.
    let output = run(dir, &["group", "--preview", "."]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("wbs"));
    assert!(dir.join("wbs_create.tmpl").exists(), "preview moved files");

    let output = run(dir, &["group", "--no-interactive", "--strip-prefix", "."]);
    assert!(output.status.success(), "{:?}", output);
    assert!(dir.join("wbs/create.tmpl").exists());
    assert!(dir.join("wbs/delete.tmpl").exists());
}

#[test]
fn test_cli_group_dry_run_writes_nothing() {
    let tmp = fixture();
    let dir = tmp.path();
    for name in ["a_one.txt", "a_two.txt"] {
        fs::write(dir.join(name), "x").unwrap();
    }

    let output = run(dir, &["group", "-d", "--no-interactive", "."]);
    assert!(output.status.success());
    assert!(dir.join("a_one.txt").exists(), "dry run moved a file");
    assert!(
        !dir.join("changes.json").exists(),
        "dry run left a changes.json describing moves that never happened"
    );
}

#[test]
fn test_cli_endings_normalizes_and_preserves_content() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::write(dir.join("a.txt"), "one\r\ntwo\r\n").unwrap();

    assert!(run(dir, &["endings", "--style", "lf", "."])
        .status
        .success());
    assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one\ntwo\n");

    assert!(run(dir, &["endings", "--style", "crlf", "."])
        .status
        .success());
    assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one\r\ntwo\r\n");
}

#[test]
fn test_cli_endings_rejects_unknown_style() {
    let tmp = fixture();
    let output = run(tmp.path(), &["endings", "--style", "nonsense", "."]);
    assert!(!output.status.success(), "an unknown style should fail");
}

#[test]
fn test_cli_indent_converts_tabs_and_spaces() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::write(dir.join("a.py"), "\tif x:\n\t\tpass\n").unwrap();

    assert!(
        run(dir, &["indent", "--style", "spaces", "--width", "4", "."])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(dir.join("a.py")).unwrap(),
        "    if x:\n        pass\n"
    );

    assert!(
        run(dir, &["indent", "--style", "tabs", "--width", "4", "."])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(dir.join("a.py")).unwrap(),
        "\tif x:\n\t\tpass\n"
    );
}

#[test]
fn test_cli_indent_rejects_unknown_style() {
    let tmp = fixture();
    let output = run(tmp.path(), &["indent", "--style", "nonsense", "."]);
    assert!(!output.status.success(), "an unknown style should fail");
}

#[test]
fn test_cli_header_inserts_then_updates_in_place() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::write(dir.join("a.rs"), "fn main() {}\n").unwrap();

    assert!(run(dir, &["header", "-t", "// (c) 2020 Acme", "."])
        .status
        .success());
    assert_eq!(
        fs::read_to_string(dir.join("a.rs")).unwrap(),
        "// (c) 2020 Acme\n\nfn main() {}\n"
    );

    // A different year must replace the header, not stack a second one on top.
    assert!(run(dir, &["header", "-t", "// (c) 2026 Acme", "."])
        .status
        .success());
    let content = fs::read_to_string(dir.join("a.rs")).unwrap();
    assert_eq!(content, "// (c) 2026 Acme\n\nfn main() {}\n");
    assert_eq!(content.matches("Acme").count(), 1);
}

#[test]
fn test_cli_header_preserves_shebang() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::write(dir.join("a.py"), "#!/usr/bin/env python\nprint(1)\n").unwrap();

    assert!(run(dir, &["header", "-t", "# (c) Acme", "-e", ".py", "."])
        .status
        .success());
    let content = fs::read_to_string(dir.join("a.py")).unwrap();
    assert!(content.starts_with("#!/usr/bin/env python\n"));
    assert!(content.contains("# (c) Acme"));
}

// ---------------------------------------------------------------------------
// Regressions for defects found in the 2026-09 review (REVIEW.md, section 1).
// ---------------------------------------------------------------------------

/// D1: the default command lowercased every file name, breaking builds.
#[test]
fn test_cli_default_command_leaves_build_files_alone() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::write(dir.join("Cargo.toml"), "[package]  \n").unwrap();
    fs::write(dir.join("Makefile"), "all:\n").unwrap();

    assert!(run(dir, &["-r", "."]).status.success());

    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Cargo.toml", "Makefile"]);
}

/// D2: `recursive` defaulted to true on a plain flag, so it could not be
/// turned off.
#[test]
fn test_cli_no_recursive_limits_to_top_level() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::create_dir(dir.join("sub")).unwrap();
    fs::write(dir.join("a.txt"), "a  \n").unwrap();
    fs::write(dir.join("sub/b.txt"), "b  \n").unwrap();

    assert!(run(dir, &["clean", "--no-recursive", "."]).status.success());
    assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "a\n");
    assert_eq!(fs::read_to_string(dir.join("sub/b.txt")).unwrap(), "b  \n");

    // The last of -r and --no-recursive wins.
    assert!(run(dir, &["clean", "--no-recursive", "-r", "."])
        .status
        .success());
    assert_eq!(fs::read_to_string(dir.join("sub/b.txt")).unwrap(), "b\n");
}

/// D3: the emoji booleans could not be disabled, and the README example
/// `--no-remove-other` was rejected.
#[test]
fn test_cli_emoji_flags_can_be_disabled() {
    let tmp = fixture();
    let dir = tmp.path();
    let text = "done \u{2705} launch \u{1F680}\n";
    fs::write(dir.join("a.md"), text).unwrap();

    let out = run(
        dir,
        &["emojis", "--replace-task", "--no-remove-other", "a.md"],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        fs::read_to_string(dir.join("a.md")).unwrap(),
        "done [x] launch \u{1F680}\n"
    );

    // Task emojis are kept even though their code points fall inside the
    // ranges --remove-other deletes.
    fs::write(dir.join("b.md"), text).unwrap();
    assert!(run(dir, &["emojis", "--no-replace-task", "b.md"])
        .status
        .success());
    assert_eq!(
        fs::read_to_string(dir.join("b.md")).unwrap(),
        "done \u{2705} launch \n"
    );
}

/// D4: `-e py` without the dot matched nothing and exited 0.
#[test]
fn test_cli_extension_without_dot() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::write(dir.join("a.py"), "x  \n").unwrap();

    assert!(run(dir, &["clean", "-e", "py", "."]).status.success());
    assert_eq!(fs::read_to_string(dir.join("a.py")).unwrap(), "x\n");
}

/// D5: `convert` logged per-file errors and exited 0; `clean` stopped at
/// the first one. Both now finish the run and exit 2.
#[cfg(unix)]
#[test]
fn test_cli_unreadable_file_fails_after_processing_the_rest() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = fixture();
    let dir = tmp.path();
    fs::write(dir.join("a.py"), "some_name = 1  \n").unwrap();
    fs::write(dir.join("b.py"), "other_name = 2  \n").unwrap();
    fs::set_permissions(dir.join("a.py"), fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(dir.join("a.py")).is_ok() {
        eprintln!("skipping: running with permission to read mode-000 files");
        return;
    }

    let convert = run(dir, &["convert", "--from-snake", "--to-camel", "."]);
    assert_eq!(convert.status.code(), Some(2), "{:?}", convert);
    assert!(fs::read_to_string(dir.join("b.py"))
        .unwrap()
        .contains("otherName"));

    let clean = run(dir, &["clean", "."]);
    assert_eq!(clean.status.code(), Some(2), "{:?}", clean);
    assert_eq!(
        fs::read_to_string(dir.join("b.py")).unwrap(),
        "otherName = 2\n"
    );

    fs::set_permissions(dir.join("a.py"), fs::Permissions::from_mode(0o644)).unwrap();
}

/// D7: a group step in a pipeline discarded its change record.
#[test]
fn test_cli_group_step_in_job_writes_changes_file() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::create_dir(dir.join("t")).unwrap();
    for name in ["x_1.txt", "x_2.txt"] {
        fs::write(dir.join("t").join(name), "x").unwrap();
    }
    fs::write(dir.join("job.json"), r#"{"steps": ["group"]}"#).unwrap();

    let out = run(dir, &["--job", "job.json", "t"]);
    assert!(out.status.success(), "{:?}", out);
    assert!(dir.join("t/x/x_1.txt").exists());
    let record = fs::read_to_string(dir.join("changes.json")).unwrap();
    assert!(record.contains("x_1.txt"), "{}", record);
}

/// D8: `group` suggested applying fixes later, but no command could.
#[test]
fn test_cli_apply_fixes_from_file() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::create_dir_all(dir.join("t")).unwrap();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("t/wbs_a.tmpl"), "x").unwrap();
    fs::write(dir.join("t/wbs_b.tmpl"), "x").unwrap();
    fs::write(dir.join("src/main.go"), "load(\"wbs_a.tmpl\")\n").unwrap();

    let out = run(
        dir,
        &[
            "group",
            "--no-interactive",
            "--strip-prefix",
            "--scope",
            "src",
            "t",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    assert!(dir.join("fixes.json").exists());

    let dry = run(dir, &["apply_fixes", "--dry-run", "fixes.json"]);
    assert!(dry.status.success());
    assert_eq!(
        fs::read_to_string(dir.join("src/main.go")).unwrap(),
        "load(\"wbs_a.tmpl\")\n"
    );

    assert!(run(dir, &["apply_fixes", "fixes.json"]).status.success());
    assert_eq!(
        fs::read_to_string(dir.join("src/main.go")).unwrap(),
        "load(\"wbs/a.tmpl\")\n"
    );
}

/// D10: `group --preview -r` ignored `-r`.
#[test]
fn test_cli_group_preview_recursive() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::create_dir(dir.join("sub")).unwrap();
    fs::write(dir.join("sub/deep_1.txt"), "x").unwrap();
    fs::write(dir.join("sub/deep_2.txt"), "x").unwrap();

    let flat = run(dir, &["group", "--preview", "."]);
    assert!(!String::from_utf8_lossy(&flat.stdout).contains("deep"));

    let deep = run(dir, &["group", "--preview", "-r", "."]);
    assert!(deep.status.success());
    assert!(String::from_utf8_lossy(&deep.stdout).contains("deep (2 files)"));
    assert!(dir.join("sub/deep_1.txt").exists(), "preview moved files");
}

/// Half of a replace-prefix pair used to be accepted and silently ignored.
#[test]
fn test_cli_replace_prefix_halves_are_required_together() {
    let tmp = fixture();
    let dir = tmp.path();
    fs::write(dir.join("a.py"), "IUserService = 1\n").unwrap();

    let out = run(
        dir,
        &[
            "convert",
            "--from-pascal",
            "--to-snake",
            "--replace-prefix-from",
            "I",
            "a.py",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--replace-prefix-to"));
    assert_eq!(
        fs::read_to_string(dir.join("a.py")).unwrap(),
        "IUserService = 1\n"
    );
}
