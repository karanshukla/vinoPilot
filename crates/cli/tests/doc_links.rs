//! Every test a comment cites after "pinned by" must be a `#[test]` fn, so a
//! rule that names its test fails CI when the test is renamed or deleted.
//! rustdoc can't check these: `#[test]` fns are compiled out of the docs. See
//! docs/comment-style.md.
//!
//! It checks every crate's `src/`, and the integration tests docs/*.md cite.
//! A citation is one of:
//! - `` `tests::name` ``: the `tests` module of the citing file, inline or
//!   in `<file>/tests.rs`;
//! - `` `module::tests::name` ``: the same, in that crate's `src/module.rs`;
//! - `` `crates/<crate>/tests/<file>.rs::name` ``: an integration test.

use std::fs;
use std::path::{Path, PathBuf};

/// The workspace root, two up from this crate.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn every_cited_test_exists() {
    let root = root();
    let mut broken = Vec::new();
    let mut check = |file: &Path, crate_src: Option<&Path>, comments_only: bool| {
        let text = fs::read_to_string(file).unwrap();
        for (line_no, cited) in citations(&text, comments_only) {
            let in_a_crate = crate_src.is_some() || cited.contains(".rs::");
            if in_a_crate && !resolves(&root, file, crate_src, &text, cited) {
                broken.push(format!("{}:{line_no}: `{cited}`", file.display()));
            }
        }
    };
    for krate in fs::read_dir(root.join("crates")).unwrap().flatten() {
        let src = krate.path().join("src");
        for file in files(&src, "rs") {
            check(&file, Some(&src), true);
        }
    }
    for file in files(&root.join("docs"), "md") {
        check(&file, None, false);
    }
    assert!(broken.is_empty(), "no such test:\n{}", broken.join("\n"));
}

#[test]
fn the_check_finds_a_cited_test_and_rejects_a_missing_one() {
    let text = "/// (pinned by `tests::real`,\n/// `tests::helper`)\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn real() {}\n    fn helper() {}\n}\n";
    let cited: Vec<_> = citations(text, true).into_iter().map(|(_, c)| c).collect();
    assert_eq!(cited, ["tests::real", "tests::helper"]);
    assert!(is_test_fn(text, "real"));
    assert!(!is_test_fn(text, "helper"), "not a #[test]");
    assert!(!is_test_fn(text, "gone"));
    let root = root();
    let here = root.join("crates/cli/tests/doc_links.rs");
    assert!(resolves(
        &root,
        &here,
        None,
        "",
        "crates/cli/tests/doc_links.rs::every_cited_test_exists"
    ));
    assert!(!resolves(
        &root,
        &here,
        None,
        "",
        "crates/cli/tests/doc_links.rs::root"
    ));
}

fn files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files(&path, extension));
        } else if path.extension().is_some_and(|e| e == extension) {
            found.push(path);
        }
    }
    found
}

/// (line number, citation) for each test cited after "pinned by", up to
/// the closing parenthesis, which may be a few lines on. In Rust only `//`
/// lines count.
fn citations(text: &str, comments_only: bool) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut open = false;
    for (i, line) in text.lines().enumerate() {
        if comments_only && !line.trim_start().starts_with("//") {
            open = false;
            continue;
        }
        let mut rest = line;
        if !open {
            let Some(at) = rest.find("pinned by") else {
                continue;
            };
            rest = &rest[at..];
        }
        let (span, closed) = match rest.find(')') {
            Some(end) => (&rest[..end], true),
            None => (rest, false),
        };
        found.extend(cited_tests(span).into_iter().map(|c| (i + 1, c)));
        open = !closed;
    }
    found
}

/// Code spans that name a test.
fn cited_tests(span: &str) -> Vec<&str> {
    span.split('`')
        .skip(1)
        .step_by(2)
        .filter(|s| {
            (s.contains("tests::") || s.contains(".rs::"))
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_:/.-".contains(c))
        })
        .collect()
}

fn resolves(root: &Path, file: &Path, crate_src: Option<&Path>, text: &str, cited: &str) -> bool {
    if let Some((path, name)) = cited.split_once(".rs::") {
        return fs::read_to_string(root.join(format!("{path}.rs")))
            .is_ok_and(|text| is_test_fn(&text, name));
    }
    let Some(src) = crate_src else {
        return false;
    };
    let (module, name) = cited.rsplit_once("tests::").unwrap();
    let module = module.trim_end_matches("::");
    let (inline, beside) = if module.is_empty() {
        (text.to_string(), file.with_extension("").join("tests.rs"))
    } else {
        let module = src.join(module.replace("::", "/"));
        let inline = fs::read_to_string(module.with_extension("rs")).unwrap_or_default();
        (inline, module.join("tests.rs"))
    };
    is_test_fn(&inline, name)
        || fs::read_to_string(beside).is_ok_and(|text| is_test_fn(&text, name))
}

/// A `fn name(` whose attributes, above it, include `#[test]`.
fn is_test_fn(text: &str, name: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    let signature = format!("fn {name}(");
    lines.iter().enumerate().any(|(i, line)| {
        line.trim_start().starts_with(&signature)
            && lines[..i]
                .iter()
                .rev()
                .map(|l| l.trim())
                .take_while(|l| l.starts_with("#[") || l.starts_with("///"))
                .any(|l| l == "#[test]")
    })
}
