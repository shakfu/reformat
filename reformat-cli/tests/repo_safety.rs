//! Safety tests: transformers must never modify version-control metadata,
//! and must not silently no-op on ordinary relative paths.
//!
//! These guard the invariant that `reformat` only ever touches the files a
//! user actually pointed it at. A transformer that walks into `.git/` can
//! destroy a repository irrecoverably, since renames are not journalled.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_reformat")
}

/// Runs a git command in `dir`, returning true on success.
fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Creates a repository with one committed file whose name and contents are
/// deliberately transformable (mixed case, trailing whitespace).
fn init_repo(dir: &Path) {
    assert!(git(dir, &["init", "-q"]), "git init failed");
    std::fs::write(dir.join("README.md"), "Title  \ntext  \n").unwrap();
    std::fs::write(dir.join("Notes.md"), "Body  \n").unwrap();
    assert!(git(dir, &["add", "."]), "git add failed");
    assert!(
        git(
            dir,
            &[
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=test",
                "commit",
                "-qm",
                "initial",
            ],
        ),
        "git commit failed"
    );
}

/// Asserts the repository is still intact and its metadata untouched.
fn assert_repo_intact(dir: &Path, what: &str) {
    for name in ["HEAD", "config", "index", "description"] {
        assert!(
            dir.join(".git").join(name).exists(),
            "{what}: .git/{name} is missing -- the repository metadata was modified"
        );
    }
    assert!(
        git(dir, &["rev-parse", "--git-dir"]),
        "{what}: `git rev-parse` failed -- the repository was destroyed"
    );
    assert!(
        git(dir, &["status", "--porcelain"]),
        "{what}: `git status` failed -- the repository was destroyed"
    );
}

/// Runs reformat with `args` from inside `dir`.
fn run(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run reformat")
}

#[test]
fn test_rename_files_does_not_touch_git_metadata() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    init_repo(dir);

    run(dir, &["rename_files", "--to-uppercase", "."]);

    assert_repo_intact(dir, "rename_files --to-uppercase");
    // The rename must still have done its job on the tracked file.
    assert!(
        dir.join("README.MD").exists() || dir.join("README.md").exists(),
        "rename_files did nothing at all -- the test would pass vacuously"
    );
}

#[test]
fn test_default_command_does_not_touch_git_metadata() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    init_repo(dir);

    run(dir, &["-r", "."]);

    assert_repo_intact(dir, "default command (-r .)");
    // Confirm the pipeline actually ran, so the test cannot pass vacuously.
    assert_eq!(
        std::fs::read_to_string(dir.join("Notes.md")).unwrap(),
        "Body\n",
        "default command did not clean Notes.md"
    );
}

#[test]
fn test_convert_does_not_touch_git_metadata() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    init_repo(dir);
    std::fs::write(dir.join("code.py"), "someName = 1\n").unwrap();

    // code.py is untracked, so the dirty-tree guard needs --allow-dirty.
    run(
        dir,
        &[
            "convert",
            "--from-camel",
            "--to-snake",
            "-r",
            "--allow-dirty",
            ".",
        ],
    );

    assert_repo_intact(dir, "convert --from-camel --to-snake");
    let converted = std::fs::read_to_string(dir.join("code.py")).unwrap();
    assert_eq!(
        converted, "some_name = 1\n",
        "convert did not process the target file -- the test would pass vacuously"
    );
}

#[test]
fn test_clean_does_not_touch_git_metadata() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    init_repo(dir);

    run(dir, &["clean", "-r", "."]);

    assert_repo_intact(dir, "clean");
}

#[test]
fn test_transformers_skip_build_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    for vendored in ["node_modules", "target", "__pycache__", ".venv"] {
        let sub = dir.join(vendored);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("Vendored.md"), "text  \n").unwrap();
    }
    std::fs::write(dir.join("Own.md"), "text  \n").unwrap();

    run(dir, &["rename_files", "--to-lowercase", "."]);
    run(dir, &["clean", "-r", "."]);

    for vendored in ["node_modules", "target", "__pycache__", ".venv"] {
        let sub = dir.join(vendored);
        assert!(
            sub.join("Vendored.md").exists(),
            "{vendored}/Vendored.md was renamed -- build directories must be skipped"
        );
        assert_eq!(
            std::fs::read_to_string(sub.join("Vendored.md")).unwrap(),
            "text  \n",
            "{vendored}/Vendored.md was cleaned -- build directories must be skipped"
        );
    }
    assert!(
        dir.join("own.md").exists(),
        "the tool skipped everything, including files it should have processed"
    );
}

/// `reformat clean .` is the most natural invocation of the tool and must not
/// be a silent no-op. The `.` component is not a hidden directory.
#[test]
fn test_relative_dot_path_is_processed() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join("a.txt"), "line1  \nline2  \n").unwrap();

    run(dir, &["clean", "-r", "."]);

    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "line1\nline2\n",
        "`clean .` did nothing -- a leading `.` path component must not be treated as hidden"
    );
}

/// An explicitly named hidden file is still skipped: the exclusion applies to
/// real directory names, not to `.` or `..` path components.
#[test]
fn test_explicit_hidden_file_is_still_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join(".hidden.txt"), "line1  \n").unwrap();

    run(dir, &["clean", ".hidden.txt"]);

    assert_eq!(
        std::fs::read_to_string(dir.join(".hidden.txt")).unwrap(),
        "line1  \n",
        "hidden files must be skipped even when named explicitly"
    );
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != ".git")
        .collect();
    v.sort();
    v
}

