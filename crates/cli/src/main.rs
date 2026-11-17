//! `vinopilot`: the vinoPilot apps from a terminal. With `--json` it is also
//! the API other frontends drive: `list`/`check` print one JSON document,
//! and the actions print one JSON event per line.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use vinopilot_core::catalog::{self, Action, Channel, Id};
use vinopilot_core::job::{Env, Job, Outcome};
use vinopilot_core::status::{self, Update};
use vinopilot_core::{Config, detect, remote};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
vinopilot: install, update and uninstall the vinoPilot apps

Usage: vinopilot [--source-root DIR] [--json] <command>

Commands:
  list                       what's installed, and how (offline)
  check                      list, plus the newest version of each (online)
  install <app> [--via WAY]  WAY: release (default), source, pypi, crates or
                             flatpak (vinowhisper, vinosearch)
  update <app>... | --all    update in the way each is installed
  uninstall <app> [--yes]
  steps <app> <install|update|uninstall> [--via WAY]
                             print the shell an action runs, without running it
  config [source_root [DIR]] show or set where source checkouts go

Apps: vinowhisper vinoauthface vinoopeneffects vinopop vinospeak vinosearch
      vinowatch ovfetch vinouserinterface vinopilot (the 'vino' is optional)

--json: list and check print one JSON document; install, update and
uninstall print one JSON object per line: {\"event\":\"line\",\"text\":...}
for output, then {\"event\":\"done\",...}.
";

struct Args {
    json: bool,
    yes: bool,
    all: bool,
    via: Option<Channel>,
    source_root: Option<PathBuf>,
    rest: Vec<String>,
}

fn parse_args(raw: impl Iterator<Item = String>) -> Result<Args, String> {
    use lexopt::prelude::*;

    let mut args = Args {
        json: false,
        yes: false,
        all: false,
        via: None,
        source_root: None,
        rest: Vec::new(),
    };
    let mut parser = lexopt::Parser::from_args(raw);
    while let Some(option) = parser.next().map_err(|e| e.to_string())? {
        match option {
            Long("json") => args.json = true,
            Long("yes") | Short('y') => args.yes = true,
            Long("all") => args.all = true,
            Long("via") => {
                let way = option_value(&mut parser, "--via")?;
                args.via = Some(Channel::parse(&way).ok_or_else(|| {
                    format!("--via {way}: use release, source, pypi, crates or flatpak")
                })?);
            }
            Long("source-root") => {
                args.source_root = Some(PathBuf::from(option_value(&mut parser, "--source-root")?));
            }
            Long("help") | Short('h') => args.rest.insert(0, "help".into()),
            Long("version") | Short('V') => args.rest.insert(0, "version".into()),
            Long(name) => return Err(format!("unknown option --{name} (see --help)")),
            Short(letter) => return Err(format!("unknown option -{letter} (see --help)")),
            Value(operand) => args.rest.push(operand.to_string_lossy().into_owned()),
        }
    }
    Ok(args)
}

/// The value of `name`, after a space or an equals sign.
fn option_value(parser: &mut lexopt::Parser, name: &str) -> Result<String, String> {
    parser
        .value()
        .map(|value| value.to_string_lossy().into_owned())
        .map_err(|_| format!("{name} needs a value"))
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("vinopilot: {e}");
            return ExitCode::from(2);
        }
    };
    let mut config = Config::load();
    if let Some(root) = &args.source_root {
        config.source_root = root.clone();
    }
    let command = args.rest.first().map(String::as_str).unwrap_or("help");
    let operands = &args.rest[args.rest.len().min(1)..];
    let result = match command {
        "help" => {
            print!("{HELP}");
            Ok(())
        }
        "version" => {
            println!("vinopilot {VERSION}");
            Ok(())
        }
        "list" => list(&config, &args, false),
        "check" => list(&config, &args, true),
        "install" => single_app(operands).and_then(|id| install(&config, &args, id)),
        "update" => update(&config, &args, operands),
        "uninstall" => single_app(operands).and_then(|id| uninstall(&config, &args, id)),
        "steps" => steps(&config, &args, operands),
        "config" => config_cmd(&mut config, &args, operands),
        other => Err(Failure::Usage(format!(
            "unknown command {other} (see --help)"
        ))),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(e)) => {
            eprintln!("vinopilot: {e}");
            ExitCode::from(2)
        }
        Err(Failure::Failed(e)) => {
            if !e.is_empty() {
                eprintln!("✗ {e}");
            }
            ExitCode::FAILURE
        }
    }
}

enum Failure {
    Usage(String),
    Failed(String),
}

fn parse_app(name: &str) -> Result<Id, Failure> {
    Id::parse(name).ok_or_else(|| Failure::Usage(format!("no app called {name} (see --help)")))
}

