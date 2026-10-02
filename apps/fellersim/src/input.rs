use crate::{command::Input, output::Failure};
use fellersim_core::{Diagnostic, preparation::*};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub const FILE_LIMIT: usize = 1024 * 1024;
pub const MANIFEST_LIMIT: usize = 64 * 1024 * 1024;
include!(concat!(env!("OUT_DIR"), "/default_apls.rs"));

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum PlannerFormat {
    #[serde(rename = "fellership-character-planner")]
    Planner,
}
/// The unmodified export from the current Fellership Character Planner.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharacterFile {
    pub format: PlannerFormat,
    #[schemars(range(min = 6, max = 6))]
    pub version: u32,
    pub build: CharacterBuild,
}
impl CharacterFile {
    pub fn new(build: CharacterBuild) -> Self {
        Self {
            format: PlannerFormat::Planner,
            version: 6,
            build,
        }
    }
    pub fn validate(self) -> Result<CharacterBuild, Failure> {
        if self.version != 6 {
            return Err(Failure::input(
                "unsupported-character-version",
                "Expected planner export version 6.",
                "/version",
            ));
        }
        Ok(self.build)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CharacterSource {
    File { path: PathBuf },
    Inline { document: CharacterFile },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AplSource {
    File {
        path: PathBuf,
    },
    Inline {
        #[schemars(length(max = 1048576))]
        source: String,
    },
    Default,
}
/// Complete CLI request. Paths are relative to the containing file, or cwd for stdin.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    pub character: CharacterSource,
    pub apl: Option<AplSource>,
    #[serde(default)]
    pub options: SimulationOptions,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptionOverrides {
    #[schemars(range(min = 100, max = 100000))]
    pub iterations: Option<u32>,
    #[schemars(range(min = 1, max = 20))]
    pub targets: Option<u32>,
    #[schemars(regex(pattern = "^[0-9a-f]{16}$"))]
    pub seed: Option<String>,
}
impl OptionOverrides {
    pub fn apply(&self, options: &mut SimulationOptions) {
        if let Some(v) = self.iterations {
            options.iterations = v;
        }
        if let Some(v) = self.targets {
            options.targets = v;
        }
        if let Some(v) = &self.seed {
            options.seed = Some(v.clone());
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Case {
    #[schemars(length(min = 1, max = 128))]
    pub id: String,
    pub character: CharacterSource,
    pub apl: AplSource,
    #[serde(default)]
    pub options: OptionOverrides,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    #[serde(default)]
    pub options: SimulationOptions,
    // Deserialize each case separately so one malformed case cannot discard valid cases.
    #[schemars(with = "Vec<Case>", length(min = 1, max = 64))]
    pub cases: Vec<serde_json::Value>,
}
pub struct Loaded {
    pub character: CharacterBuild,
    pub apl: Option<String>,
    pub options: SimulationOptions,
    pub character_source: String,
    pub apl_source: String,
    pub character_prefix: String,
    pub apl_pointer: Option<String>,
    pub options_source: String,
    pub options_prefix: String,
    pub argument_options: Vec<&'static str>,
}
impl Loaded {
    pub fn simulation_input(&self) -> Result<SimulationInput, Failure> {
        let apl_source = self.apl.clone().ok_or_else(|| {
            Failure::input(
                "missing-apl",
                "Supply --apl FILE or --default-apl, or an APL source in the request.",
                "/apl",
            )
        })?;
        Ok(SimulationInput {
            schema_version: INPUT_SCHEMA_VERSION,
            run_id: "cli".into(),
            data_build_id: fellersim_data::build_id().into(),
            character: self.character.clone(),
            apl_source,
            options: self.options.clone(),
        })
    }
    pub fn locate(&self, error: fellersim_core::SimulationError) -> Failure {
        let mut failure = Failure::from(error);
        for d in &mut failure.diagnostics {
            if d.engine_code == Some(fellersim_core::SimulationErrorCode::InvalidActionPriorityList)
            {
                d.source = Some(self.apl_source.clone());
                d.path = self.apl_pointer.clone();
            } else if d
                .path
                .as_deref()
                .is_some_and(|p| p.starts_with("/options/"))
            {
                if self
                    .argument_options
                    .iter()
                    .any(|field| d.path.as_deref() == Some(&format!("/options/{field}")))
                {
                    d.source = Some("arguments".into());
                    d.path = d.path.take().map(|p| p["/options".len()..].to_owned());
                    continue;
                }
                d.source = Some(self.options_source.clone());
                d.path = d
                    .path
                    .take()
                    .map(|p| format!("{}{}", self.options_prefix, &p["/options".len()..]));
            } else {
                d.source = Some(self.character_source.clone());
                d.path = d.path.take().map(|p| {
                    let relative = p.strip_prefix("/character/build").unwrap_or(&p);
                    format!("{}{relative}", self.character_prefix)
                });
            }
        }
        failure
    }
    pub fn prepare(&self) -> Result<fellersim_core::SimulationRequest, Failure> {
        prepare(&self.simulation_input()?).map_err(|e| self.locate(e))
    }
}
pub fn read(path: &Path, limit: usize) -> Result<String, Failure> {
    let mut contents = String::new();
    let input: Box<dyn Read> = if path == Path::new("-") {
        Box::new(std::io::stdin())
    } else {
        Box::new(
            std::fs::File::open(path)
                .map_err(|e| Failure::input("file-read", e.to_string(), "").source(path))?,
        )
    };
    input
        .take(limit as u64 + 1)
        .read_to_string(&mut contents)
        .map_err(|e| Failure::input("file-read", e.to_string(), "").source(path))?;
    if contents.len() > limit {
        return Err(Failure::input(
            "file-too-large",
            format!("Input exceeds {} MiB.", limit / FILE_LIMIT),
            "",
        )
        .source(path));
    }
    Ok(contents)
}
pub fn decode<T: DeserializeOwned>(source: &str, path: &Path) -> Result<T, Failure> {
    let mut de = serde_json::Deserializer::from_str(source);
    let value = serde_path_to_error::deserialize(&mut de).map_err(|e| {
        use serde_path_to_error::Segment;
        let pointer = e
            .path()
            .iter()
            .map(|s| match s {
                Segment::Seq { index } => index.to_string(),
                Segment::Map { key } => key.replace('~', "~0").replace('/', "~1"),
                Segment::Enum { variant } => variant.clone(),
                Segment::Unknown => String::new(),
            })
            .fold(String::new(), |p, s| format!("{p}/{s}"));
        let mut d = Diagnostic::error("invalid-json", e.inner().to_string())
            .at(pointer)
            .help("Use fellersim schema to inspect the accepted document.");
        d.source = Some(path.display().to_string());
        d.line = Some(e.inner().line() as u32);
        d.column = Some(e.inner().column() as u32);
        Failure {
            exit: 2,
            diagnostics: vec![d],
            truncated: false,
        }
    })?;
    de.end()
        .map_err(|e| Failure::input("invalid-json", e.to_string(), "").source(path))?;
    Ok(value)
}
pub fn character(path: &Path) -> Result<CharacterBuild, Failure> {
    decode::<CharacterFile>(&read(path, FILE_LIMIT)?, path)?
        .validate()
        .map_err(|e| e.source(path))
}
pub fn base(path: &Path) -> PathBuf {
    if path == Path::new("-") {
        PathBuf::from(".")
    } else {
        path.parent().unwrap_or(Path::new(".")).to_owned()
    }
}
pub fn resolve(
    character_source: &CharacterSource,
    apl_source: Option<&AplSource>,
    options: SimulationOptions,
    directory: &Path,
    source: &Path,
) -> Result<Loaded, Failure> {
    let (character, character_origin) = match character_source {
        CharacterSource::File { path } => {
            let path = directory.join(path);
            (character(&path)?, path.display().to_string())
        }
        CharacterSource::Inline { document } => {
            if serde_json::to_vec(document).unwrap().len() > FILE_LIMIT {
                return Err(Failure::input(
                    "file-too-large",
                    "Inline character exceeds 1 MiB.",
                    "/character",
                )
                .source(source));
            }
            (
                document.clone().validate().map_err(|mut e| {
                    for d in &mut e.diagnostics {
                        d.path = d.path.take().map(|p| format!("/character/document{p}"));
                    }
                    e.source(source)
                })?,
                source.display().to_string(),
            )
        }
    };
    let (apl, apl_origin) = match apl_source {
        Some(AplSource::File { path }) => {
            let path = directory.join(path);
            (Some(read(&path, FILE_LIMIT)?), path.display().to_string())
        }
        Some(AplSource::Inline { source: text }) => {
            if text.len() > FILE_LIMIT {
                return Err(
                    Failure::input("file-too-large", "Inline APL exceeds 1 MiB.", "/apl")
                        .source(source),
                );
            }
            (Some(text.clone()), source.display().to_string())
        }
        Some(AplSource::Default) => (
            Some(
                default_apl(&character.hero_id)
                    .ok_or_else(|| {
                        Failure::input(
                            "unsupported-hero",
                            "No default APL for this hero.",
                            "/build/heroId",
                        )
                    })?
                    .into(),
            ),
            format!("embedded:{}", character.hero_id),
        ),
        None => (None, source.display().to_string()),
    };
    Ok(Loaded {
        character,
        apl,
        options,
        character_source: character_origin,
        apl_source: apl_origin,
        character_prefix: match character_source {
            CharacterSource::File { .. } => "/build",
            CharacterSource::Inline { .. } => "/character/document/build",
        }
        .into(),
        apl_pointer: matches!(apl_source, Some(AplSource::Inline { .. }))
            .then(|| "/apl/source".into()),
        options_source: source.display().to_string(),
        options_prefix: "/options".into(),
        argument_options: vec![],
    })
}
pub fn load(input: &Input) -> Result<Loaded, Failure> {
    if let Some(path) = &input.input {
        let request: Request = decode(&read(path, MANIFEST_LIMIT)?, path)?;
        if request.version != 1 {
            return Err(Failure::input(
                "unsupported-request-version",
                "Expected request version 1.",
                "/version",
            )
            .source(path));
        }
        return resolve(
            &request.character,
            request.apl.as_ref(),
            request.options,
            &base(path),
            path,
        );
    }
    let path = input.character.as_ref().ok_or_else(|| {
        Failure::input(
            "missing-character",
            "Supply --character FILE or --input FILE|-.",
            "/character",
        )
    })?;
    let mut options = match &input.config {
        Some(path) => decode(&read(path, FILE_LIMIT)?, path)?,
        None => SimulationOptions::default(),
    };
    OptionOverrides {
        iterations: input.iterations,
        targets: input.targets,
        seed: input.seed.clone(),
    }
    .apply(&mut options);
    let apl = if input.default_apl {
        Some(AplSource::Default)
    } else {
        input.apl.clone().map(|path| AplSource::File { path })
    };
    let mut loaded = resolve(
        &CharacterSource::File { path: path.clone() },
        apl.as_ref(),
        options,
        Path::new("."),
        path,
    )?;
    loaded.options_source = input
        .config
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "arguments".into());
    loaded.options_prefix.clear();
    if input.iterations.is_some() {
        loaded.argument_options.push("iterations");
    }
    if input.targets.is_some() {
        loaded.argument_options.push("targets");
    }
    if input.seed.is_some() {
        loaded.argument_options.push("seed");
    }
    Ok(loaded)
}
