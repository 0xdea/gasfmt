//! Integration tests for the `gasfmt` library and binary.

#![expect(clippy::expect_used, reason = "tests can use `expect`")]

#[cfg(test)]
mod tests {
    use std::io::{self, Write as _};
    use std::path::PathBuf;
    use std::process::{Command, Output, Stdio};
    use std::{env, fs, process, str};

    /// Absolute path to the compiled `gasfmt` binary under test.
    const BIN: &str = env!("CARGO_BIN_EXE_gasfmt");

    /// Hand-formatted sources, as pairs of file name and content.
    const FIXTURES: [(&str, &str); 5] = [
        ("include.S", include_str!("data/include.S")),
        ("listing1-1.S", include_str!("data/listing1-1.S")),
        ("listing2-4.S", include_str!("data/listing2-4.S")),
        ("listing5-9.S", include_str!("data/listing5-9.S")),
        ("listing7-3.S", include_str!("data/listing7-3.S")),
    ];

    /// Source that needs formatting.
    const UNFORMATTED: &str = "_main: mov x0,#0\n  bl _exit\n";

    /// The formatted version of [`UNFORMATTED`].
    const FORMATTED: &str = "_main:\n\tmov x0, #0\n\tbl  _exit\n";

    /// Runs the binary under test with `args`, feeding it `input` on standard
    /// input, and returns its output.
    fn run_bin(args: &[&str], input: &str) -> Output {
        run_bin_with(args, input, Stdio::piped(), Stdio::piped())
    }

    /// Runs the binary under test with `args`, feeding it `input` on standard
    /// input, with the given `stdout` and `stderr`, and returns its output.
    fn run_bin_with(args: &[&str], input: &str, stdout: Stdio, stderr: Stdio) -> Output {
        let mut child = Command::new(BIN)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .expect("failed to spawn the binary under test");
        let written = child
            .stdin
            .take()
            .expect("standard input should be piped")
            .write_all(input.as_bytes());
        // The binary may exit without reading its input: that shows in its
        // output, so only another failure to write is one of the test.
        if let Err(err) = written {
            assert_eq!(
                err.kind(),
                io::ErrorKind::BrokenPipe,
                "failed to write to standard input: {err}"
            );
        }

        child
            .wait_with_output()
            .expect("failed to wait for the binary under test")
    }

    /// Returns a pipe that nobody reads, so that every write to it fails.
    fn closed_pipe() -> Stdio {
        // The reading end is closed before the binary starts, so that the
        // outcome doesn't depend on timing.
        let (reader, writer) = io::pipe().expect("failed to create the pipe");
        drop(reader);

        writer.into()
    }

    /// Returns the standard output of `output` as UTF-8 text.
    fn stdout(output: &Output) -> &str {
        str::from_utf8(&output.stdout).expect("stdout should be UTF-8")
    }

    /// Returns the standard error of `output` as UTF-8 text.
    fn stderr(output: &Output) -> &str {
        str::from_utf8(&output.stderr).expect("stderr should be UTF-8")
    }

    /// Creates an empty temporary directory scoped by `label` and process ID.
    fn temp_dir(label: &str) -> PathBuf {
        let dirpath = env::temp_dir().join(format!("gasfmt-{label}-{}", process::id()));
        if dirpath.exists() {
            fs::remove_dir_all(&dirpath).expect("failed to remove the stale directory");
        }
        fs::create_dir_all(&dirpath).expect("failed to create the temporary directory");

        dirpath
    }

    /// Messes up the whitespace of the formatted `source`, without touching
    /// its tokens or whether each line is indented.
    fn scramble(source: &str) -> String {
        let mut scrambled = String::from("\n\n");
        for line in source.lines() {
            let (indent, text) = line
                .strip_prefix('\t')
                .map_or(("", line), |text| ("   \t ", text));
            // Comments and literals would be altered by the replacements.
            let text = if text.contains("//") || text.contains('"') || text.starts_with('#') {
                text.to_owned()
            } else {
                text.replace(' ', " \t ").replace(',', "  ,")
            };
            scrambled.push_str(indent);
            scrambled.push_str(&text);
            scrambled.push_str(" \t\r\n");
            if line.is_empty() {
                scrambled.push_str("  \r\n");
            }
        }

        scrambled
    }

