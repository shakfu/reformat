//! CI and editor workflow features: `--check`, `--diff`, multiple paths,
//! file selection, end-of-file fixes and `.editorconfig`.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reformat"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run reformat")
}

/// Runs reformat with `input` on stdin.
fn run_stdin(dir: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_reformat"))
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run reformat");
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn fixture(files: &[(&str, &str)]) -> tempfile::TempDir {
    let tmp = tempfile::Builder::new()
        .prefix("reformat-workflow-")
        .tempdir()
        .unwrap();
    for (name, content) in files {
        let path = tmp.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    tmp
}

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).unwrap()
}

#[test]
fn test_check_exits_1_when_changes_are_pending_and_writes_nothing() {
    let tmp = fixture(&[("dirty.py", "x  \n"), ("clean.py", "y\n")]);
    let dir = tmp.path();

    let out = run(dir, &["clean", "--check", "."]);
    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    assert_eq!(read(dir, "dirty.py"), "x  \n");

    let out = run(dir, &["clean", "--check", "clean.py"]);
    assert_eq!(out.status.code(), Some(0), "{:?}", out);
}

#[test]
fn test_check_applies_to_pipelines_and_renames() {
    let tmp = fixture(&[("Upper.txt", "a  \n")]);
    let dir = tmp.path();
    fs::write(dir.join("job.json"), r#"{"steps": ["clean"]}"#).unwrap();

    assert_eq!(
        run(dir, &["--job", "job.json", "--check", "."])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        run(dir, &["rename_files", "--to-lowercase", "--check", "."])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(run(dir, &["--check", "."]).status.code(), Some(1));

    let names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"Upper.txt".to_string()), "{:?}", names);
    assert_eq!(read(dir, "Upper.txt"), "a  \n");
}

#[test]
fn test_errors_exit_2() {
    let tmp = fixture(&[]);
    let out = run(tmp.path(), &["clean", "--check", "missing.py"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn test_diff_prints_unified_diff_and_writes_nothing() {
    let tmp = fixture(&[("a.py", "keep\nx  \n")]);
    let dir = tmp.path();

    let out = run(dir, &["clean", "--diff", "a.py"]);
    assert!(out.status.success(), "{:?}", out);
    let text = stdout(&out);
    assert!(text.contains("--- a/a.py"), "{}", text);
    assert!(text.contains("+++ b/a.py"), "{}", text);
    assert!(text.contains("-x  \n+x\n"), "{}", text);
    assert_eq!(read(dir, "a.py"), "keep\nx  \n");
}

#[test]
fn test_diff_shows_line_ending_changes_and_combines_steps() {
    let tmp = fixture(&[("a.txt", "q  \n")]);
    let dir = tmp.path();
    fs::write(
        dir.join("job.json"),
        r#"{"steps": ["endings", "clean"], "endings": {"style": "crlf"}}"#,
    )
    .unwrap();

    let out = run(dir, &["--job", "job.json", "--diff", "a.txt"]);
    let text = stdout(&out);
    // One diff for the file, reflecting both steps.
    assert_eq!(text.matches("+++ b/a.txt").count(), 1, "{}", text);
    assert!(text.contains("+q\\r\n"), "{}", text);
}

#[test]
fn test_multiple_paths() {
    let tmp = fixture(&[("a.py", "a  \n"), ("b.md", "b  \n"), ("c.py", "c  \n")]);
    let dir = tmp.path();

    let out = run(dir, &["clean", "a.py", "b.md"]);
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "a.py"), "a\n");
    assert_eq!(read(dir, "b.md"), "b\n");
    assert_eq!(read(dir, "c.py"), "c  \n");
}

#[test]
fn test_missing_path_aborts_before_any_file_is_changed() {
    let tmp = fixture(&[("a.py", "a  \n")]);
    let dir = tmp.path();

    let out = run(dir, &["clean", "a.py", "missing.py"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(read(dir, "a.py"), "a  \n");
}

#[test]
fn test_include_reaches_extensionless_and_hidden_files() {
    let tmp = fixture(&[
        ("Makefile", "all:  \n"),
        (".github/ci.yml", "on: push  \n"),
        ("a.py", "a  \n"),
    ]);
    let dir = tmp.path();

    // Without --hidden, .github is not walked even when a glob matches.
    let out = run(
        dir,
        &["clean", "--include", "Makefile", "--include", "*.yml", "."],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "Makefile"), "all:\n");
    assert_eq!(read(dir, ".github/ci.yml"), "on: push  \n");
    assert_eq!(
        read(dir, "a.py"),
        "a  \n",
        "--include must restrict selection"
    );

    let out = run(dir, &["clean", "--hidden", "--include", "*.yml", "."]);
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, ".github/ci.yml"), "on: push\n");
}

