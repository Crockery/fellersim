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
            assert!(error.contains(path.to_str().unwrap()), "{error}");
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