    #[test]
    fn formatted_fixtures_are_left_unchanged() {
        for (name, source) in FIXTURES {
            assert_eq!(gasfmt::format(source), source, "{name} should not change");
        }
    }

    #[test]
    fn scrambled_fixtures_are_restored() {
        for (name, source) in FIXTURES {
            let scrambled = scramble(source);

            assert_ne!(scrambled, source, "{name} should have been scrambled");
            assert_eq!(
                gasfmt::format(&scrambled),
                source,
                "{name} should be restored"
            );
        }
    }

    #[test]
    fn standard_input_is_formatted_to_standard_output() {
        let output = run_bin(&[], UNFORMATTED);

        assert!(output.status.success(), "formatting should succeed");
        assert_eq!(
            stdout(&output),
            FORMATTED,
            "stdout should hold only the result"
        );
        assert!(
            stderr(&output).starts_with("gasfmt "),
            "the banner should be printed to stderr"
        );
    }

    #[test]
    fn check_reports_unformatted_standard_input() {
        let unformatted = run_bin(&["--check"], UNFORMATTED);
        let formatted = run_bin(&["--check"], FORMATTED);

        assert!(
            !unformatted.status.success(),
            "unformatted input should fail the check"
        );
        assert_eq!(
            stdout(&unformatted),
            "",
            "the check should not print the result"
        );
        assert!(
            formatted.status.success(),
            "formatted input should pass the check"
        );
    }

    #[test]
    fn files_are_formatted_in_place() {
        let dirpath = temp_dir("in-place");
        let dirty = dirpath.join("dirty.S");
        let clean = dirpath.join("clean.S");
        fs::write(&dirty, UNFORMATTED).expect("failed to write the dirty file");
        fs::write(&clean, FORMATTED).expect("failed to write the clean file");
        let dirty_arg = dirty.to_str().expect("the path should be UTF-8");
        let clean_arg = clean.to_str().expect("the path should be UTF-8");

        let output = run_bin(&[clean_arg, dirty_arg], "");

        assert!(output.status.success(), "formatting should succeed");
        assert_eq!(
            stdout(&output).lines().collect::<Vec<_>>(),
            [dirty_arg],
            "only the changed file should be listed"
        );
        assert!(
            stderr(&output)
                .lines()
                .any(|line| line == "[+] Formatted 1 of 2 files"),
            "the summary should count the changed file, got: {}",
            stderr(&output)
        );
        assert_eq!(
            fs::read_to_string(&dirty).expect("failed to read the dirty file"),
            FORMATTED,
            "the dirty file should have been rewritten"
        );
        assert_eq!(
            fs::read_to_string(&clean).expect("failed to read the clean file"),
            FORMATTED,
            "the clean file should be untouched"
        );

        fs::remove_dir_all(&dirpath).expect("failed to remove the temporary directory");
    }

    #[test]
    fn check_lists_unformatted_files_without_writing_them() {
        let dirpath = temp_dir("check");
        let dirty = dirpath.join("dirty.S");
        fs::write(&dirty, UNFORMATTED).expect("failed to write the dirty file");
        let dirty_arg = dirty.to_str().expect("the path should be UTF-8");

        let output = run_bin(&["--check", dirty_arg], "");

        assert!(
            !output.status.success(),
            "an unformatted file should fail the check"
        );
        assert_eq!(
            stdout(&output).lines().collect::<Vec<_>>(),
            [dirty_arg],
            "the unformatted file should be listed"
        );
        assert!(
            stderr(&output)
                .lines()
                .any(|line| line == "[!] 1 of 1 files need formatting"),
            "the summary should count the unformatted file, got: {}",
            stderr(&output)
        );
        assert_eq!(
            fs::read_to_string(&dirty).expect("failed to read the dirty file"),
            UNFORMATTED,
            "the check should not rewrite the file"
        );

        fs::remove_dir_all(&dirpath).expect("failed to remove the temporary directory");
    }