#[test]
fn test_exclude_glob() {
    let tmp = fixture(&[("src/a.py", "a  \n"), ("third_party/b.py", "b  \n")]);
    let dir = tmp.path();

    assert!(run(dir, &["clean", "--exclude", "third_party", "."])
        .status
        .success());
    assert_eq!(read(dir, "src/a.py"), "a\n");
    assert_eq!(read(dir, "third_party/b.py"), "b  \n");
}

/// `--exclude DIR` also covers files inside DIR that are named directly, as
/// a pre-commit hook names them.
#[test]
fn test_exclude_applies_to_named_files_in_excluded_directories() {
    let tmp = fixture(&[("vendor/lib/b.py", "b  \n"), ("src/a.py", "a  \n")]);
    let dir = tmp.path();

    let out = run(
        dir,
        &[
            "clean",
            "--exclude",
            "vendor",
            "vendor/lib/b.py",
            "src/a.py",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "vendor/lib/b.py"), "b  \n");
    assert_eq!(read(dir, "src/a.py"), "a\n");
}

#[test]
fn test_gitignore_is_respected_unless_no_ignore() {
    let tmp = fixture(&[
        (".gitignore", "generated.py\n"),
        ("generated.py", "g  \n"),
        ("src.py", "s  \n"),
        ("build/out.py", "o  \n"),
    ]);
    let dir = tmp.path();
    fs::create_dir(dir.join(".git")).unwrap();

    assert!(run(dir, &["clean", "."]).status.success());
    assert_eq!(read(dir, "src.py"), "s\n");
    assert_eq!(read(dir, "generated.py"), "g  \n");
    assert_eq!(read(dir, "build/out.py"), "o  \n");

    assert!(run(dir, &["clean", "--no-ignore", "."]).status.success());
    assert_eq!(read(dir, "generated.py"), "g\n");
    assert_eq!(read(dir, "build/out.py"), "o\n");
}

#[test]
fn test_no_ignore_and_hidden_never_touch_git_metadata() {
    let tmp = fixture(&[(".git/config", "[core]  \n"), ("a.txt", "a  \n")]);
    let dir = tmp.path();

    let out = run(
        dir,
        &["clean", "--hidden", "--no-ignore", "--include", "*", "."],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "a.txt"), "a\n");
    assert_eq!(read(dir, ".git/config"), "[core]  \n");
}

#[test]
fn test_clean_end_of_file_flags() {
    let tmp = fixture(&[("a.txt", "a\n\n\n"), ("b.txt", "b"), ("c.txt", "c\n\n")]);
    let dir = tmp.path();

    let out = run(
        dir,
        &[
            "clean",
            "--final-newline",
            "--trim-blank-lines",
            "a.txt",
            "b.txt",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "a.txt"), "a\n");
    assert_eq!(read(dir, "b.txt"), "b\n");

    // Off by default.
    assert!(run(dir, &["clean", "c.txt"]).status.success());
    assert_eq!(read(dir, "c.txt"), "c\n\n");
}

#[test]
fn test_clean_end_of_file_options_in_a_job() {
    let tmp = fixture(&[("a.md", "a")]);
    let dir = tmp.path();
    fs::write(
        dir.join("job.json"),
        r#"{"steps": ["clean"], "clean": {"insert_final_newline": true}}"#,
    )
    .unwrap();

    assert!(run(dir, &["--job", "job.json", "a.md"]).status.success());
    assert_eq!(read(dir, "a.md"), "a\n");
}