fn single_app(operands: &[String]) -> Result<Id, Failure> {
    match operands {
        [name] => parse_app(name),
        [] => Err(Failure::Usage("name an app (see --help)".into())),
        _ => Err(Failure::Usage("one app at a time".into())),
    }
}

fn components(config: &Config, online: bool) -> Vec<status::Component> {
    let paths = config.paths();
    let mut local = detect::scan(&paths);
    let mut latest = if online {
        remote::check(&paths)
    } else {
        Default::default()
    };
    Id::ALL
        .into_iter()
        .map(|id| {
            status::component(
                id,
                local.remove(&id).unwrap_or_default(),
                latest.remove(&id),
            )
        })
        .collect()
}

fn list(config: &Config, args: &Args, online: bool) -> Result<(), Failure> {
    let all = components(config, online);
    if args.json {
        print_document(config, &all);
    } else {
        print_table(&all, online);
        print_duplicate_installs(&all);
    }
    Ok(())
}

/// `list --json` and `check --json`: docs/json-api.md (pinned by
/// `crates/cli/tests/cli.rs::every_component_has_every_field`).
fn print_document(config: &Config, components: &[status::Component]) {
    let doc = serde_json::json!({
        "vinopilot": VERSION,
        "source_root": config.source_root,
        "components": components,
    });
    println!("{}", serde_json::to_string_pretty(&doc).unwrap());
}

fn print_table(components: &[status::Component], online: bool) {
    println!(
        "{:<18} {:<12} {:<10} {}",
        "APP",
        "VERSION",
        "VIA",
        if online { "UPDATE" } else { "" }
    );
    for component in components {
        let (version, via) = match &component.local.installed {
            Some(installed) => (
                installed.version.clone().unwrap_or_else(|| "?".into()),
                installed
                    .channel
                    .label(Id::parse(&component.id).unwrap())
                    .to_string(),
            ),
            None => ("-".into(), "-".into()),
        };
        let update = update_column(
            &component.update,
            online,
            component.local.installed.is_some(),
        );
        println!(
            "{:<18} {:<12} {:<10} {}",
            component.name, version, via, update
        );
    }
}

/// The UPDATE column: empty offline, and for what isn't installed.
fn update_column(update: &Update, online: bool, installed: bool) -> String {
    if !online || !installed {
        return String::new();
    }
    match update {
        Update::Available { to } => format!("{to} available"),
        Update::UpToDate => "up to date".into(),
        Update::Unknown { reason } => reason.clone().unwrap_or_else(|| "?".into()),
    }
}

/// vinoPilot's own `.deb` or `.rpm` is how it's installed, not a second copy.
fn is_own_package(channel: Option<Channel>) -> bool {
    matches!(channel, Some(Channel::Deb | Channel::Rpm))
}

/// Under the table: distro packages vinopilot doesn't remove, and commands
/// installed twice.
fn print_duplicate_installs(components: &[status::Component]) {
    for component in components {
        let channel = component.local.installed.as_ref().map(|i| i.channel);
        if let Some(system) = &component.local.system_package
            && !is_own_package(channel)
        {
            println!(
                "\n{} is also a distro package, which vinopilot doesn't remove: {}",
                component.name, system.remove
            );
        }
        for shadowed in &component.local.shadowed {
            println!(
                "\n{} is installed twice: {} runs, {} doesn't; uninstall and reinstall it to keep one",
                component.name,
                shadowed.runs.display(),
                shadowed.hidden.display()
            );
        }
    }
}

/// Runs an action, printing its output as it comes. With `--json`, a line
/// event per line, then one done event; the exit code says the same (pinned
/// by `crates/cli/tests/cli.rs::an_action_prints_line_events_then_one_done_event`,
/// `crates/cli/tests/cli.rs::a_failed_action_gives_the_installers_cross_line_as_its_reason_and_exits_1`).
fn run(
    config: &Config,
    args: &Args,
    id: Id,
    channel: Channel,
    action: Action,
) -> Result<(), Failure> {
    let steps = catalog::steps(id, channel, action);
    if steps.is_empty() {
        return Err(Failure::Usage(format!(
            "{} can't be {} from {}",
            id.name(),
            past_tense(action),
            channel.label(id)
        )));
    }
    let json = args.json;
    if !json {
        println!("== {} {} ({})", action.verb(), id.name(), channel.label(id));
    }
    let notify: vinopilot_core::job::Notify = Arc::new(move |line| {
        let Some(line) = line else { return };
        let mut out = std::io::stdout().lock();
        if json {
            let _ = writeln!(
                out,
                "{}",
                serde_json::json!({"event": "line", "text": line})
            );
        } else {
            let _ = writeln!(out, "{line}");
        }
    });
    let env = Env::for_id(id, &config.paths());
    let outcome = Job::start(id, channel, action, steps, env, notify).wait();
    if json {
        println!(
            "{}",
            serde_json::json!({"event": "done", "app": id.slug(), "channel": channel, "result": outcome})
        );
    }
    match outcome {
        Outcome::Done => Ok(()),
        Outcome::Failed(why) => Err(Failure::Failed(if json { String::new() } else { why })),
        Outcome::Stopped | Outcome::Running => Err(Failure::Failed("stopped".into())),
    }
}

