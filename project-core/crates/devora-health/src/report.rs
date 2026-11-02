//! Parse Debi's health JSON and derive the presentation shared by both UIs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    Ok,
    Warn,
    Error,
    Info,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub version: String,
    pub overall_state: HealthState,
    pub overall_label: String,
    pub tiles: Vec<HealthTile>,
    pub sections: Vec<HealthSection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthTile {
    pub label: String,
    pub met: u32,
    pub total: u32,
    pub state: HealthState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSection {
    pub title: String,
    pub count: Option<String>,
    pub state: HealthState,
    pub items: Vec<HealthItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthItem {
    pub name: String,
    pub state: HealthState,
    pub message: String,
    pub version: Option<String>,
    pub path: Option<String>,
    pub fix_hint: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawReport {
    version: String,
    config: FileCheck,
    completion: FileCheck,
    required: Vec<Dependency>,
    optional: Vec<Dependency>,
    credentials: Vec<Credential>,
    summary: Summary,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileCheck {
    path: String,
    found: bool,
    fix_hint: Option<String>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    found: bool,
    version: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Credential {
    name: String,
    status: String,
    message: String,
    fix_hint: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    required_met: u32,
    required_total: u32,
    optional_met: u32,
    optional_total: u32,
    credentials_met: u32,
    credentials_total: u32,
}

fn count_state(met: u32, total: u32, required: bool) -> HealthState {
    if met >= total {
        HealthState::Ok
    } else if required {
        HealthState::Error
    } else {
        HealthState::Warn
    }
}

fn tile(label: &str, met: u32, total: u32, required: bool) -> HealthTile {
    HealthTile {
        label: label.into(),
        met,
        total,
        state: count_state(met, total, required),
    }
}

fn dependencies(title: &str, dependencies: Vec<Dependency>, required: bool) -> HealthSection {
    let met = dependencies.iter().filter(|d| d.found).count() as u32;
    let total = dependencies.len() as u32;
    let items = dependencies
        .into_iter()
        .map(|d| HealthItem {
            name: d.name,
            state: if d.found {
                HealthState::Ok
            } else if required {
                HealthState::Error
            } else {
                HealthState::Warn
            },
            message: if d.found {
                String::new()
            } else {
                "not found".into()
            },
            version: d.found.then(|| {
                if d.version.is_empty() {
                    "—".into()
                } else {
                    d.version
                }
            }),
            path: d.found.then_some(d.path),
            fix_hint: None,
        })
        .collect();
    HealthSection {
        title: title.into(),
        count: Some(format!("{met} / {total}")),
        state: count_state(met, total, required),
        items,
    }
}

/// Return a report ready for both clients, or identify the executable whose JSON did not match.
pub fn parse_report(json: &str, executable: &str) -> Result<HealthReport, String> {
    let raw: RawReport = serde_json::from_str(json).map_err(|e| {
        format!("Health output from {executable} did not match the expected format: {e}")
    })?;
    let s = raw.summary;
    let (overall_state, overall_label) = if s.required_met < s.required_total {
        (HealthState::Error, "Missing required")
    } else if s.optional_met < s.optional_total
        || s.credentials_met < s.credentials_total
        || !raw.completion.found
    {
        (HealthState::Warn, "Issues found")
    } else {
        (HealthState::Ok, "Healthy")
    };
    let tiles = vec![
        tile("Required", s.required_met, s.required_total, true),
        tile("Optional", s.optional_met, s.optional_total, false),
        tile("Credentials", s.credentials_met, s.credentials_total, false),
        tile(
            "Setup",
            u32::from(raw.config.found) + u32::from(raw.completion.found),
            2,
            false,
        ),
    ];
    let required = dependencies("Required dependencies", raw.required, true);
    let optional = dependencies("Optional dependencies", raw.optional, false);
    let credentials = HealthSection {
        title: "Credentials".into(),
        count: Some(format!("{} / {}", s.credentials_met, s.credentials_total)),
        state: count_state(s.credentials_met, s.credentials_total, false),
        items: raw
            .credentials
            .into_iter()
            .map(|c| HealthItem {
                name: c.name,
                state: match c.status.as_str() {
                    "ok" => HealthState::Ok,
                    "failed" => HealthState::Error,
                    "unchecked" => HealthState::Warn,
                    _ => HealthState::Info,
                },
                message: c.message,
                version: None,
                path: None,
                fix_hint: c.fix_hint.filter(|s| !s.is_empty()),
            })
            .collect(),
    };
    let config = HealthItem {
        name: "config".into(),
        state: if raw.config.found {
            HealthState::Ok
        } else {
            HealthState::Warn
        },
        message: if raw.config.found {
            "found"
        } else {
            "not found"
        }
        .into(),
        path: Some(raw.config.path),
        version: None,
        fix_hint: None,
    };
    let completion = HealthItem {
        name: "zsh completion".into(),
        state: if raw.completion.found {
            HealthState::Ok
        } else {
            HealthState::Warn
        },
        message: if raw.completion.found {
            "installed"
        } else {
            "not installed"
        }
        .into(),
        path: raw.completion.found.then_some(raw.completion.path),
        version: None,
        fix_hint: if raw.completion.found {
            None
        } else {
            raw.completion.fix_hint.filter(|s| !s.is_empty())
        },
    };
    let configuration = HealthSection {
        title: "Configuration".into(),
        count: None,
        state: HealthState::Ok,
        items: vec![config, completion],
    };
    Ok(HealthReport {
        version: raw.version,
        overall_state,
        overall_label: overall_label.into(),
        tiles,
        sections: vec![required, optional, credentials, configuration],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../../acceptance/fixtures/health-report.json");

    #[test]
    fn captured_report_derives_tiles_sections_and_hints() {
        let report = parse_report(FIXTURE, "fixture-debi").unwrap();
        assert_eq!(report.overall_state, HealthState::Warn);
        assert_eq!(report.overall_label, "Issues found");
        assert_eq!((report.tiles[0].met, report.tiles[0].total), (3, 3));
        assert_eq!((report.tiles[3].met, report.tiles[3].total), (1, 2));
        assert_eq!(report.sections[0].title, "Required dependencies");
        assert_eq!(
            report.sections[3].items[1].fix_hint.as_deref(),
            Some("debi completion zsh > ~/.zsh/completions/_debi")
        );
    }

    #[test]
    fn missing_required_takes_precedence_and_errors_name_the_executable() {
        let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        value["summary"]["requiredMet"] = 2.into();
        assert_eq!(
            parse_report(&value.to_string(), "debi")
                .unwrap()
                .overall_state,
            HealthState::Error
        );
        let error = parse_report("{}", "/fixture/debi").unwrap_err();
        assert!(error.contains("/fixture/debi"));
        assert!(error.contains("did not match"));
    }
}