#[test]
fn test_editorconfig_command() {
    let tmp = fixture(&[
        (
            ".editorconfig",
            "root = true\n\n[*]\ntrim_trailing_whitespace = true\ninsert_final_newline = true\n\
             end_of_line = lf\nindent_style = space\nindent_size = 4\n\n\
             [*.md]\ntrim_trailing_whitespace = false\n\n[Makefile]\nindent_style = tab\n",
        ),
        ("a.txt", "x  \r\n\ty"),
        ("notes.md", "keep  "),
        ("Makefile", "all:\n\techo  \n"),
    ]);
    let dir = tmp.path();

    let check = run(dir, &["editorconfig", "--check", "."]);
    assert_eq!(check.status.code(), Some(1), "{:?}", check);

    let out = run(dir, &["editorconfig", "."]);
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "a.txt"), "x\n\ty\n", "indent is opt-in");
    assert_eq!(read(dir, "notes.md"), "keep  \n");
    assert_eq!(read(dir, "Makefile"), "all:\n\techo\n");

    assert!(run(dir, &["editorconfig", "--indent", "a.txt"])
        .status
        .success());
    assert_eq!(read(dir, "a.txt"), "x\n    y\n");

    assert_eq!(
        run(dir, &["editorconfig", "--check", "."]).status.code(),
        Some(0)
    );
}

/// A hidden file name, such as the pre-commit config a hook passes, needs
/// `--hidden`, whether named directly or reached by walking.
#[test]
fn test_hidden_flag_reaches_dotfiles() {
    let tmp = fixture(&[(".pre-commit-config.yaml", "repos:  \n")]);
    let dir = tmp.path();

    let without = run(dir, &["clean", "--include", "*", ".pre-commit-config.yaml"]);
    assert!(without.status.success(), "{:?}", without);
    assert_eq!(read(dir, ".pre-commit-config.yaml"), "repos:  \n");

    let named = run(
        dir,
        &[
            "clean",
            "--hidden",
            "--include",
            "*",
            ".pre-commit-config.yaml",
        ],
    );
    assert!(named.status.success(), "{:?}", named);
    assert_eq!(read(dir, ".pre-commit-config.yaml"), "repos:\n");

    fs::write(dir.join(".pre-commit-config.yaml"), "repos:  \n").unwrap();
    assert!(run(dir, &["clean", "--hidden", "--include", "*.yaml", "."])
        .status
        .success());
    assert_eq!(read(dir, ".pre-commit-config.yaml"), "repos:\n");
}

/// The entries in `.pre-commit-hooks.yaml`, run as pre-commit runs them:
/// split into arguments with no shell, followed by the staged file names.
#[test]
fn test_pre_commit_hook_entries() {
    let manifest = include_str!("../../.pre-commit-hooks.yaml");
    let entries: Vec<&str> = manifest
        .lines()
        .filter_map(|l| l.trim().strip_prefix("entry: "))
        .map(|e| e.trim_matches('"'))
        .collect();
    assert_eq!(entries.len(), 4, "{:?}", entries);

    let tmp = fixture(&[
        (
            ".editorconfig",
            "root = true\n[*]\ninsert_final_newline = true\n",
        ),
        ("Makefile", "all:  \n\n\n"),
        (".pre-commit-config.yaml", "repos: []  \r\n"),
        ("notes.md", "done \u{2705}\n"),
        ("c.txt", "z"),
    ]);
    let dir = tmp.path();
    let files = ["Makefile", ".pre-commit-config.yaml", "notes.md", "c.txt"];

    for entry in &entries {
        let mut args: Vec<&str> = entry.split_whitespace().collect();
        assert_eq!(args.remove(0), "reformat");
        args.extend(files);
        let out = run(dir, &args);
        assert!(out.status.success(), "{}: {:?}", entry, out);
    }

    assert_eq!(read(dir, "Makefile"), "all:\n");
    assert_eq!(read(dir, ".pre-commit-config.yaml"), "repos: []\n");
    assert_eq!(read(dir, "notes.md"), "done [x]\n");
    assert_eq!(read(dir, "c.txt"), "z\n");
}

// ---------------------------------------------------------------------------
// stdin/stdout mode
// ---------------------------------------------------------------------------

#[test]
fn test_stdin_filename_writes_result_to_stdout_only() {
    let tmp = fixture(&[]);
    let dir = tmp.path();

    let out = run_stdin(dir, &["clean", "--stdin-filename", "a.py"], b"x  \r\ny\t\n");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(out.stdout, b"x\r\ny\n");
    // Log lines must not corrupt the content on stdout.
    assert!(String::from_utf8_lossy(&out.stderr).contains("Cleaned"));
    assert!(
        fs::read_dir(dir).unwrap().next().is_none(),
        "a file was written"
    );
}