fn past_tense(action: Action) -> &'static str {
    match action {
        Action::Install => "installed",
        Action::Update => "updated",
        Action::Uninstall => "uninstalled",
    }
}

fn parse_action(name: &str) -> Result<Action, Failure> {
    match name {
        "install" => Ok(Action::Install),
        "update" => Ok(Action::Update),
        "uninstall" => Ok(Action::Uninstall),
        other => Err(Failure::Usage(format!("no action called {other}"))),
    }
}

fn installed_channel(config: &Config, id: Id) -> Option<Channel> {
    detect::scan(&config.paths())
        .remove(&id)
        .and_then(|local| local.installed)
        .map(|installed| installed.channel)
}

fn install(config: &Config, args: &Args, id: Id) -> Result<(), Failure> {
    let channels = id.channels();
    let channel = args.via.unwrap_or(channels[0]);
    if !channels.contains(&channel) {
        let ways: Vec<_> = channels.iter().map(|c| c.label(id)).collect();
        return Err(Failure::Usage(format!(
            "{} installs from {}",
            id.name(),
            ways.join(", ")
        )));
    }
    run(config, args, id, channel, Action::Install)
}

fn update(config: &Config, args: &Args, operands: &[String]) -> Result<(), Failure> {
    let ids: Vec<Id> = if args.all {
        if !operands.is_empty() {
            return Err(Failure::Usage("--all or app names, not both".into()));
        }
        let todo: Vec<Id> = components(config, true)
            .into_iter()
            .filter(|component| matches!(component.update, Update::Available { .. }))
            .filter_map(|component| Id::parse(&component.id))
            .collect();
        if todo.is_empty() && !args.json {
            println!("Everything installed is up to date.");
        }
        todo
    } else if operands.is_empty() {
        return Err(Failure::Usage("name an app, or --all (see --help)".into()));
    } else {
        operands
            .iter()
            .map(|name| parse_app(name))
            .collect::<Result<_, _>>()?
    };
    let mut failed = Vec::new();
    for id in ids {
        let Some(channel) = installed_channel(config, id) else {
            eprintln!("✗ {} isn't installed", id.name());
            failed.push(id.name());
            continue;
        };
        if run(config, args, id, channel, Action::Update).is_err() {
            failed.push(id.name());
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(Failure::Failed(format!(
            "didn't update: {}",
            failed.join(", ")
        )))
    }
}

/// Asks on the terminal, after saying what goes and what stays. Without a
/// terminal, `--yes` is required (pinned by
/// `crates/cli/tests/cli.rs::uninstall_without_yes_or_a_terminal_is_a_usage_error`).
fn confirm_uninstall(id: Id, channel: Channel) -> Result<(), Failure> {
    if !std::io::stdin().is_terminal() {
        return Err(Failure::Usage(
            "pass --yes to uninstall without a terminal".into(),
        ));
    }
    eprintln!("{}", catalog::uninstall_note(id, channel));
    eprint!("Uninstall {} ({})? [y/N] ", id.name(), channel.label(id));
    let mut answer = String::new();
    let _ = std::io::stdin().read_line(&mut answer);
    if matches!(answer.trim(), "y" | "Y" | "yes") {
        Ok(())
    } else {
        Err(Failure::Failed("left it installed".into()))
    }
}

fn uninstall(config: &Config, args: &Args, id: Id) -> Result<(), Failure> {
    let local = detect::scan(&config.paths())
        .remove(&id)
        .unwrap_or_default();
    let system = local.system_package.map(|p| p.remove);
    let channel = match (args.via, local.installed.map(|i| i.channel), &system) {
        (Some(via), ..) => via,
        (None, Some(found), _) => found,
        (None, None, Some(remove)) => {
            return Err(Failure::Failed(format!(
                "{} is only installed by your package manager: {remove}",
                id.name()
            )));
        }
        (None, None, None) => {
            return Err(Failure::Failed(format!("{} isn't installed", id.name())));
        }
    };
    if !args.yes {
        confirm_uninstall(id, channel)?;
    }
    run(config, args, id, channel, Action::Uninstall)?;
    if let Some(remove) = system
        && !args.json
        && !is_own_package(Some(channel))
    {
        println!(
            "{} is still a distro package. Remove it with: {remove}",
            id.name()
        );
    }
    Ok(())
}

fn steps(config: &Config, args: &Args, operands: &[String]) -> Result<(), Failure> {
    let [name, action] = operands else {
        return Err(Failure::Usage(
            "steps <app> <install|update|uninstall>".into(),
        ));
    };
    let id = parse_app(name)?;
    let action = parse_action(action)?;
    let channel = args
        .via
        .or_else(|| installed_channel(config, id))
        .unwrap_or(id.channels()[0]);
    let env = Env::for_id(id, &config.paths());
    println!(
        "# {} {} via {}",
        action.verb(),
        id.name(),
        channel.label(id)
    );
    println!(
        "# CHECKOUT={} REPO_URL={} RELEASE={}",
        env.checkout.display(),
        env.repo_url,
        env.release
    );
    for step in catalog::steps(id, channel, action) {
        println!(
            "# --- as {}",
            if step.root { "root (pkexec)" } else { "you" }
        );
        println!("{}", step.script);
    }
    Ok(())
}

fn config_cmd(config: &mut Config, args: &Args, operands: &[String]) -> Result<(), Failure> {
    match operands {
        [] => {
            if args.json {
                println!("{}", serde_json::json!({"source_root": config.source_root}));
            } else {
                println!("source_root = {}", config.source_root.display());
            }
            Ok(())
        }
        [key] if key == "source_root" => {
            println!("{}", config.source_root.display());
            Ok(())
        }
        [key, value] if key == "source_root" => {
            config.source_root =
                vinopilot_core::config::expand(value, &vinopilot_core::config::home());
            config
                .save()
                .map_err(|e| Failure::Failed(format!("couldn't save the config: {e}")))?;
            println!("source_root = {}", config.source_root.display());
            Ok(())
        }
        _ => Err(Failure::Usage("config [source_root [DIR]]".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(words: &[&str]) -> Result<Args, String> {
        parse_args(words.iter().map(|word| word.to_string()))
    }

    #[test]
    fn options_are_read_wherever_they_are_and_the_rest_keeps_its_order() {
        let args = parse(&["--json", "update", "-y", "whisper", "--all", "pop"]).unwrap();
        assert!(args.json && args.yes && args.all);
        assert_eq!(args.rest, ["update", "whisper", "pop"]);
    }

    #[test]
    fn an_option_takes_its_value_after_a_space_or_an_equals_sign() {
        for words in [
            ["install", "whisper", "--via", "pypi"].as_slice(),
            ["install", "whisper", "--via=pypi"].as_slice(),
        ] {
            assert_eq!(parse(words).unwrap().via, Some(Channel::Package));
        }
        let args = parse(&["--source-root=/src", "list"]).unwrap();
        assert_eq!(args.source_root, Some(PathBuf::from("/src")));
    }

    #[test]
    fn a_missing_or_unknown_value_is_refused() {
        assert_eq!(parse(&["--via"]).err().unwrap(), "--via needs a value");
        assert_eq!(
            parse(&["--via", "snap"]).err().unwrap(),
            "--via snap: use release, source, pypi, crates or flatpak"
        );
        assert_eq!(
            parse(&["--nope"]).err().unwrap(),
            "unknown option --nope (see --help)"
        );
    }

    #[test]
    fn a_double_dash_ends_the_options() {
        let args = parse(&["--json", "install", "--", "--all", "-y"]).unwrap();
        assert!(args.json && !args.all && !args.yes);
        assert_eq!(args.rest, ["install", "--all", "-y"]);
    }

    #[test]
    fn short_options_cluster() {
        let args = parse(&["uninstall", "pop", "-yh"]).unwrap();
        assert!(args.yes);
        assert_eq!(args.rest, ["help", "uninstall", "pop"]);
        assert_eq!(
            parse(&["-yx"]).err().unwrap(),
            "unknown option -x (see --help)"
        );
    }

    #[test]
    fn a_value_on_an_option_that_takes_none_is_refused() {
        assert_eq!(
            parse(&["--json=x", "list"]).err().unwrap(),
            "unexpected argument for option '--json': \"x\""
        );
        assert!(parse(&["--json", "list"]).is_ok());
    }

    #[test]
    fn help_and_version_become_the_command() {
        assert_eq!(parse(&["list", "-h"]).unwrap().rest, ["help", "list"]);
        assert_eq!(parse(&["--version"]).unwrap().rest, ["version"]);
    }

    #[test]
    fn the_update_column_is_empty_offline_and_for_what_is_not_installed() {
        let available = Update::Available { to: "v2".into() };
        assert_eq!(update_column(&available, true, true), "v2 available");
        assert_eq!(update_column(&available, false, true), "");
        assert_eq!(update_column(&available, true, false), "");
        assert_eq!(update_column(&Update::UpToDate, true, true), "up to date");
        let unknown = Update::Unknown { reason: None };
        assert_eq!(update_column(&unknown, true, true), "?");
    }
}