    #[test]
    fn diff_shows_the_changes_to_standard_input() {
        let unformatted = run_bin(&["--diff"], UNFORMATTED);
        let formatted = run_bin(&["--diff"], FORMATTED);

        assert!(
            !unformatted.status.success(),
            "unformatted input should fail"
        );
        assert_eq!(
            stdout(&unformatted),
            "--- <stdin>\n+++ <stdin>\n@@ -1,2 +1,3 @@\n-_main: mov x0,#0\n-  bl _exit\n\
             +_main:\n+\tmov x0, #0\n+\tbl  _exit\n",
            "stdout should hold the unified diff"
        );
        assert!(formatted.status.success(), "formatted input should pass");
        assert_eq!(stdout(&formatted), "", "there should be nothing to show");
    }

    #[test]
    fn diff_shows_the_changes_to_files_without_writing_them() {
        let dirpath = temp_dir("diff");
        let dirty = dirpath.join("dirty.S");
        let clean = dirpath.join("clean.S");
        fs::write(&dirty, UNFORMATTED).expect("failed to write the dirty file");
        fs::write(&clean, FORMATTED).expect("failed to write the clean file");
        let dirty_arg = dirty.to_str().expect("the path should be UTF-8");
        let clean_arg = clean.to_str().expect("the path should be UTF-8");

        let output = run_bin(&["--diff", clean_arg, dirty_arg], "");

        assert!(!output.status.success(), "an unformatted file should fail");
        assert_eq!(
            stdout(&output).lines().take(3).collect::<Vec<_>>(),
            [
                format!("--- {dirty_arg}").as_str(),
                format!("+++ {dirty_arg}").as_str(),
                "@@ -1,2 +1,3 @@"
            ],
            "only the diff of the unformatted file should be printed"
        );
        assert!(
            stderr(&output)
                .lines()
                .any(|line| line == "[!] 1 of 2 files need formatting"),
            "the summary should count the unformatted file, got: {}",
            stderr(&output)
        );
        assert_eq!(
            fs::read_to_string(&dirty).expect("failed to read the dirty file"),
            UNFORMATTED,
            "the diff should not rewrite the file"
        );

        fs::remove_dir_all(&dirpath).expect("failed to remove the temporary directory");
    }

    #[test]
    fn diff_conflicts_with_check() {
        let output = run_bin(&["--diff", "--check"], "");

        assert_eq!(
            output.status.code(),
            Some(2),
            "the two options should be rejected as a usage error"
        );
        assert_eq!(stdout(&output), "", "nothing should be formatted");
    }

    #[test]
    fn closed_pipes_fail_without_crashing() {
        // More output than a pipe can hold, so that the writes can't succeed
        // even if another test's child briefly inherits the reading end.
        let source = UNFORMATTED.repeat(8192);

        for args in [&[][..], &["--check"], &["--diff"]] {
            let output = run_bin_with(args, &source, closed_pipe(), closed_pipe());

            assert_eq!(
                output.status.code(),
                Some(1),
                "{args:?} should exit with a plain failure, not a panic"
            );
        }
    }

    #[test]
    fn file_that_is_not_utf8_reports_an_error() {
        let dirpath = temp_dir("not-utf8");
        let binary = dirpath.join("binary.S");
        let content = b"  mov x0,x1\n\xff\xfe\n";
        fs::write(&binary, content).expect("failed to write the binary file");
        let binary_arg = binary.to_str().expect("the path should be UTF-8");

        let output = run_bin(&[binary_arg], "");

        assert!(
            !output.status.success(),
            "a file that isn't text should fail"
        );
        assert!(
            stderr(&output)
                .lines()
                .any(|line| line.starts_with(&format!("[!] Error: failed to read {binary_arg}"))),
            "the error should name the file, got: {}",
            stderr(&output)
        );
        assert_eq!(
            fs::read(&binary).expect("failed to read the binary file"),
            content,
            "the file should be left as it was"
        );

        fs::remove_dir_all(&dirpath).expect("failed to remove the temporary directory");
    }

    #[test]
    fn missing_file_reports_an_error() {
        let dirpath = temp_dir("missing");
        let missing = dirpath.join("missing.S");
        let missing_arg = missing.to_str().expect("the path should be UTF-8");

        let output = run_bin(&[missing_arg], "");

        assert!(!output.status.success(), "a missing file should fail");
        assert!(
            stderr(&output)
                .lines()
                .any(|line| line.starts_with(&format!("[!] Error: failed to read {missing_arg}"))),
            "the error should name the missing file, got: {}",
            stderr(&output)
        );

        fs::remove_dir_all(&dirpath).expect("failed to remove the temporary directory");
    }
}
