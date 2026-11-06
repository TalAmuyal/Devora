//! The template's inputs and where each comes from (ADR-012): the launch spec,
//! the built-in default role, the global or profile config, or the space itself. The creation form lists them with [`fields`], and
//! creation renders them with the form's values on top.
use crate::{template, CreationForm};
use devora_config::agent::{self, Scope};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) const DEFAULT_ROLE: &str = "General assistant";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Launch,
    Default,
    Global,
    Profile,
    /// Describes the space itself; read-only in the form.
    Derived,
}

/// One placeholder of the template, as the creation form shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    /// `None` until the form fills it; a derived value needs the space's name.
    pub value: Option<String>,
    /// `None` for a field only the form can fill.
    pub source: Option<Source>,
}

pub(crate) struct Inputs {
    pub template: String,
    pub values: BTreeMap<String, (String, Source)>,
}

/// The inputs before the form: the launch spec, then the default role, then the
/// global and profile config, each overriding the previous one per key.
pub(crate) fn prefilled(
    profile: &Path,
    global_config: &Path,
    launch_inputs: BTreeMap<String, String>,
) -> Result<Inputs, String> {
    let config = agent::read(global_config, profile)?;
    let mut values: BTreeMap<_, _> = launch_inputs
        .into_iter()
        .map(|(key, value)| (key, (value, Source::Launch)))
        .collect();
    values
        .entry("agent.role".into())
        .or_insert_with(|| (DEFAULT_ROLE.into(), Source::Default));
    for (key, (value, scope)) in config.inputs {
        let source = match scope {
            Scope::Global => Source::Global,
            Scope::Profile => Source::Profile,
        };
        values.insert(key, (value, source));
    }
    let template = match config.template_file {
        Some(file) => std::fs::read_to_string(&file)
            .map_err(|e| format!("couldn't read the agent template {}: {e}", file.display()))?,
        None => template::DEFAULT.into(),
    };
    Ok(Inputs { template, values })
}

pub(crate) const DERIVED_KEYS: [&str; 4] =
    ["agent.name", "space.path", "memory.path", "space.repos"];

/// What only the space supplies, in [`DERIVED_KEYS`] order: its name, its paths
/// and its initial repos.
pub(crate) fn derived(space: &Path, form: &CreationForm) -> [String; 4] {
    [
        form.name.clone(),
        space.to_string_lossy().into_owned(),
        space.join("memory").to_string_lossy().into_owned(),
        form.repos.join("\n"),
    ]
}

/// The template's placeholders for the creation form: the ones only the form can
/// fill first, then the pre-filled ones, then the derived ones, each group in
/// template order. `form` supplies the derived values once it has a name.
pub fn fields(
    profile: &Path,
    global_config: &Path,
    launch_inputs: BTreeMap<String, String>,
    form: &CreationForm,
) -> Result<Vec<Field>, String> {
    let profile = std::fs::canonicalize(profile).map_err(crate::db::storage)?;
    let inputs = prefilled(&profile, global_config, launch_inputs)?;
    let derived = (!form.name.is_empty())
        .then(|| derived(&profile.join("agent-spaces").join(&form.name), form));
    let mut fields: Vec<Field> = template::placeholders(&inputs.template)?
        .into_iter()
        .map(|key| {
            if let Some(index) = DERIVED_KEYS.iter().position(|k| *k == key) {
                let value = derived.as_ref().map(|d| d[index].clone());
                return Field {
                    key,
                    value,
                    source: Some(Source::Derived),
                };
            }
            match inputs.values.get(&key) {
                Some((value, source)) => Field {
                    value: Some(value.clone()),
                    source: Some(*source),
                    key,
                },
                None => Field {
                    key,
                    value: None,
                    source: None,
                },
            }
        })
        .collect();
    fields.sort_by_key(|field| match field.source {
        None => 0,
        Some(Source::Derived) => 2,
        Some(_) => 1,
    });
    Ok(fields)
}