/// Renames, groups, conversions and replacements over uncommitted work
/// cannot be separated from the user's edits or undone with git.
#[test]
fn test_destructive_commands_refuse_uncommitted_changes() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    init_repo(dir);
    std::fs::write(dir.join("Notes.md"), "edited\n").unwrap();

    let out = run(dir, &["rename_files", "--to-uppercase", "."]);
    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("Notes.md"));
    assert_eq!(names(dir), ["Notes.md", "README.md"]);

    let out = run(
        dir,
        &["replace", "-f", "edited", "--replace-with", "x", "."],
    );
    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    assert_eq!(
        std::fs::read_to_string(dir.join("Notes.md")).unwrap(),
        "edited\n"
    );

    std::fs::write(
        dir.join("job.json"),
        r#"{"steps": ["replace"], "replace": {"patterns": [{"find": "edited", "replace": "x"}]}}"#,
    )
    .unwrap();
    let out = run(dir, &["--job", "job.json", "Notes.md"]);
    assert_eq!(out.status.code(), Some(2), "{:?}", out);

    // Previews and --allow-dirty still work.
    let out = run(
        dir,
        &[
            "replace",
            "-f",
            "edited",
            "--replace-with",
            "x",
            "--diff",
            ".",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let out = run(dir, &["--job", "job.json", "--allow-dirty", "Notes.md"]);
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        std::fs::read_to_string(dir.join("Notes.md")).unwrap(),
        "x\n"
    );
}

#[test]
fn test_untracked_files_count_as_uncommitted() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    init_repo(dir);
    std::fs::write(dir.join("new.py"), "someName = 1\n").unwrap();

    let out = run(dir, &["convert", "--from-camel", "--to-snake", "new.py"]);
    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    assert_eq!(
        std::fs::read_to_string(dir.join("new.py")).unwrap(),
        "someName = 1\n"
    );
}

/// Only the paths a command touches are checked, and hygiene commands such
/// as `clean` are not guarded: pre-commit runs them on staged files.
#[test]
fn test_guard_scope() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    init_repo(dir);
    std::fs::write(dir.join("Notes.md"), "edited  \n").unwrap();

    let out = run(dir, &["rename_files", "--to-uppercase", "README.md"]);
    assert!(out.status.success(), "{:?}", out);

    let out = run(dir, &["clean", "Notes.md"]);
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        std::fs::read_to_string(dir.join("Notes.md")).unwrap(),
        "edited\n"
    );
}

#[test]
fn test_group_refuses_uncommitted_changes() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    init_repo(dir);
    std::fs::create_dir(dir.join("t")).unwrap();
    std::fs::write(dir.join("t/a_1.txt"), "x").unwrap();
    std::fs::write(dir.join("t/a_2.txt"), "x").unwrap();

    let out = run(dir, &["group", "--no-interactive", "t"]);
    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    assert!(dir.join("t/a_1.txt").exists());

    let out = run(dir, &["group", "--no-interactive", "--allow-dirty", "t"]);
    assert!(out.status.success(), "{:?}", out);
    assert!(dir.join("t/a/a_1.txt").exists());
}

/// Scan directories typed at the interactive prompt are chosen after the
/// initial guard, so fixes to files with uncommitted changes are refused
/// at the point of applying them.
#[test]
fn test_interactive_group_refuses_to_fix_uncommitted_files() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let commit = |dir: &Path| {
        assert!(git(dir, &["add", "."]));
        assert!(git(
            dir,
            &[
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "commit",
                "-qm",
                "c"
            ],
        ));
    };
    let answer = |dir: &Path, extra: &[&str]| {
        let mut child = Command::new(binary())
            .args(["group", "--strip-prefix"])
            .args(extra)
            .arg("t")
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Scan? yes. Directories: src. Apply? yes.
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"y\nsrc\ny\n")
            .unwrap();
        child.wait_with_output().unwrap()
    };

    for (allow, expected) in [
        (false, "load(\"wbs_a.tmpl\") // edited\n"),
        (true, "load(\"wbs/a.tmpl\") // edited\n"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert!(git(dir, &["init", "-q"]));
        std::fs::create_dir_all(dir.join("t")).unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("t/wbs_a.tmpl"), "x").unwrap();
        std::fs::write(dir.join("t/wbs_b.tmpl"), "x").unwrap();
        std::fs::write(dir.join("src/main.go"), "load(\"wbs_a.tmpl\")\n").unwrap();
        commit(dir);
        std::fs::write(dir.join("src/main.go"), "load(\"wbs_a.tmpl\") // edited\n").unwrap();

        let extra: &[&str] = if allow { &["--allow-dirty"] } else { &[] };
        let out = answer(dir, extra);

        assert!(
            dir.join("t/wbs/a.tmpl").exists(),
            "the grouping itself should run"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("src/main.go")).unwrap(),
            expected
        );
        if allow {
            assert!(out.status.success(), "{:?}", out);
        } else {
            assert_eq!(out.status.code(), Some(2), "{:?}", out);
            assert!(String::from_utf8_lossy(&out.stderr).contains("apply_fixes"));
        }
    }
}
