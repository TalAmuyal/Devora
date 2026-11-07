use std::collections::BTreeMap;

/// The built-in template, used when no `agent.template-file` is configured.
pub const DEFAULT: &str = include_str!("default-template.md");

/// Placeholders of the built-in template that may stay empty; they default to
/// an empty value, so an unset optional section doesn't block creation.
pub const OPTIONAL: [&str; 4] = [
    "role.details",
    "user.details",
    "extra.details",
    "custom.sections",
];

/// Literal, single-pass {{key}} replacement. Dotted/underscored identifier keys
/// are supported. Missing keys, stray delimiters and malformed keys are errors.
pub fn render(template: &str, inputs: &BTreeMap<String, String>) -> Result<String, String> {
    let mut output = String::new();
    walk(template, |piece| {
        match piece {
            Piece::Text(text) => output.push_str(text),
            Piece::Placeholder(key) => output.push_str(
                inputs
                    .get(key)
                    .ok_or_else(|| format!("missing template value: {key}"))?,
            ),
        }
        Ok(())
    })?;
    Ok(output)
}

/// Collapses the blank lines that empty optional values leave into one, and
/// ends the text with a single newline.
pub fn tidy(rendered: &str) -> String {
    let mut output = String::new();
    let mut blank = false;
    for line in rendered.trim().lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if !blank {
                output.push('\n');
            }
            blank = true;
        } else {
            output.push_str(line);
            output.push('\n');
            blank = false;
        }
    }
    output
}

/// The template's placeholders, in order of first appearance, under the same
/// syntax rules as [`render`].
pub fn placeholders(template: &str) -> Result<Vec<String>, String> {
    let mut keys: Vec<String> = vec![];
    walk(template, |piece| {
        if let Piece::Placeholder(key) = piece {
            if !keys.iter().any(|k| k == key) {
                keys.push(key.into());
            }
        }
        Ok(())
    })?;
    Ok(keys)
}

enum Piece<'a> {
    Text(&'a str),
    Placeholder(&'a str),
}

fn walk<'a>(
    template: &'a str,
    mut visit: impl FnMut(Piece<'a>) -> Result<(), String>,
) -> Result<(), String> {
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        let before = &rest[..start];
        if before.contains("}}") {
            return Err("unmatched template closing delimiter".into());
        }
        visit(Piece::Text(before))?;
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
        visit(Piece::Placeholder(key))?;
        rest = &rest[end + 2..];
    }
    if rest.contains("}}") {
        return Err("unmatched template closing delimiter".into());
    }
    visit(Piece::Text(rest))
}
