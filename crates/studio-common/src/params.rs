use app_server_protocol::{
    ParameterDefinition, ParameterEntry, ParameterKind, ParameterValue, ParsedParameters,
};
use regex::Regex;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParameterStore {
    items: Vec<ParameterEntry>,
}

pub fn parse_parameters(source: &str) -> ParsedParameters {
    let mut parsed = ParsedParameters::default();
    let mut current_group = None;
    let mut hidden = false;
    for line in source.lines() {
        if let Some(group) = parse_group_header(line) {
            hidden = group == "Hidden";
            current_group = (!hidden).then_some(group);
            continue;
        }
        if let Some(result) = parse_parameter_line(line, current_group.clone(), hidden) {
            match result {
                Ok(item) => parsed.items.push(item),
                Err(message) => parsed.warnings.push(message),
            }
        }
    }
    parsed
}

impl ParameterStore {
    pub fn from_parsed(parsed: ParsedParameters) -> Self {
        Self {
            items: parsed
                .items
                .into_iter()
                .map(|definition| ParameterEntry {
                    value: definition.default_value.clone(),
                    definition,
                })
                .collect(),
        }
    }

    pub fn merge_reparsed(&mut self, reparsed: ParsedParameters) {
        let previous = self
            .items
            .iter()
            .map(|entry| (entry.definition.name.clone(), entry.value.clone()))
            .collect::<BTreeMap<_, _>>();
        self.items = reparsed
            .items
            .into_iter()
            .map(|definition| ParameterEntry {
                value: previous
                    .get(&definition.name)
                    .cloned()
                    .unwrap_or_else(|| definition.default_value.clone()),
                definition,
            })
            .collect();
    }

    pub fn set_value(&mut self, name: &str, value: ParameterValue) -> Result<(), String> {
        self.items
            .iter_mut()
            .find(|entry| entry.definition.name == name)
            .map(|entry| entry.value = value)
            .ok_or_else(|| format!("未找到参数 {name}"))?;
        Ok(())
    }

    pub fn restore_default(&mut self, name: &str) -> Result<(), String> {
        self.items
            .iter_mut()
            .find(|entry| entry.definition.name == name)
            .map(|entry| entry.value = entry.definition.default_value.clone())
            .ok_or_else(|| format!("未找到参数 {name}"))?;
        Ok(())
    }

    pub fn value(&self, name: &str) -> Option<&ParameterValue> {
        self.items
            .iter()
            .find(|entry| entry.definition.name == name)
            .map(|entry| &entry.value)
    }

    pub fn cli_defines(&self) -> Vec<String> {
        self.items
            .iter()
            .map(|entry| format!("{}={}", entry.definition.name, format_value(&entry.value)))
            .collect()
    }

    pub fn current_values(&self) -> BTreeMap<String, ParameterValue> {
        self.items
            .iter()
            .map(|entry| (entry.definition.name.clone(), entry.value.clone()))
            .collect()
    }

    pub fn entries(&self) -> &[ParameterEntry] {
        &self.items
    }
}

fn parse_group_header(line: &str) -> Option<String> {
    group_regex()
        .captures(line)
        .and_then(|caps| caps.name("name"))
        .map(|value| value.as_str().trim().to_string())
}

fn parse_parameter_line(
    line: &str,
    group: Option<String>,
    hidden: bool,
) -> Option<Result<ParameterDefinition, String>> {
    parse_number_parameter(line, group.clone(), hidden)
        .or_else(|| parse_bool_parameter(line, group.clone(), hidden))
        .or_else(|| parse_choice_parameter(line, group, hidden))
}

fn parse_number_parameter(
    line: &str,
    group: Option<String>,
    hidden: bool,
) -> Option<Result<ParameterDefinition, String>> {
    let caps = number_regex().captures(line)?;
    let name = capture(&caps, "name")?;
    let value = capture(&caps, "value")?.parse().ok()?;
    let min = capture(&caps, "min").and_then(|raw| raw.parse().ok());
    let step = capture(&caps, "step").and_then(|raw| raw.parse().ok());
    let max = capture(&caps, "max").and_then(|raw| raw.parse().ok());
    Some(Ok(ParameterDefinition {
        name,
        group,
        hidden,
        kind: ParameterKind::Number { min, step, max },
        default_value: ParameterValue::Number(value),
    }))
}

fn parse_bool_parameter(
    line: &str,
    group: Option<String>,
    hidden: bool,
) -> Option<Result<ParameterDefinition, String>> {
    let caps = bool_regex().captures(line)?;
    Some(Ok(ParameterDefinition {
        name: capture(&caps, "name")?,
        group,
        hidden,
        kind: ParameterKind::Bool,
        default_value: ParameterValue::Bool(capture(&caps, "value")? == "true"),
    }))
}

fn parse_choice_parameter(
    line: &str,
    group: Option<String>,
    hidden: bool,
) -> Option<Result<ParameterDefinition, String>> {
    let caps = choice_regex().captures(line)?;
    let options = capture(&caps, "options")?
        .split(',')
        .map(|value| value.trim().trim_matches('"').to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    Some(Ok(ParameterDefinition {
        name: capture(&caps, "name")?,
        group,
        hidden,
        kind: ParameterKind::Choice { options },
        default_value: ParameterValue::Text(capture(&caps, "value")?.to_string()),
    }))
}

fn capture(caps: &regex::Captures<'_>, name: &str) -> Option<String> {
    caps.name(name).map(|value| value.as_str().to_string())
}

fn format_value(value: &ParameterValue) -> String {
    match value {
        ParameterValue::Number(number) => format_number(*number),
        ParameterValue::Bool(value) => value.to_string(),
        ParameterValue::Text(text) => format!("\"{text}\""),
    }
}

fn format_number(number: f64) -> String {
    let text = format!("{number:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn group_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"/\*\s*\[(?P<name>[^\]]+)\]\s*\*/").expect("group regex"))
}

fn number_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"^\s*(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?P<value>-?\d+(?:\.\d+)?)\s*;\s*(?://\s*\[\s*(?P<min>-?\d+(?:\.\d+)?)\s*:\s*(?P<step>-?\d+(?:\.\d+)?)\s*:\s*(?P<max>-?\d+(?:\.\d+)?)\s*\])?\s*$"#,
        )
        .expect("number regex")
    })
}

fn bool_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"^\s*(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?P<value>true|false)\s*;\s*(?://.*)?$"#,
        )
        .expect("bool regex")
    })
}

fn choice_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"^\s*(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*"(?P<value>[^"]*)"\s*;\s*//\s*\[(?P<options>[^\]]+)\]\s*$"#,
        )
        .expect("choice regex")
    })
}
