//! Execute the examples readers copy, including their stated outcomes.
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::{Command, Stdio},
};

fn docs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|p| p.join("docs/apl"))
        .find(|p| p.join("Home.md").is_file())
        .expect("APL documentation in monorepo and public export")
}

fn cli(arguments: &[&str], input: Option<&Value>) -> (i32, Value) {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_fellersim"))
        .args(arguments)
        .args(["--json", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("invalid JSON: {}", String::from_utf8_lossy(&output.stdout)));
    (output.status.code().unwrap(), value)
}

fn request(hero: &str, source: &str, targets: u64) -> Value {
    let (code, character) = cli(&["character", "init", "--hero", hero], None);
    assert_eq!(code, 0, "{character}");
    json!({"version":1,"character":{"kind":"inline","document":character["data"]},
        "apl":{"kind":"inline","source":source},
        "options":{"targets":targets,"iterations":100,"seed":"0123456789abcdef"}})
}

fn check_example(source: &str, spec: &Value, location: &str) {
    let request = request(
        spec["hero"].as_str().unwrap(),
        source,
        spec["targets"].as_u64().unwrap_or(1),
    );
    let (code, validation) = cli(&["validate", "--input", "-"], Some(&request));
    if let Some(error) = spec["error"].as_str() {
        assert_eq!(code, 2, "{location}: {validation}");
        assert_eq!(validation["ok"], false);
        let diagnostics = validation["diagnostics"].as_array().unwrap();
        assert!(
            diagnostics
                .iter()
                .any(|d| d["message"].as_str().is_some_and(|m| m.contains(error))),
            "{location}: expected {error:?}: {validation}"
        );
        assert!(diagnostics.iter().any(|d| d["line"].as_u64().is_some()));
        return;
    }
    assert_eq!(code, 0, "{location}: {validation}");
    assert_eq!(validation["ok"], true, "{location}: {validation}");
    if let Some(statuses) = spec.get("statuses") {
        let (code, explanation) = cli(&["apl", "explain", "--input", "-"], Some(&request));
        assert_eq!(code, 0, "{location}: {explanation}");
        let actual: Vec<_> = explanation["data"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["status"].clone())
            .collect();
        assert_eq!(&json!(actual), statuses, "{location}");
    }
    if spec.get("firstAction").is_none() {
        return;
    }
    let seconds = spec["seconds"].as_u64().unwrap_or(10).to_string();
    let (code, trace) = cli(
        &[
            "trace",
            "--input",
            "-",
            "--seconds",
            &seconds,
            "--decisions",
            "10000",
        ],
        Some(&request),
    );
    assert_eq!(code, 0, "{location}: {trace}");
    let decisions = trace["data"]["decisions"].as_array().unwrap();
    let expected_action = if let Some(id) = spec["firstAction"].as_str() {
        let (code, catalog) = cli(
            &[
                "catalog",
                "abilities",
                "--hero",
                spec["hero"].as_str().unwrap(),
                "--id",
                id,
            ],
            None,
        );
        assert_eq!(code, 0, "{catalog}");
        catalog["data"]["entries"][0]["gameAbilityId"].clone()
    } else {
        Value::Null
    };
    assert_eq!(
        decisions[0]["selectedAbility"], expected_action,
        "{location}"
    );
    let rules: Vec<_> = decisions
        .iter()
        .flat_map(|d| d["rules"].as_array().unwrap())
        .collect();
    if let Some(blocker) = spec.get("blocker") {
        assert!(rules.iter().any(|r| &r["blocker"] == blocker), "{location}");
    }
    let nodes: Vec<_> = rules
        .iter()
        .flat_map(|r| r["nodes"].as_array().unwrap())
        .collect();
    if spec["shortCircuit"] == true {
        assert!(
            nodes.iter().any(|n| n["shortCircuited"] == true),
            "{location}"
        );
    }
    if let Some(values) = spec.get("observedValues") {
        assert!(
            nodes.iter().any(|n| &n["observedValues"] == values),
            "{location}"
        );
    }
    if let Some(time) = spec.get("firstCastAtMs") {
        let cast = decisions.iter().find(|d| !d["selectedAbility"].is_null());
        assert_eq!(&cast.expect(location)["timeMs"], time, "{location}");
    }
}

#[test]
fn documented_apls_validate_and_behave_as_described() {
    let mut examples = 0;
    for entry in std::fs::read_dir(docs()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let document = std::fs::read_to_string(&path).unwrap();
        let mut lines = document.lines().enumerate();
        let mut pending = None;
        while let Some((number, line)) = lines.next() {
            if let Some(spec) = line.strip_prefix("<!-- apl-test ") {
                assert!(pending.is_none(), "unused example metadata: {path:?}");
                pending = Some(
                    serde_json::from_str::<Value>(spec.strip_suffix(" -->").unwrap()).unwrap(),
                );
            } else if line == "```apl" {
                let spec = pending.take().expect("every APL block has test metadata");
                let mut source = String::new();
                let mut closed = false;
                for (_, line) in lines.by_ref() {
                    if line == "```" {
                        closed = true;
                        break;
                    }
                    source.push_str(line);
                    source.push('\n');
                }
                assert!(closed, "unclosed APL block: {path:?}");
                check_example(
                    &source,
                    &spec,
                    &format!("{}:{}", path.display(), number + 1),
                );
                examples += 1;
            }
        }
        assert!(pending.is_none(), "unused example metadata: {path:?}");
    }
    assert!(examples >= 25, "expected complete documentation examples");
}

#[test]
fn documented_query_examples_cover_discovery_and_validate() {
    let source = std::fs::read_to_string(docs().join("Checking-combat-state.md")).unwrap();
    let mut covered = BTreeSet::new();
    let mut checked = 0;
    for line in source.lines().filter(|l| l.starts_with("| `")) {
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        let pattern = columns[1].trim_matches('`');
        let condition = columns[4].trim_matches('`');
        let apl = format!("actions=/infernal_wave,if={condition}");
        check_example(&apl, &json!({"hero":"firemage"}), pattern);
        covered.insert(pattern.to_owned());
        checked += 1;
    }
    assert_eq!(checked, 22);
    // Discover all heroes: a newly exposed query family needs a reference row.
    let mut offset = 0;
    loop {
        let offset_text = offset.to_string();
        let (code, catalog) = cli(
            &[
                "catalog",
                "apl-references",
                "--limit",
                "500",
                "--offset",
                &offset_text,
            ],
            None,
        );
        assert_eq!(code, 0, "{catalog}");
        for entry in catalog["data"]["entries"].as_array().unwrap() {
            let token = entry["token"].as_str().unwrap();
            let parts: Vec<_> = token.split('.').collect();
            let placeholder = match parts[0] {
                "cooldown" | "dot" => Some("ACTION"),
                "resource" => Some("RESOURCE"),
                "buff" => Some("BUFF"),
                "debuff" => Some("EFFECT"),
                "talent" => Some("TALENT"),
                "legendary" => Some("ITEM"),
                _ => None,
            };
            let pattern = placeholder.map_or_else(
                || token.to_owned(),
                |name| format!("{}.{name}.{}", parts[0], parts[2]),
            );
            assert!(covered.contains(&pattern), "undocumented query: {token}");
        }
        match catalog["data"]["nextOffset"].as_u64() {
            None => break,
            Some(next) => {
                assert!(next > offset);
                offset = next;
            }
        }
    }
}

#[test]
fn getting_started_commands_work_in_an_empty_directory() {
    let directory = tempfile::tempdir().unwrap();
    let document = std::fs::read_to_string(docs().join("Getting-started.md")).unwrap();
    let mut lines = document.lines();
    let mut commands = 0;
    let mut simulated = false;
    while let Some(line) = lines.next() {
        if line == "```apl" {
            let source: Vec<_> = lines.by_ref().take_while(|line| *line != "```").collect();
            std::fs::write(directory.path().join("rotation.apl"), source.join("\n")).unwrap();
        } else if line == "```sh" {
            for line in lines.by_ref().take_while(|line| *line != "```") {
                let words: Vec<_> = line.split_whitespace().collect();
                assert_eq!(words.first(), Some(&"fellersim"), "{line}");
                let redirect = words.iter().position(|word| *word == ">");
                let end = redirect.unwrap_or(words.len());
                let output = Command::new(env!("CARGO_BIN_EXE_fellersim"))
                    .args(&words[1..end])
                    .current_dir(directory.path())
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{line}: {} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                if let Some(index) = redirect {
                    assert_eq!(words.len(), index + 2);
                    std::fs::write(directory.path().join(words[index + 1]), &output.stdout)
                        .unwrap();
                }
                if words.contains(&"--json") {
                    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert_eq!(value["ok"], true, "{line}: {value}");
                    if words[1] == "run" {
                        assert_eq!(value["data"]["iterations"], 1000);
                        assert!(value["data"]["meanDps"].as_f64().unwrap() > 0.0);
                        simulated = true;
                    }
                }
                commands += 1;
            }
        }
    }
    assert_eq!(commands, 9);
    assert!(simulated);
}
