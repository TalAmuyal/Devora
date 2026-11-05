use std::collections::BTreeMap;

pub const DEFAULT: &str = "You are {{agent.name}}.\nYour role is {{agent.role}}.\nWork in {{space.path}} and keep persistent notes in {{memory.path}}.\n";

/// Literal, single-pass {{key}} replacement. Dotted/underscored identifier keys
/// are supported. Missing keys, stray delimiters and malformed keys are errors.
pub fn render(template: &str, inputs: &BTreeMap<String, String>) -> Result<String, String> {
    let mut output = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        let before = &rest[..start];
        if before.contains("}}") {
            return Err("unmatched template closing delimiter".into());
        }
        output.push_str(before);
        rest = &rest[start + 2..];
        let end = rest.find("}}").ok_or("unclosed template placeholder")?;
        let key = &rest[..end];
        if key.is_empty()
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'_')
        {
            return Err(format!("invalid template placeholder: {key}"));
        }
        output.push_str(
            inputs
                .get(key)
                .ok_or_else(|| format!("missing template value: {key}"))?,
        );
        rest = &rest[end + 2..];
    }
    if rest.contains("}}") {
        return Err("unmatched template closing delimiter".into());
    }
    output.push_str(rest);
    Ok(output)
}