#[test]
fn test_stdin_unselected_content_passes_through() {
    let tmp = fixture(&[]);
    let dir = tmp.path();

    // No step accepts the extension.
    let out = run_stdin(dir, &["clean", "--stdin-filename", "a.bin"], b"x  \n");
    assert_eq!(out.stdout, b"x  \n");

    // Excluded by glob.
    let out = run_stdin(
        dir,
        &[
            "clean",
            "--exclude",
            "vendor",
            "--stdin-filename",
            "vendor/a.py",
        ],
        b"x  \n",
    );
    assert_eq!(out.stdout, b"x  \n");

    // Hidden unless --hidden.
    let out = run_stdin(
        dir,
        &["clean", "--include", "*", "--stdin-filename", ".env"],
        b"x  \n",
    );
    assert_eq!(out.stdout, b"x  \n");
    let out = run_stdin(
        dir,
        &[
            "clean",
            "--hidden",
            "--include",
            "*",
            "--stdin-filename",
            ".env",
        ],
        b"x  \n",
    );
    assert_eq!(out.stdout, b"x\n");
}

#[test]
fn test_stdin_check_and_diff() {
    let tmp = fixture(&[]);
    let dir = tmp.path();

    let out = run_stdin(
        dir,
        &["clean", "--check", "--stdin-filename", "a.py"],
        b"x  \n",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());

    let out = run_stdin(
        dir,
        &["clean", "--check", "--stdin-filename", "a.py"],
        b"x\n",
    );
    assert_eq!(out.status.code(), Some(0));

    let out = run_stdin(
        dir,
        &["clean", "--diff", "--stdin-filename", "a.py"],
        b"x  \n",
    );
    assert!(out.status.success());
    assert_eq!(
        stdout(&out),
        "--- a/a.py\n+++ b/a.py\n@@ -1 +1 @@\n-x  \n+x\n"
    );
}

#[test]
fn test_stdin_with_default_command_job_and_editorconfig() {
    let tmp = fixture(&[
        (
            ".editorconfig",
            "root = true\n[*.txt]\ninsert_final_newline = true\n",
        ),
        (
            "job.json",
            r#"{"steps": ["endings", "clean"], "endings": {"style": "crlf"}}"#,
        ),
    ]);
    let dir = tmp.path();

    let out = run_stdin(
        dir,
        &["--stdin-filename", "n.md"],
        "done \u{2705}  \n".as_bytes(),
    );
    assert_eq!(stdout(&out), "done [x]\n");

    let out = run_stdin(
        dir,
        &["--job", "job.json", "--stdin-filename", "a.txt"],
        b"a  \n",
    );
    assert_eq!(out.stdout, b"a\r\n");

    let out = run_stdin(dir, &["editorconfig", "--stdin-filename", "f.txt"], b"z");
    assert_eq!(out.stdout, b"z\n");
}

