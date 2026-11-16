# Comment style

A comment is the last resort. Work down this ladder, and write prose only
when all four rungs fail.

1. **Extract it.** Logic that a comment explains belongs in a function with
   a domain name. Arithmetic spelled out in a comment belongs in named
   constants. A coercion that is explained twice belongs in one named helper.

   ```rust
   // Before
   // A copy in the home runs first; without one, vinoPilot is its package,
   // and an app its Flatpak.
   let installed = match in_home {
       None if id == Id::App(App::Pilot) => packaged(paths, system_package.as_ref()),
       None => flatpak(paths, id),
       in_home => in_home,
   };

   // After
   let installed = in_home.or_else(|| outside_home(paths, id, system_package.as_ref()));
   ```

2. **Name it.** A comment that labels a block (`// Update all: the next
   one`) means the block wants to be a function (`update_next_queued`).
   Sentinels and stubs get names too: `INFO_FINDS_SYSTEM`, not a string with
   `// installed system-wide` beside it. A field whose comment says what it
   means gets the name instead (`updated_itself`).

3. **Pin it with a test pair.** A comment that states a business rule is a
   rule nothing enforces. Write one test inside the boundary and one
   outside, named after the rule (`a_command_beside_its_upgrade_script_is_a_release`
   / `a_command_without_its_upgrade_script_is_a_source_build`). The pair is
   the point: it fails the day someone moves the rule. In a test, a comment
   that labels the next few asserts is the same smell. Split those asserts
   into their own test, named after what the comment said.

4. **Pin cross-boundary rules with an end-to-end test.** The `--json`
   shape and exit codes (docs/json-api.md) belong in `crates/cli/tests/cli.rs`,
   what `install.sh` writes in `tests/install-e2e.sh`, and what each step's
   shell does in a test that runs it, not in a paragraph.

5. **Only then, prose.** Hidden constraints, upstream bugs, incident
   history that still constrains the code, protocol requirements. State the
   constraint, not its biography: "pkexec clears the environment" is enough
   without the story of how we found out. Measured claims carry a date.

## Linking the test from the code

When rung 3 or 4 replaced a comment, the code it governs says which test
pins the rule, always as "(pinned by ...)":

```rust
/// The release's bare binary in `~/.local/bin` runs before a cargo
/// install's (pinned by `tests::a_release_ovfetch_wins_over_a_cargo_install`).
```

- `` `tests::name` ``: the citing file's `tests` module, inline or in
  `<file>/tests.rs` (`catalog.rs` and `catalog/tests.rs`).
- `` `module::tests::name` ``: another module's, in the same crate
  (`catalog::tests::name` from `catalog/steps.rs`).
- `` `crates/<crate>/tests/<file>.rs::name` ``: an integration test, from
  the code or from docs/*.md (docs/json-api.md cites
  `crates/cli/tests/cli.rs::an_action_prints_line_events_then_one_done_event`).
- In the TUI, `` `test_file.py::test_name` `` in a docstring, for a test in
  `tui/tests/`.

rustdoc can't link to `#[test]` functions (they are compiled out even
under `--cfg test`), so these are code spans, not intra-doc links.
`crates/cli/tests/doc_links.rs` and `tui/tests/test_doc_links.py` (CI job
"Doc Links") check them instead: every citation after "pinned by" must be a
`#[test]` fn, or a `def test_`, where it says.

## Exempt

- `///` and `//!` docs on items, which say what an item is and how to call
  it. They follow the same "constraint, not biography" rule, but aren't
  replaced by code. Don't restate the signature.
- `// SAFETY:` comments on `unsafe`, `#[allow]` reasons, and coverage
  pragmas.
- Comments inside the shell scripts in `catalog/scripts.rs`: they are part
  of the script `steps.snapshot` pins.
