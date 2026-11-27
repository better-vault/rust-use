use rust_use_core::{ComponentInfo, Value};
use serde_json::json;
use std::fmt::{self, Write};

fn bare(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./:+-".contains(&b))
        && !matches!(s, "true" | "false" | "null")
        && s.parse::<f64>().is_err()
}

pub(crate) fn string(w: &mut impl Write, s: &str, force_quotes: bool) -> fmt::Result {
    if !force_quotes && bare(s) {
        return w.write_str(s);
    }
    w.write_char('"')?;
    for c in s.chars() {
        match c {
            '"' => w.write_str(r#"\""#)?,
            '\\' => w.write_str(r"\\")?,
            '\n' => w.write_str(r"\n")?,
            '\r' => w.write_str(r"\r")?,
            '\t' => w.write_str(r"\t")?,
            '\x00'..='\x1f' => write!(w, "\\u{:04x}", u32::from(c))?,
            _ => w.write_char(c)?,
        }
    }
    w.write_char('"')
}

pub(crate) fn value(w: &mut impl Write, v: &Value) -> fmt::Result {
    match v {
        Value::String(s) => string(w, s, false),
        Value::Array(items) => {
            w.write_char('[')?;
            for (i, item) in items.iter().enumerate() {
                if i != 0 {
                    w.write_char(',')?;
                }
                value(w, item)?;
            }
            w.write_char(']')
        }
        _ => write!(w, "{v}"),
    }
}

pub(crate) fn component(w: &mut impl Write, c: &ComponentInfo, docs: bool) -> fmt::Result {
    write!(w, "\n{} {}", c.id, c.name)?;
    if let Some(s) = &c.label {
        w.write_char(' ')?;
        string(w, s, true)?;
    }
    // A null primary value (an empty `Option`) is "no value": omit it, as for attributes.
    if let Some(v) = c.describe.primary().filter(|v| !v.is_null()) {
        w.write_str(" =")?;
        value(w, v)?;
    }
    for (k, v) in c.describe.attrs() {
        match v {
            Value::Null | Value::Bool(false) => (),
            Value::Bool(true) => write!(w, " {k}")?,
            _ => {
                write!(w, " {k}=")?;
                value(w, v)?;
            }
        }
    }
    for a in &c.actions {
        let prefix = if a.confirm.is_some() { "!!" } else { "!" };
        write!(w, " {prefix}{}", a.name)?;
        if !a.params.is_empty() {
            w.write_char('(')?;
            for (i, p) in a.params.iter().enumerate() {
                if i != 0 {
                    w.write_char(',')?;
                }
                w.write_str(p.name)?;
                if p.ty != "str" {
                    write!(w, ":{}", p.ty)?;
                }
                if p.optional {
                    w.write_char('?')?;
                }
            }
            w.write_char(')')?;
        }
    }
    if c.queryable {
        w.write_str(" ?query")?;
    }
    if docs {
        for a in &c.actions {
            for line in a.doc.lines() {
                write!(w, "\n  # {line}")?;
            }
            if let Some(text) = a.confirm {
                if a.doc.is_empty() {
                    w.write_str("\n  #")?;
                }
                write!(w, " (needs a person: {text})")?;
            }
        }
    }
    Ok(())
}

pub(crate) fn json_component(c: &ComponentInfo, docs: bool) -> Value {
    let actions = c
        .actions
        .iter()
        .map(|a| {
            let params = a
                .params
                .iter()
                .map(|p| json!({"name":p.name,"type":p.ty,"optional":p.optional}))
                .collect::<Vec<_>>();
            let mut action = json!({"name":a.name,"params":params});
            if let Some(text) = a.confirm {
                action["confirm"] = text.into();
            }
            if docs {
                action["doc"] = a.confirm.map_or_else(
                    || a.doc.into(),
                    |text| format!("{} (needs a person: {text})", a.doc).into(),
                );
            }
            action
        })
        .collect::<Vec<_>>();
    let attrs = c
        .describe
        .attrs()
        .map(|(k, v)| (k.to_owned(), v.clone()))
        .collect::<serde_json::Map<_, _>>();
    let mut result = json!({"id":c.id.get(),"kind":c.name,"label":c.label,"revision":c.revision,"attrs":attrs,"actions":actions,"query":c.queryable});
    if let Some(v) = c.describe.primary() {
        result["value"] = v.clone();
    }
    result
}
