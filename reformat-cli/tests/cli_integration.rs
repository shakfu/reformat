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
    let test_dir = std::env::temp_dir().join("reformat_test_cli_basic");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "myVariable = 'test'").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["convert", "--from-camel", "--to-snake"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("my_variable"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_dry_run() {
    let test_dir = std::env::temp_dir().join("reformat_test_cli_dry");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    let original = "myVariable = 'test'";
    fs::write(&test_file, original).unwrap();

    let output = Command::new(get_binary_path())
        .args(&["convert", "--from-camel", "--to-snake", "--dry-run"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_recursive() {
    let test_dir = std::env::temp_dir().join("reformat_test_cli_recursive");
    fs::create_dir_all(&test_dir).unwrap();

    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();

    let file1 = test_dir.join("file1.py");
    let file2 = sub_dir.join("file2.py");

    fs::write(&file1, "topLevel = 1").unwrap();
    fs::write(&file2, "nestedVar = 2").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["convert", "--from-camel", "--to-snake", "-r"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content1 = fs::read_to_string(&file1).unwrap();
    let content2 = fs::read_to_string(&file2).unwrap();

    assert!(content1.contains("top_level"));
    assert!(content2.contains("nested_var"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_with_prefix() {
    let test_dir = std::env::temp_dir().join("reformat_test_cli_prefix");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "myVariable = 'test'").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["convert", "--from-camel", "--to-snake", "--prefix", "old_"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("old_my_variable"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_with_suffix() {
    let test_dir = std::env::temp_dir().join("reformat_test_cli_suffix");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "myVariable = 'test'").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["convert", "--from-camel", "--to-snake", "--suffix", "_new"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("my_variable_new"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_word_filter() {
    let test_dir = std::env::temp_dir().join("reformat_test_cli_filter");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "getUserName = 'alice'\nmyVariable = 123").unwrap();

    let output = Command::new(get_binary_path())
        .args(&[
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_multiple_extensions() {
    let test_dir = std::env::temp_dir().join("reformat_test_cli_exts");
    fs::create_dir_all(&test_dir).unwrap();

    let py_file = test_dir.join("test.py");
    let js_file = test_dir.join("test.js");
    let txt_file = test_dir.join("test.txt");

    fs::write(&py_file, "myVariable = 1").unwrap();
    fs::write(&js_file, "myVariable = 2").unwrap();
    fs::write(&txt_file, "myVariable = 3").unwrap();

    let output = Command::new(get_binary_path())
        .args(&[
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_error_missing_from() {
    let output = Command::new(get_binary_path())
        .args(&["convert", "--to-snake", "dummy.py"])
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("required") || stderr.contains("from"));
}

#[test]
fn test_cli_error_missing_to() {
    let output = Command::new(get_binary_path())
        .args(&["convert", "--from-camel", "dummy.py"])
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("required") || stderr.contains("to"));
}

#[test]
fn test_cli_error_conflicting_from() {
    let output = Command::new(get_binary_path())
        .args(&[
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
    let test_cases = vec![
        ("--from-camel", "--to-pascal", "myName", "MyName"),
        ("--from-pascal", "--to-snake", "MyName", "my_name"),
        ("--from-snake", "--to-kebab", "my_name", "my-name"),
        ("--from-kebab", "--to-screaming-snake", "my-name", "MY_NAME"),
        ("--from-screaming-snake", "--to-camel", "MY_NAME", "myName"),
    ];

    for (idx, (from_arg, to_arg, input, expected)) in test_cases.iter().enumerate() {
        let test_dir = std::env::temp_dir().join(format!("reformat_test_cli_combo_{}", idx));
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        fs::write(&test_file, input).unwrap();

        let output = Command::new(get_binary_path())
            .args(&["convert", from_arg, to_arg, "-e", ".txt"])
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

        fs::remove_dir_all(&test_dir).unwrap();
    }
}

// Whitespace cleaning tests

#[test]
fn test_cli_clean_basic() {
    let test_dir = std::env::temp_dir().join("reformat_test_clean_basic");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "line1   \nline2\t\nline3\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["clean"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "line1\nline2\nline3\n");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cleaned"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_clean_dry_run() {
    let test_dir = std::env::temp_dir().join("reformat_test_clean_dry");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.txt");
    let original = "line1   \nline2\t\nline3\n";
    fs::write(&test_file, original).unwrap();

    let output = Command::new(get_binary_path())
        .args(&["clean", "--dry-run"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    // File should be unchanged
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, original);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[DRY-RUN]") || stdout.contains("Would clean"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_clean_recursive() {
    let test_dir = std::env::temp_dir().join("reformat_test_clean_recursive");
    fs::create_dir_all(&test_dir).unwrap();

    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();

    let file1 = test_dir.join("file1.txt");
    let file2 = sub_dir.join("file2.txt");

    fs::write(&file1, "line1   \n").unwrap();
    fs::write(&file2, "line2\t\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["clean", "-r"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    let content1 = fs::read_to_string(&file1).unwrap();
    let content2 = fs::read_to_string(&file2).unwrap();

    assert_eq!(content1, "line1\n");
    assert_eq!(content2, "line2\n");

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_clean_extension_filtering() {
    let test_dir = std::env::temp_dir().join("reformat_test_clean_exts");
    fs::create_dir_all(&test_dir).unwrap();

    let py_file = test_dir.join("test.py");
    let txt_file = test_dir.join("test.txt");

    fs::write(&py_file, "line1   \n").unwrap();
    fs::write(&txt_file, "line1   \n").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["clean", "-e", ".py"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    let py_content = fs::read_to_string(&py_file).unwrap();
    let txt_content = fs::read_to_string(&txt_file).unwrap();

    assert_eq!(py_content, "line1\n"); // Should be cleaned
    assert_eq!(txt_content, "line1   \n"); // Should not be cleaned

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_clean_no_changes_needed() {
    let test_dir = std::env::temp_dir().join("reformat_test_clean_no_changes");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "line1\nline2\nline3\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["clean"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat clean");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No files needed cleaning"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_clean_help() {
    let output = Command::new(get_binary_path())
        .args(&["clean", "--help"])
        .output()
        .expect("Failed to execute reformat clean --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Remove trailing whitespace"));
}

#[test]
fn test_cli_convert_subcommand() {
    let test_dir = std::env::temp_dir().join("reformat_test_convert_subcommand");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.py");
    fs::write(&test_file, "myVariable = 'test'").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["convert", "--from-camel", "--to-snake"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat convert");

    assert!(output.status.success());

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("my_variable"));

    fs::remove_dir_all(&test_dir).unwrap();
}

// Rename tests

#[test]
fn test_cli_rename_lowercase() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_lowercase");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("TestFile.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--to-lowercase"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_uppercase() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_uppercase");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("testfile.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--to-uppercase"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_capitalize() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_capitalize");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("testFile.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--to-capitalize"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_to_underscore() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_underscore");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--underscored"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("test_file.txt").exists());
    assert!(!test_file.exists());

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_to_hyphen() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_hyphen");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--hyphenated"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("test-file.txt").exists());
    assert!(!test_file.exists());

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_add_prefix() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_add_prefix");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--add-prefix", "new_"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("new_file.txt").exists());
    assert!(!test_file.exists());

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_rm_prefix() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_rm_prefix");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("old_file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--rm-prefix", "old_"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("file.txt").exists());
    assert!(!test_file.exists());

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_add_suffix() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_add_suffix");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("file.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--add-suffix", "_backup"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("file_backup.txt").exists());
    assert!(!test_file.exists());

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_rm_suffix() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_rm_suffix");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("file_old.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--rm-suffix", "_old"])
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("file.txt").exists());
    assert!(!test_file.exists());

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_combined() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_combined");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("old_Test File.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&[
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_dry_run() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_dry");
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("TestFile.txt");
    let original_content = "content";
    fs::write(&test_file, original_content).unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--to-lowercase", "--dry-run"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_recursive() {
    let test_dir = std::env::temp_dir().join("reformat_test_rename_recursive");
    fs::create_dir_all(&test_dir).unwrap();

    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();

    let file1 = test_dir.join("File1.txt");
    let file2 = sub_dir.join("File2.txt");

    fs::write(&file1, "content1").unwrap();
    fs::write(&file2, "content2").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--to-lowercase", "-r"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat rename");

    assert!(output.status.success());
    assert!(test_dir.join("file1.txt").exists());
    assert!(sub_dir.join("file2.txt").exists());

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_rename_help() {
    let output = Command::new(get_binary_path())
        .args(&["rename_files", "--help"])
        .output()
        .expect("Failed to execute reformat rename --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Rename files"));
}

// Combined default command tests
#[test]
fn test_cli_combined_default() {
    let test_dir = std::env::temp_dir().join("reformat_test_combined_default");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    // Create a file with uppercase name, emojis, and trailing whitespace
    let test_file = test_dir.join("TestFile.txt");
    fs::write(&test_file, "Line 1   \nTask ✅ done\nLine 3\t\n").unwrap();

    let output = Command::new(get_binary_path())
        .arg(&test_file)
        .output()
        .expect("Failed to execute reformat (default command)");

    assert!(output.status.success());

    // File should be renamed to lowercase
    let renamed_file = test_dir.join("testfile.txt");
    assert!(renamed_file.exists(), "File should be renamed to lowercase");

    // On case-insensitive filesystems (like macOS default), TestFile.txt and testfile.txt
    // refer to the same file. Check that the actual filename on disk is lowercase.
    let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "Should have exactly one file");
    let actual_name = entries[0].as_ref().unwrap().file_name();
    assert_eq!(
        actual_name.to_str().unwrap(),
        "testfile.txt",
        "Filename should be lowercase"
    );

    // Check content transformations
    let content = fs::read_to_string(&renamed_file).unwrap();

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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_combined_recursive() {
    let test_dir = std::env::temp_dir().join("reformat_test_combined_recursive");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();

    let file1 = test_dir.join("File1.txt");
    let file2 = sub_dir.join("File2.md");

    fs::write(&file1, "Text   \n✅ Done\n").unwrap();
    fs::write(&file2, "More text\t\n☐ Todo\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["-r"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat -r");

    assert!(output.status.success());

    // Both files should be renamed
    assert!(test_dir.join("file1.txt").exists());
    assert!(sub_dir.join("file2.md").exists());

    // Check content transformations for file1
    let content1 = fs::read_to_string(&test_dir.join("file1.txt")).unwrap();
    assert!(content1.contains("[x]"));
    assert!(!content1.contains("✅"));
    assert!(!content1.contains("   \n"));

    // Check content transformations for file2
    let content2 = fs::read_to_string(&sub_dir.join("file2.md")).unwrap();
    assert!(content2.contains("[ ]"));
    assert!(!content2.contains("☐"));
    assert!(!content2.contains("\t\n"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_combined_dry_run() {
    let test_dir = std::env::temp_dir().join("reformat_test_combined_dry");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("TestFile.txt");
    let original_content = "Line 1   \nTask ✅\n";
    fs::write(&test_file, original_content).unwrap();

    let output = Command::new(get_binary_path())
        .args(&["--dry-run"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_cli_combined_no_changes_needed() {
    let test_dir = std::env::temp_dir().join("reformat_test_combined_nochange");
    let _ = fs::remove_dir_all(&test_dir);
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

    fs::remove_dir_all(&test_dir).unwrap();
}

// =============================================================================
// Preset tests
// =============================================================================

#[test]
fn test_preset_clean_step() {
    let test_dir = std::env::temp_dir().join("reformat_test_preset_clean");
    let _ = fs::remove_dir_all(&test_dir);
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
        .args(&["-p", "tidy"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_preset_rename_step() {
    let test_dir = std::env::temp_dir().join("reformat_test_preset_rename");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(
        test_dir.join("reformat.json"),
        r#"{"lower": {"steps": ["rename"], "rename": {"case_transform": "lowercase"}}}"#,
    )
    .unwrap();

    let test_file = test_dir.join("MyFile.txt");
    fs::write(&test_file, "content").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["-p", "lower"])
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
        .filter(|e| {
            e.file_name()
                .to_str()
                .map_or(false, |n| n.ends_with(".txt"))
        })
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file_name().to_str().unwrap(), "myfile.txt");

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_preset_multi_step() {
    let test_dir = std::env::temp_dir().join("reformat_test_preset_multi");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(
        test_dir.join("reformat.json"),
        r#"{"all": {"steps": ["rename", "clean"], "rename": {"case_transform": "lowercase"}}}"#,
    )
    .unwrap();

    let test_file = test_dir.join("MyCode.py");
    fs::write(&test_file, "x = 1   \ny = 2\t\n").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["-p", "all"])
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
        .filter(|e| e.file_name().to_str().map_or(false, |n| n.ends_with(".py")))
        .collect();
    assert_eq!(entries.len(), 1);
    let actual_name = entries[0].file_name();
    assert_eq!(actual_name.to_str().unwrap(), "mycode.py");

    let content = fs::read_to_string(entries[0].path()).unwrap();
    assert_eq!(content, "x = 1\ny = 2\n");

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_preset_dry_run_override() {
    let test_dir = std::env::temp_dir().join("reformat_test_preset_dryrun");
    let _ = fs::remove_dir_all(&test_dir);
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
        .args(&["-p", "tidy", "--dry-run"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_preset_missing_config_file() {
    let test_dir = std::env::temp_dir().join("reformat_test_preset_noconfig");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    // No reformat.json created
    let output = Command::new(get_binary_path())
        .args(&["-p", "whatever"])
        .arg(&test_dir)
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reformat.json not found"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_preset_unknown_preset_name() {
    let test_dir = std::env::temp_dir().join("reformat_test_preset_unknown");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(
        test_dir.join("reformat.json"),
        r#"{"code": {"steps": ["clean"]}}"#,
    )
    .unwrap();

    let output = Command::new(get_binary_path())
        .args(&["-p", "nonexistent"])
        .arg(&test_dir)
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("preset 'nonexistent' not found"));

    fs::remove_dir_all(&test_dir).unwrap();
}

// ==================== Job tests ====================

#[test]
fn test_job_from_file() {
    let test_dir = std::env::temp_dir().join("reformat_test_job_file");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(&job_file, r#"{"steps": ["clean"]}"#).unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "hello   \nworld  \n").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["--job"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_job_from_stdin() {
    let test_dir = std::env::temp_dir().join("reformat_test_job_stdin");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, "hello   \n").unwrap();

    let mut child = Command::new(get_binary_path())
        .args(&["--job", "-"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_job_multi_step() {
    let test_dir = std::env::temp_dir().join("reformat_test_job_multi");
    let _ = fs::remove_dir_all(&test_dir);
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
        .args(&["--job"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_job_dry_run() {
    let test_dir = std::env::temp_dir().join("reformat_test_job_dry");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(&job_file, r#"{"steps": ["clean"]}"#).unwrap();

    let test_file = test_dir.join("test.txt");
    let original = "hello   \n";
    fs::write(&test_file, original).unwrap();

    let output = Command::new(get_binary_path())
        .args(&["--job"])
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

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_job_missing_file() {
    let test_dir = std::env::temp_dir().join("reformat_test_job_missing");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let output = Command::new(get_binary_path())
        .args(&["--job", "/nonexistent/job.json"])
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to read job file"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_job_invalid_json() {
    let test_dir = std::env::temp_dir().join("reformat_test_job_badjson");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(&job_file, "not valid json {{{").unwrap();

    let output = Command::new(get_binary_path())
        .args(&["--job"])
        .arg(&job_file)
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse job"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_job_unknown_step() {
    let test_dir = std::env::temp_dir().join("reformat_test_job_badstep");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let job_file = test_dir.join("job.json");
    fs::write(&job_file, r#"{"steps": ["clean", "bogus"]}"#).unwrap();

    let output = Command::new(get_binary_path())
        .args(&["--job"])
        .arg(&job_file)
        .arg(&test_dir)
        .output()
        .expect("Failed to execute reformat");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown step 'bogus'"));

    fs::remove_dir_all(&test_dir).unwrap();
}

#[test]
fn test_job_conflicts_with_preset() {
    let output = Command::new(get_binary_path())
        .args(&["--job", "job.json", "-p", "code", "."])
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