#[test]
fn test_stdin_rejects_path_steps_and_a_stdin_job() {
    let tmp = fixture(&[]);
    let dir = tmp.path();

    let out = run_stdin(
        dir,
        &[
            "rename_files",
            "--to-lowercase",
            "--stdin-filename",
            "A.txt",
        ],
        b"x",
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot read stdin"));

    let out = run_stdin(dir, &["--job", "-", "--stdin-filename", "a.py"], b"{}");
    assert_eq!(out.status.code(), Some(2));

    let out = run(dir, &["clean", "--stdin-filename", "a.py", "a.py"]);
    assert!(!out.status.success(), "paths and --stdin-filename conflict");
}

#[test]
fn test_diff_output_is_not_mixed_with_log_lines() {
    let tmp = fixture(&[("a.py", "x  \n")]);
    let out = run(tmp.path(), &["clean", "--diff", "."]);
    let text = stdout(&out);
    assert!(text.starts_with("--- a/"), "{}", text);
    assert!(!text.contains("Would clean"), "{}", text);
}

// ---------------------------------------------------------------------------
// replace: repeated pairs, --literal, --ignore-case
// ---------------------------------------------------------------------------

#[test]
fn test_replace_pairs_literal_and_ignore_case() {
    let tmp = fixture(&[("a.txt", "foo.bar(x) Foo $1\n")]);
    let dir = tmp.path();

    let out = run(
        dir,
        &[
            "replace",
            "-f",
            "foo.bar(",
            "--replace-with",
            "baz($1",
            "-f",
            "foo",
            "--replace-with",
            "qux",
            "--literal",
            "-i",
            "a.txt",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "a.txt"), "baz($1x) qux $1\n");
}

#[test]
fn test_replace_pairs_apply_in_order_as_regexes() {
    let tmp = fixture(&[("a.txt", "a1 b2\n")]);
    let dir = tmp.path();

    let out = run(
        dir,
        &[
            "replace",
            "-f",
            r"a(\d)",
            "--replace-with",
            "b$1",
            "-f",
            "b",
            "--replace-with",
            "c",
            "a.txt",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "a.txt"), "c1 c2\n");
}

#[test]
fn test_replace_unpaired_find_is_an_error() {
    let tmp = fixture(&[("a.txt", "a\n")]);
    let dir = tmp.path();
    let out = run(
        dir,
        &[
            "replace",
            "-f",
            "a",
            "-f",
            "b",
            "--replace-with",
            "c",
            "a.txt",
        ],
    );
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(read(dir, "a.txt"), "a\n");
}

#[test]
fn test_replace_pattern_options_in_a_job() {
    let tmp = fixture(&[
        ("a.txt", "x.y X.Y\n"),
        (
            "job.json",
            r#"{"steps": ["replace"], "replace": {"patterns": [{"find": "x.y", "replace": "z", "literal": true, "ignore_case": true}]}}"#,
        ),
    ]);
    let dir = tmp.path();
    assert!(run(dir, &["--job", "job.json", "a.txt"]).status.success());
    assert_eq!(read(dir, "a.txt"), "z z\n");
}

// ---------------------------------------------------------------------------
// Config lookup and the presets listing
// ---------------------------------------------------------------------------

#[test]
fn test_preset_found_from_target_path_and_listed() {
    let tmp = fixture(&[
        (
            "project/reformat.json",
            r#"{"tidy": {"steps": ["clean", "endings"]}, "a": {"steps": ["emojis"]}}"#,
        ),
        ("project/src/a.py", "q  \n"),
        ("elsewhere/.keep", ""),
    ]);
    let dir = tmp.path();
    let elsewhere = dir.join("elsewhere");

    let out = run(&elsewhere, &["-p", "tidy", "../project/src"]);
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(read(dir, "project/src/a.py"), "q\n");

    let out = run(&elsewhere, &["presets"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--config"));

    let out = run(
        &elsewhere,
        &["presets", "--config", "../project/reformat.json"],
    );
    assert!(out.status.success(), "{:?}", out);
    let text = stdout(&out);
    assert!(
        text.contains("  a: emojis\n  tidy: clean, endings\n"),
        "{}",
        text
    );
}

#[test]
fn test_config_flag_selects_the_file() {
    let tmp = fixture(&[
        ("reformat.json", r#"{"p": {"steps": ["emojis"]}}"#),
        ("other.json", r#"{"p": {"steps": ["clean"]}}"#),
        ("a.md", "x  \n"),
    ]);
    let dir = tmp.path();

    assert!(run(dir, &["--config", "other.json", "-p", "p", "a.md"])
        .status
        .success());
    assert_eq!(read(dir, "a.md"), "x\n");
}

// ---------------------------------------------------------------------------
// Completions and man pages
// ---------------------------------------------------------------------------

#[test]
fn test_completions_for_each_shell() {
    let tmp = fixture(&[]);
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let out = run(tmp.path(), &["completions", shell]);
        assert!(out.status.success(), "{}: {:?}", shell, out);
        let text = stdout(&out);
        assert!(
            text.contains("editorconfig"),
            "{} completions miss a subcommand",
            shell
        );
        assert!(
            text.contains("stdin-filename"),
            "{} completions miss a flag",
            shell
        );
    }
}

#[test]
fn test_man_page_to_stdout_and_directory() {
    let tmp = fixture(&[]);
    let dir = tmp.path();

    let out = run(dir, &["man"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains(".TH reformat 1"));

    let out = run(dir, &["man", "--out-dir", "pages"]);
    assert!(out.status.success(), "{:?}", out);
    for page in ["reformat.1", "reformat-clean.1", "reformat-replace.1"] {
        assert!(dir.join("pages").join(page).exists(), "{} missing", page);
    }
}
