use serde_json::Value;

#[derive(Debug, PartialEq)]
enum Step {
    Key(String),
    Index(usize),
    Each,
}

fn parse(expr: &str) -> Result<Vec<Step>, String> {
    let expr = expr.trim();
    if expr == "." {
        return Ok(vec![]);
    }

    let mut steps = vec![];
    let mut rest = expr
        .strip_prefix('.')
        .ok_or("a --jq filter starts with a dot, like .name or .[].title")?;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']').ok_or("missing ] in --jq filter")?;
            let inner = &r[..end];
            steps.push(if inner.is_empty() {
                Step::Each
            } else {
                Step::Index(inner.parse().map_err(|_| "only numbers go inside [ ]")?)
            });
            rest = r[end + 1..].strip_prefix('.').unwrap_or(&r[end + 1..]);
        } else {
            let end = rest.find(['.', '[']).unwrap_or(rest.len());
            let key = &rest[..end];
            if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(format!("cannot read {key:?} in --jq filter"));
            }

            steps.push(Step::Key(key.to_string()));
            rest = rest[end..].strip_prefix('.').unwrap_or(&rest[end..]);
        }
    }
    Ok(steps)
}

pub fn apply(expr: &str, value: &Value) -> Result<Vec<Value>, String> {
    let mut current = vec![value.clone()];
    for step in parse(expr)? {
        current = current
            .into_iter()
            .flat_map(|v| match &step {
                Step::Key(k) => vec![v.get(k).cloned().unwrap_or(Value::Null)],
                Step::Index(i) => vec![v.get(*i).cloned().unwrap_or(Value::Null)],
                Step::Each => match v {
                    Value::Array(a) => a,
                    Value::Object(o) => o.into_iter().map(|(_, v)| v).collect(),
                    _ => vec![],
                },
            })
            .collect();
    }
    Ok(current)
}

pub fn show(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_keys_and_arrays() {
        let v = json!({ "cards": [{ "title": "A", "board": { "name": "B1" } }, { "title": "C", "board": { "name": "B2" } }] });
        assert_eq!(
            apply(".cards[].title", &v).unwrap(),
            [json!("A"), json!("C")]
        );
        assert_eq!(apply(".cards[1].board.name", &v).unwrap(), [json!("B2")]);
        assert_eq!(apply(".", &v).unwrap(), std::slice::from_ref(&v));
        assert!(apply("cards", &v).is_err());
        assert!(apply(".cards[x]", &v).is_err());
        assert_eq!(show(&json!("x")), "x");
    }
}
