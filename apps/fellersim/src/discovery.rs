use crate::{command::Cli, input::*, output::*};
use clap::CommandFactory;
use schemars::JsonSchema;
use serde_json::{Value, json};

pub const SCHEMAS: &[&str] = &[
    "character",
    "request",
    "batch",
    "response",
    "run-response",
    "validate-response",
    "prepare-response",
    "trace-response",
    "batch-response",
    "compare-response",
    "version-response",
    "catalog-response",
    "describe-response",
    "schema-response",
    "character-response",
    "apl-default-response",
    "apl-explain-response",
    "help-response",
];
fn generated<T: JsonSchema>() -> Value {
    let settings = schemars::generate::SchemaSettings::draft2020_12();
    serde_json::to_value(settings.into_generator().into_root_schema_for::<T>()).unwrap()
}
pub fn schema(name: &str) -> Result<Value, Failure> {
    Ok(match name {
        "character" => generated::<CharacterFile>(),
        "request" => generated::<Request>(),
        "batch" => generated::<Manifest>(),
        "run-response" => generated::<Response<SimulationReport>>(),
        "version-response" => generated::<Response<Versions>>(),
        "character-response" => generated::<Response<CharacterFile>>(),
        "batch-response" | "compare-response" => generated::<Response<BatchReport>>(),
        "validate-response" => generated::<Response<ValidationReport>>(),
        "trace-response" => generated::<Response<TraceReport>>(),
        "prepare-response" | "apl-explain-response" => generated::<Response<PreparationReport>>(),
        "apl-default-response" => generated::<Response<DefaultAplReport>>(),
        "catalog-response" => generated::<Response<CatalogReport>>(),
        "response" | "describe-response" | "schema-response" | "help-response" => {
            generated::<Response<Value>>()
        }
        _ => {
            return Err(Failure::input(
                "unknown-schema",
                format!("Schemas: {}.", SCHEMAS.join(", ")),
                "/document",
            ));
        }
    })
}
pub fn describe(path: Option<&str>) -> Result<Value, Failure> {
    let mut command = Cli::command();
    command.build();
    if let Some(path) = path {
        for part in path.split_whitespace() {
            command = command
                .find_subcommand(part)
                .ok_or_else(|| {
                    Failure::input(
                        "unknown-command",
                        format!("Unknown command: {path}"),
                        "/command",
                    )
                })?
                .clone();
        }
    }
    fn describe_command(command: &clap::Command) -> Value {
        json!({"name":command.get_name(),"about":command.get_about().map(ToString::to_string),
            "arguments":command.get_arguments().map(|a| json!({
                "id":a.get_id().as_str(),"long":a.get_long(),"short":a.get_short().map(|c|c.to_string()),"help":a.get_help().map(ToString::to_string),
                "required":a.is_required_set(),"global":a.is_global_set(),
                "defaults":a.get_default_values().iter().map(|s|s.to_string_lossy()).collect::<Vec<_>>(),
                "action":format!("{:?}",a.get_action()),
                "choices":a.get_value_parser().possible_values().map(|values| values.map(|v|v.get_name().to_owned()).collect::<Vec<_>>()),
                "conflicts":command.get_arg_conflicts_with(a).iter().map(|a|a.get_id().as_str()).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),
            "commands":command.get_subcommands().map(describe_command).collect::<Vec<_>>()})
    }
    Ok(
        json!({"command":describe_command(&command),"schemas":SCHEMAS,"catalogs":crate::catalog::KINDS,
        "limits":{"characterBytes":FILE_LIMIT,"aplBytes":FILE_LIMIT,"manifestBytes":MANIFEST_LIMIT,"batchCases":64,"workers":4,"iterations":{"min":100,"max":100000,"default":10000},"targets":{"min":1,"max":20,"default":1},"diagnostics":100,"traceDecisions":{"default":200,"max":10000},"traceSeconds":{"default":10,"max":300}},
        "exitCodes":{"0":"success","1":"execution or mixed batch failure","2":"argument or input validation","124":"timeout","130":"cancelled"},
        "examples":["fellersim catalog heroes --json","fellersim character init --hero firemage > character.json","fellersim validate --character character.json --default-apl --json","fellersim prepare --character character.json --default-apl --json","fellersim run --character character.json --default-apl --iterations 100 --json","fellersim compare --input variants.json --baseline original --json"]}),
    )
}
