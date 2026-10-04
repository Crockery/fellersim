use serde_json::Value;
use std::{path::PathBuf, process::Command};

fn example(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
        .to_string_lossy()
        .into_owned()
}
fn default_apl(name: &str) -> String {
    // The public export places default-apls at the repository root; the monorepo
    // keeps it with the CLI. Both layouts expose it under a manifest ancestor.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|parent| parent.join("default-apls").join(name))
        .find(|path| path.is_file())
        .expect("default APL must be present in the source checkout")
        .to_string_lossy()
        .into_owned()
}
fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fellersim"))
        .args(arguments)
        .output()
        .unwrap()
}

fn help(arguments: &[&str]) -> String {
    let output = run(arguments);
    assert!(output.status.success(), "{arguments:?}: {output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn root_help_includes_the_complete_command_reference() {
    let text = help(&["--help"]);
    assert_eq!(help(&["-h"]), text);
    assert_eq!(help(&["help"]), text);
    assert!(text.contains("An action priority list (APL)"));
    assert!(text.contains("Use --json and --quiet with any command"));
    assert!(text.contains("fellersim describe --json"));

    let output = run(&["describe", "--json"]);
    assert!(output.status.success());
    let discovery: Value = serde_json::from_slice(&output.stdout).unwrap();
    fn check_commands(command: &Value, path: &str, text: &str) {
        for child in command["commands"].as_array().unwrap() {
            let name = child["name"].as_str().unwrap();
            if name == "help" {
                continue;
            }
            let path = format!("{path} {name}");
            let heading = format!("Usage: {path}");
            let section = text
                .split_once(&heading)
                .unwrap_or_else(|| panic!("Missing {heading}"))
                .1
                .split("\nUsage:")
                .next()
                .unwrap();
            let about = child["about"].as_str().expect("command description");
            assert!(!about.is_empty(), "{path}");
            assert!(section.contains(about), "{path}: {about}");
            for arg in child["arguments"].as_array().unwrap() {
                if arg["global"] == true || arg["id"] == "help" {
                    continue;
                }
                let description = arg["help"].as_str().expect("argument description");
                assert!(!description.is_empty(), "{path}: {arg}");
                assert!(section.contains(description), "{path}: {description}");
                if let Some(flag) = arg["long"].as_str() {
                    assert!(section.contains(&format!("--{flag}")), "{path}: {flag}");
                }
            }
            check_commands(child, &path, text);
        }
    }
    check_commands(&discovery["data"]["command"], "fellersim", &text);
    for flag in ["--json", "--quiet"] {
        assert_eq!(
            text.lines()
                .filter(|line| line.trim_start().starts_with(&format!("{flag} ")))
                .count(),
            1,
            "{flag} should be documented once"
        );
    }
}

#[test]
fn command_help_stays_focused_and_includes_shared_options() {
    for (path, expected) in [
        (vec!["run"], "Set how many fights to simulate"),
        (vec!["describe"], "Quote nested names"),
        (vec!["catalog"], "Find an entry with this exact ID"),
        (vec!["character"], "Create a character JSON file"),
        (vec!["character", "init"], "Choose a hero by ID"),
        (vec!["apl", "default"], "Choose a hero by ID"),
        (vec!["apl", "explain"], "Read ability rules"),
        (vec!["trace"], "Record at most this many ability decisions"),
        (
            vec!["compare"],
            "Compare every other case with this case ID",
        ),
    ] {
        let text = help(&[path.as_slice(), &["--help"]].concat());
        assert_eq!(help(&[&["help"], path.as_slice()].concat()), text);
        assert!(text.contains(expected), "{path:?}: {text}");
        assert!(text.contains("--json"));
        assert!(text.contains("--quiet"));
        assert!(!text.contains("Command reference:"));
    }
}

#[test]
fn json_help_preserves_the_envelope_and_human_help_text() {
    for args in [
        vec!["--help"],
        vec!["help"],
        vec!["describe", "--help"],
        vec!["character", "init", "--help"],
        vec!["help", "apl", "explain"],
    ] {
        let text = help(&args);
        let mut invocations = vec![[&["--json"], args.as_slice()].concat()];
        // Clap's help subcommand accepts command names only after `help`.
        if args[0] != "help" {
            invocations.push([args.as_slice(), &["--json"]].concat());
        }
        for json_args in invocations {
            let output = run(&json_args);
            assert!(output.status.success(), "{json_args:?}: {output:?}");
            assert!(output.stderr.is_empty());
            let response: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(response["ok"], true);
            assert_eq!(response["protocolVersion"], 1);
            assert_eq!(response["command"], "help");
            assert_eq!(
                response["data"]["text"].as_str().unwrap().trim(),
                text.trim()
            );
            assert!(response["data"]["discovery"]["command"].is_object());
        }
    }
}

#[test]
fn runs_all_heroes_offline_with_clean_json_output() {
    for name in ["ardeos", "rime", "tariq", "elarion", "mara", "gunde"] {
        let validated = run(&[
            "validate",
            "--character",
            &example(&format!("{name}.json")),
            "--apl",
            &default_apl(&format!("{name}.apl")),
            "--json",
        ]);
        assert!(
            validated.status.success(),
            "{}",
            String::from_utf8_lossy(&validated.stderr)
        );
        let validation: Value = serde_json::from_slice(&validated.stdout).unwrap();
        assert_eq!(validation["data"]["valid"], true);
        let output = run(&[
            "run",
            "--character",
            &example(&format!("{name}.json")),
            "--apl",
            &default_apl(&format!("{name}.apl")),
            "--iterations",
            "100",
            "--json",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["data"]["iterations"], 100);
        assert!(result["data"]["meanDps"].as_f64().unwrap() > 0.0);
        assert_eq!(result["versions"]["simulator"], env!("CARGO_PKG_VERSION"));
        assert_eq!(result["data"]["dataBuildId"], fellersim_data::build_id());
    }
}
#[test]
fn rejects_invalid_character_files_with_file_context() {
    let temp = tempfile::tempdir().unwrap();
    let export: Value =
        serde_json::from_str(&std::fs::read_to_string(example("ardeos.json")).unwrap()).unwrap();
    let changed = |pointer: &str, value: Value| {
        let mut file = export.clone();
        *file.pointer_mut(pointer).unwrap() = value;
        file.to_string()
    };
    let mut cases = vec![
        ("bare-build", export["build"].to_string(), "unknown field"),
        (
            "wrong-format",
            changed("/format", "another-planner".into()),
            "unknown variant",
        ),
        ("wrong-version", changed("/version", 7.into()), "version 6"),
        (
            "string-version",
            changed("/version", "6".into()),
            "invalid type",
        ),
        ("null-build", changed("/build", Value::Null), "invalid type"),
        (
            "malformed-build",
            changed("/build/positions", "invalid".into()),
            "invalid type",
        ),
        ("malformed-json", "{".into(), "EOF"),
        (
            "oversized",
            format!("{}{}", export, " ".repeat(1024 * 1024)),
            "exceeds 1 MiB",
        ),
    ];
    for field in ["format", "version", "build"] {
        let mut file = export.clone();
        file.as_object_mut().unwrap().remove(field);
        cases.push((field, file.to_string(), "missing field"));
    }
    let mut unknown_field = export.clone();
    unknown_field["unexpected"] = true.into();
    cases.push(("unknown-field", unknown_field.to_string(), "unknown field"));
    let mut unknown_build_field = export.clone();
    unknown_build_field["build"]["unexpected"] = true.into();
    cases.push((
        "unknown-build-field",
        unknown_build_field.to_string(),
        "unknown field",
    ));
    for (name, contents, expected) in cases {
        let path = temp.path().join(format!("{name}.json"));
        std::fs::write(&path, contents).unwrap();
        for command in ["validate", "run"] {
            let output = run(&[
                command,
                "--character",
                path.to_str().unwrap(),
                "--apl",
                &default_apl("ardeos.apl"),
                "--json",
            ]);
            assert!(!output.status.success(), "{command}: {name}");
            assert_eq!(output.status.code(), Some(2));
            let response: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(response["ok"], false);
            let error = response["diagnostics"].to_string();
            assert_eq!(response["diagnostics"][0]["source"].as_str(), path.to_str());
            assert!(error.contains(expected), "{command}: {name}: {error}");
        }
    }
}
#[test]
fn validates_nested_build_schema_and_selections() {
    let temp = tempfile::tempdir().unwrap();
    let export: Value =
        serde_json::from_str(&std::fs::read_to_string(example("ardeos.json")).unwrap()).unwrap();
    for (field, value, expected) in [
        ("schemaVersion", Value::from(5), "schemaVersion 6"),
        ("heroId", Value::from("unknown-hero"), "supported hero"),
        (
            "selectedTalentIds",
            serde_json::json!(["unknown-talent"]),
            "Unknown or duplicate talent",
        ),
    ] {
        let mut file = export.clone();
        file["build"][field] = value;
        let path = temp.path().join(format!("{field}.json"));
        std::fs::write(&path, file.to_string()).unwrap();
        for command in ["validate", "run"] {
            let output = run(&[
                command,
                "--character",
                path.to_str().unwrap(),
                "--apl",
                &default_apl("ardeos.apl"),
                "--json",
            ]);
            assert!(!output.status.success(), "{command}: {field}");
            assert_eq!(output.status.code(), Some(2));
            let response: Value = serde_json::from_slice(&output.stdout).unwrap();
            let error = response["diagnostics"].to_string();
            assert!(error.contains(expected), "{command}: {field}: {error}");
        }
    }
}
#[test]
fn flags_override_config_and_invalid_inputs_exit_unsuccessfully() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.json");
    std::fs::write(
        &config,
        r#"{"iterations":100001,"targets":3,"seed":"0000000000000001"}"#,
    )
    .unwrap();
    let character = example("ardeos.json");
    let apl = default_apl("ardeos.apl");
    let output = run(&[
        "run",
        "--character",
        &character,
        "--apl",
        &apl,
        "--config",
        config.to_str().unwrap(),
        "--iterations",
        "100",
        "--targets",
        "1",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["iterations"], 100);
    assert_eq!(result["data"]["scenario"]["targetCount"], 1);
    assert_eq!(result["data"]["seed"], "0000000000000001");
    let output = run(&[
        "validate",
        "--character",
        &character,
        "--apl",
        &apl,
        "--config",
        config.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let output = run(&["run", "--character", "missing.json", "--apl", &apl]);
    assert!(!output.status.success());
}
#[test]
fn explicit_seed_matches_library_and_default_seed_ignores_comments() {
    use fellersim_core::{preparation::*, simulate_owned};
    use std::sync::{Arc, atomic::AtomicBool};
    let character = example("ardeos.json");
    let apl = default_apl("ardeos.apl");
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&character).unwrap()).unwrap();
    let input = SimulationInput {
        schema_version: 1,
        run_id: "cli".into(),
        data_build_id: fellersim_data::build_id().into(),
        character: serde_json::from_value(file["build"].clone()).unwrap(),
        apl_source: std::fs::read_to_string(&apl).unwrap(),
        options: SimulationOptions {
            iterations: 100,
            targets: 1,
            seed: Some("0000000000000001".into()),
        },
    };
    let expected = simulate_owned(
        prepare(&input).unwrap(),
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    let output = run(&[
        "run",
        "--character",
        &character,
        "--apl",
        &apl,
        "--iterations",
        "100",
        "--seed",
        "0000000000000001",
        "--json",
        "--detail",
        "full",
    ]);
    assert!(output.status.success());
    let mut actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    actual = actual["data"].take();
    actual
        .as_object_mut()
        .unwrap()
        .remove("modelingLimitations");
    actual
        .as_object_mut()
        .unwrap()
        .remove("evidenceFingerprint");
    assert_eq!(actual, serde_json::to_value(expected).unwrap());
    let mut a = input.clone();
    a.options.seed = None;
    let mut b = a.clone();
    b.run_id = "different".into();
    b.apl_source = format!("# new comment\n{}\n# trailing", b.apl_source);
    assert_eq!(prepare(&a).unwrap().seed, prepare(&b).unwrap().seed);
}
#[cfg(unix)]
#[test]
fn ctrl_c_cancels_without_a_success_result() {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_fellersim"))
        .args([
            "run",
            "--character",
            &example("ardeos.json"),
            "--apl",
            &default_apl("ardeos.apl"),
            "--iterations",
            "100000",
            "--targets",
            "5",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    stderr.read_line(&mut line).unwrap();
    assert!(line.contains("Iterations"));
    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["diagnostics"][0]["code"], "cancelled");
}
