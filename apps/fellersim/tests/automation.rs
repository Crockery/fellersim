use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};
fn invoke(args: &[&str], input: Option<&Value>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fellersim"));
    command
        .args(args)
        .arg("--json")
        .arg("--quiet")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}
fn parse_response(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "{e}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}
fn success(args: &[&str], input: Option<&Value>) -> Value {
    let output = invoke(args, input);
    let r = parse_response(&output);
    assert!(output.status.success(), "{r}");
    assert_eq!(r["ok"], true);
    assert_eq!(r["protocolVersion"], 1);
    assert!(output.stderr.is_empty());
    r
}
fn schema(name: &str) -> Value {
    success(&["schema", name], None)["data"].clone()
}
fn conforms(document: &str, instance: &Value) {
    let schema = schema(document);
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors = validator
        .iter_errors(instance)
        .map(|e| e.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{document}: {}", errors.join("\n"));
}
fn starter(hero: &str) -> Value {
    success(&["character", "init", "--hero", hero], None)["data"].clone()
}
fn request(character: Value) -> Value {
    json!({"version":1,"character":{"kind":"inline","document":character},"apl":{"kind":"default"},"options":{"iterations":100,"seed":"0123456789abcdef"}})
}
fn case(id: &str, character: Value) -> Value {
    json!({"id":id,"character":{"kind":"inline","document":character},"apl":{"kind":"default"}})
}
fn manifest(cases: Vec<Value>) -> Value {
    json!({"version":1,"options":{"iterations":100},"cases":cases})
}
#[test]
fn every_command_has_machine_output_and_response_schema() {
    let req = request(starter("firemage"));
    for (args, schema_name, input) in [
        (vec!["describe"], "describe-response", None),
        (vec!["version"], "version-response", None),
        (vec!["--version"], "version-response", None),
        (vec!["--help"], "help-response", None),
        (vec!["catalog", "heroes"], "catalog-response", None),
        (vec!["schema", "character"], "schema-response", None),
        (
            vec!["character", "init", "--hero", "firemage"],
            "character-response",
            None,
        ),
        (
            vec!["apl", "default", "--hero", "firemage"],
            "apl-default-response",
            None,
        ),
        (
            vec!["validate", "--input", "-"],
            "validate-response",
            Some(&req),
        ),
        (
            vec!["prepare", "--input", "-"],
            "prepare-response",
            Some(&req),
        ),
        (
            vec!["apl", "explain", "--input", "-"],
            "apl-explain-response",
            Some(&req),
        ),
        (
            vec!["trace", "--input", "-", "--decisions", "2"],
            "trace-response",
            Some(&req),
        ),
        (vec!["run", "--input", "-"], "run-response", Some(&req)),
    ] {
        conforms(schema_name, &success(&args, input));
    }
    let batch = manifest(vec![
        case("a", starter("firemage")),
        case("b", starter("firemage")),
    ]);
    conforms("batch", &batch);
    conforms(
        "batch-response",
        &success(&["batch", "--input", "-"], Some(&batch)),
    );
    conforms(
        "compare-response",
        &success(
            &["compare", "--input", "-", "--baseline", "a"],
            Some(&batch),
        ),
    );
}
#[test]
fn offline_discover_repair_prepare_simulate_compare_workflow_for_all_heroes() {
    let heroes = success(&["catalog", "heroes"], None)["data"]["entries"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(heroes.len(), 6);
    for hero in heroes {
        let id = hero["id"].as_str().unwrap();
        let character = starter(id);
        conforms("character", &character);
        let mut req = request(character.clone());
        conforms("request", &req);
        let validated = success(&["validate", "--input", "-"], Some(&req));
        assert_eq!(validated["data"]["valid"], true);
        let talents = success(&["catalog", "talents", "--hero", id], None);
        let talent = talents["data"]["entries"][0]["id"].clone();
        req["character"]["document"]["build"]["selectedTalentIds"] =
            json!(["bad-talent-1", "bad-talent-2"]);
        req["options"]["targets"] = json!(0);
        let failure = invoke(&["validate", "--input", "-"], Some(&req));
        assert_eq!(failure.status.code(), Some(2));
        let failure = parse_response(&failure);
        let diagnostics = failure["diagnostics"].as_array().unwrap();
        assert!(diagnostics.len() >= 3, "{failure}");
        assert!(
            diagnostics
                .iter()
                .all(|d| d["path"].is_string() && d["code"].is_string())
        );
        req["character"]["document"]["build"]["selectedTalentIds"] = json!([talent]);
        req["options"]["targets"] = json!(1);
        let prepared = success(&["prepare", "--input", "-"], Some(&req));
        assert!(
            prepared["data"]["availableAbilities"]
                .as_array()
                .unwrap()
                .len()
                > 1
        );
        let summary = success(&["run", "--input", "-"], Some(&req));
        let full = success(&["run", "--input", "-", "--detail", "full"], Some(&req));
        for key in [
            "meanDps",
            "primaryTargetDps",
            "seed",
            "profileFingerprint",
            "modelingLimitations",
            "confidenceInterval95",
        ] {
            assert_eq!(summary["data"][key], full["data"][key], "{key}");
        }
        assert!(summary["data"]["abilities"].is_null());
        assert!(full["data"]["abilities"].is_array());
        conforms("run-response", &summary);
        conforms("run-response", &full);
        let batch = manifest(vec![
            case("original", character),
            case("variant", req["character"]["document"].clone()),
        ]);
        let compared = success(
            &[
                "compare",
                "--input",
                "-",
                "--baseline",
                "original",
                "--jobs",
                "2",
            ],
            Some(&batch),
        );
        assert!(compared["data"]["cases"][1]["comparison"]["adjustedInterval"]["low"].is_number());
        // The same ID and exact token must be discoverable using exact lookup.
        let found = success(
            &[
                "catalog",
                "talents",
                "--hero",
                id,
                "--id",
                talent.as_str().unwrap(),
            ],
            None,
        );
        assert_eq!(found["data"]["total"], 1);
    }
}
#[test]
fn batch_partial_failures_order_and_comparison_compatibility() {
    let character = starter("firemage");
    let mut bad = case("bad", character.clone());
    bad["options"] = json!({"iterations":0});
    let batch = manifest(vec![
        case("base", character.clone()),
        bad,
        case("same", character.clone()),
    ]);
    let output = invoke(
        &[
            "compare",
            "--input",
            "-",
            "--baseline",
            "base",
            "--jobs",
            "4",
        ],
        Some(&batch),
    );
    assert_eq!(output.status.code(), Some(1));
    let response = parse_response(&output);
    conforms("compare-response", &response);
    assert_eq!(response["data"]["cases"][0]["status"], "completed");
    assert_eq!(response["data"]["cases"][1]["status"], "invalid");
    assert_eq!(
        response["data"]["cases"][2]["comparison"]["absoluteChange"],
        0.
    );
    assert_eq!(
        response["data"]["cases"][2]["comparison"]["classification"],
        "inconclusive"
    );
    assert_eq!(response["data"]["cases"][2]["comparison"]["comparisons"], 2);
    let invalid_base = invoke(
        &["compare", "--input", "-", "--baseline", "bad"],
        Some(&batch),
    );
    assert_eq!(invalid_base.status.code(), Some(2));
    let incompatible = manifest(vec![
        case("base", character),
        case("different", starter("rime")),
    ]);
    let output = invoke(
        &["compare", "--input", "-", "--baseline", "base"],
        Some(&incompatible),
    );
    assert_eq!(output.status.code(), Some(1));
    let response = parse_response(&output);
    assert_eq!(
        response["data"]["cases"][1]["diagnostics"][0]["code"],
        "incompatible-comparison"
    );
    let mixed = success(&["batch", "--input", "-"], Some(&incompatible));
    assert_eq!(mixed["data"]["cases"][1]["status"], "completed");
}
#[test]
fn file_stdin_relative_paths_case_order_and_concurrency_are_deterministic() {
    let temp = tempfile::tempdir().unwrap();
    let character = starter("mara");
    std::fs::write(temp.path().join("character.json"), character.to_string()).unwrap();
    let req = json!({"version":1,"character":{"kind":"file","path":"character.json"},"apl":{"kind":"default"},"options":{"iterations":100}});
    std::fs::write(temp.path().join("request.json"), req.to_string()).unwrap();
    let file = success(
        &[
            "run",
            "--input",
            temp.path().join("request.json").to_str().unwrap(),
        ],
        None,
    );
    let mut inline = req.clone();
    inline["character"] = json!({"kind":"inline","document":character});
    let stdin = success(&["run", "--input", "-"], Some(&inline));
    assert_eq!(file, stdin);
    let batch = manifest(vec![
        case("first", character.clone()),
        case("second", character),
    ]);
    let one = success(&["batch", "--input", "-", "--jobs", "1"], Some(&batch));
    let four = success(&["batch", "--input", "-", "--jobs", "4"], Some(&batch));
    assert_eq!(one, four);
    let mut reversed = batch;
    reversed["cases"].as_array_mut().unwrap().reverse();
    let reversed = success(&["batch", "--input", "-", "--jobs", "2"], Some(&reversed));
    assert_eq!(one["data"]["cases"][0], reversed["data"]["cases"][1]);
}
#[test]
fn argument_errors_and_timeouts_use_the_protocol() {
    for args in [
        vec!["run", "--nonexistent"],
        vec!["batch", "--input", "-", "--jobs", "5"],
        vec!["run", "--input", "-", "--iterations", "100"],
        vec!["run", "--apl", "a", "--default-apl"],
        vec!["trace", "--decisions", "10001"],
        vec!["catalog", "not-a-catalog"],
    ] {
        let output = invoke(&args, None);
        assert_eq!(output.status.code(), Some(2));
        conforms("response", &parse_response(&output));
    }
    let mut req = request(starter("firemage"));
    req["options"]["iterations"] = json!(100000);
    req["options"]["targets"] = json!(5);
    let output = invoke(&["run", "--input", "-", "--timeout", "1"], Some(&req));
    assert_eq!(output.status.code(), Some(124));
    assert_eq!(parse_response(&output)["diagnostics"][0]["code"], "timeout");
    let mut slow = case("slow", starter("firemage"));
    slow["options"] = req["options"].clone();
    let batch = manifest(vec![case("quick", starter("firemage")), slow.clone(), {
        slow["id"] = json!("queued");
        slow
    }]);
    let output = invoke(&["batch", "--input", "-", "--timeout", "2"], Some(&batch));
    assert_eq!(output.status.code(), Some(124));
    let response = parse_response(&output);
    assert_eq!(response["data"]["cases"][0]["status"], "completed");
    assert_eq!(response["data"]["cases"][1]["status"], "timed-out");
    assert_eq!(response["data"]["cases"][2]["status"], "not-started");
}
#[test]
fn embedded_apls_match_visible_files_and_schemas_are_stable() {
    for (hero, name) in [
        ("firemage", "ardeos"),
        ("rime", "rime"),
        ("ink", "tariq"),
        ("bowguy", "elarion"),
        ("mara", "mara"),
        ("gunde", "gunde"),
    ] {
        let source = success(&["apl", "default", "--hero", hero], None)["data"]["source"]
            .as_str()
            .unwrap()
            .to_owned();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let path = root
            .ancestors()
            .map(|p| p.join("default-apls").join(format!("{name}.apl")))
            .find(|p| p.is_file())
            .unwrap();
        assert_eq!(source, std::fs::read_to_string(path).unwrap());
    }
    for name in success(&["describe"], None)["data"]["schemas"]
        .as_array()
        .unwrap()
    {
        let name = name.as_str().unwrap();
        let a = schema(name);
        assert_eq!(a, schema(name));
        assert_eq!(a["$schema"], "https://json-schema.org/draft/2020-12/schema");
        jsonschema::validator_for(&a).unwrap();
    }
}

#[test]
fn diagnostics_cap_semantic_apl_locations_and_manifest_limits() {
    let mut req = request(starter("firemage"));
    req["apl"] = json!({"kind":"inline","source":(0..128).map(|i|format!("actions{}=/not_a_spell_{i}",if i==0 {""} else {"+"})).collect::<Vec<_>>().join("\n")});
    let output = invoke(&["validate", "--input", "-"], Some(&req));
    assert_eq!(output.status.code(), Some(2));
    let response = parse_response(&output);
    assert_eq!(response["diagnostics"].as_array().unwrap().len(), 100);
    assert_eq!(response["diagnosticsTruncated"], true);
    assert_eq!(response["diagnostics"][0]["line"], 1);
    assert_eq!(response["diagnostics"][99]["line"], 100);
    assert_eq!(response["diagnostics"][0]["path"], "/apl/source");
    let character = starter("firemage");
    let too_many = manifest(
        (0..65)
            .map(|i| case(&i.to_string(), character.clone()))
            .collect(),
    );
    let output = invoke(&["batch", "--input", "-"], Some(&too_many));
    assert_eq!(output.status.code(), Some(2));
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("oversized.json");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    let output = invoke(&["batch", "--input", path.to_str().unwrap()], None);
    assert_eq!(
        parse_response(&output)["diagnostics"][0]["code"],
        "file-too-large"
    );
    let duplicate = manifest(vec![
        case("same", character.clone()),
        case("same", character),
    ]);
    let output = invoke(&["batch", "--input", "-"], Some(&duplicate));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        parse_response(&output)["data"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["status"] == "invalid")
    );
}

#[test]
fn catalog_pagination_queries_and_starters_match_planner_examples() {
    for (hero, name) in [
        ("firemage", "ardeos"),
        ("rime", "rime"),
        ("ink", "tariq"),
        ("bowguy", "elarion"),
        ("mara", "mara"),
        ("gunde", "gunde"),
    ] {
        let document = starter(hero);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(format!("{name}.json"));
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(document, expected);
        for kind in [
            "abilities",
            "talents",
            "equipment",
            "gems",
            "traits",
            "blessings",
            "sets",
            "resources",
            "buffs",
            "target-effects",
            "apl-references",
            "apl-operators",
        ] {
            let first = success(&["catalog", kind, "--hero", hero, "--limit", "1"], None);
            if kind == "target-effects" && first["data"]["total"] == 0 {
                continue;
            }
            assert!(
                first["data"]["total"].as_u64().unwrap() > 0,
                "{hero} {kind}"
            );
            let id = first["data"]["entries"][0]["id"].as_str().unwrap();
            let exact = success(&["catalog", kind, "--hero", hero, "--id", id], None);
            assert_eq!(exact["data"]["total"], 1);
            if first["data"]["total"].as_u64().unwrap() > 1 {
                let second = success(
                    &[
                        "catalog", kind, "--hero", hero, "--limit", "1", "--offset", "1",
                    ],
                    None,
                );
                assert_ne!(
                    second["data"]["entries"][0]["id"],
                    first["data"]["entries"][0]["id"]
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn cancelled_batch_preserves_completed_cases_and_queued_status() {
    use std::io::{BufRead, BufReader};
    let temp = tempfile::tempdir().unwrap();
    let character = starter("firemage");
    let mut slow = case("active", character.clone());
    slow["options"] = json!({"iterations":100000,"targets":5});
    let mut queued = slow.clone();
    queued["id"] = json!("queued");
    let document = manifest(vec![case("complete", character), slow, queued]);
    let path = temp.path().join("batch.json");
    std::fs::write(&path, document.to_string()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fellersim"))
        .args(["batch", "--input", path.to_str().unwrap(), "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let reader = BufReader::new(child.stderr.take().unwrap());
    for line in reader.lines() {
        if line.unwrap().contains("Starting case active") {
            break;
        }
    }
    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));
    let response = parse_response(&output);
    assert_eq!(response["data"]["cases"][0]["status"], "completed");
    assert_eq!(response["data"]["cases"][1]["status"], "cancelled");
    assert_eq!(response["data"]["cases"][2]["status"], "not-started");
}
