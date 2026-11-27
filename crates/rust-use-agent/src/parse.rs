use crate::{ActError, SnapshotOptions, Step, step};
use rust_use_core::{Ref, Value};
use serde_json::{Deserializer, Number};

#[derive(Debug, thiserror::Error)]
#[error("parse error at {line}:{column}: {message}")]
pub(crate) struct Syntax {
    line: usize,
    column: usize,
    message: String,
}

impl From<Syntax> for ActError {
    fn from(s: Syntax) -> Self {
        Self::Parse {
            line: s.line,
            column: s.column,
            message: s.message,
        }
    }
}

pub(crate) struct Parser<'a> {
    input: &'a str,
    rest: &'a str,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self { input, rest: input }
    }
    fn error(&self, message: impl Into<String>) -> Syntax {
        let before = &self.input[..self.input.len() - self.rest.len()];
        Syntax {
            line: before.bytes().filter(|&c| c == b'\n').count() + 1,
            column: before.rsplit('\n').next().unwrap_or("").chars().count() + 1,
            message: message.into(),
        }
    }
    fn spaces(&mut self) {
        self.rest = self
            .rest
            .trim_start_matches(|c: char| c.is_whitespace() && c != '\n');
    }
    fn skip_comment(&mut self) {
        if self.rest.starts_with('#') {
            self.rest = self.rest.find('\n').map_or("", |i| &self.rest[i..]);
        }
    }
    fn list_space(&mut self) {
        self.rest = self.rest.trim_start();
        while self.rest.starts_with('#') {
            self.skip_comment();
            self.rest = self.rest.trim_start();
        }
    }
    fn boundary(&self) -> bool {
        self.rest.is_empty() || self.rest.starts_with([';', '#', '\n'])
    }
    fn gap(&mut self) -> Result<(), Syntax> {
        if !self.boundary() && !self.rest.starts_with(char::is_whitespace) {
            return Err(self.error("expected whitespace"));
        }
        self.spaces();
        Ok(())
    }
    fn word(&mut self) -> Result<&'a str, Syntax> {
        let end = self
            .rest
            .find(|c: char| c.is_whitespace() || "[]{}#,;\"".contains(c))
            .unwrap_or(self.rest.len());
        if end == 0 {
            return Err(self.error("expected a token"));
        }
        let (s, rest) = self.rest.split_at(end);
        self.rest = rest;
        Ok(s)
    }
    fn json<T: serde::de::DeserializeOwned>(&mut self) -> Result<T, Syntax> {
        let mut stream = Deserializer::from_str(self.rest).into_iter::<T>();
        let v = stream
            .next()
            .ok_or_else(|| self.error("expected JSON"))?
            .map_err(|e| self.error(e.to_string()))?;
        self.rest = &self.rest[stream.byte_offset()..];
        Ok(v)
    }
    fn target(&mut self) -> Result<Ref, Syntax> {
        if self.rest.starts_with('"') {
            return self.json().map(Ref::Label);
        }
        let s = self.word()?;
        if s.starts_with('@') {
            s.parse()
                .map(Ref::Id)
                .map_err(|_| self.error("expected @ followed by a component id"))
        } else {
            Ok(s.into())
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value, Syntax> {
        if depth >= 128 {
            return Err(self.error("list nesting limit exceeded"));
        }
        if self.rest.starts_with(['"', '{']) {
            return self.json();
        }
        if let Some(rest) = self.rest.strip_prefix('[') {
            self.rest = rest;
            self.list_space();
            let mut items = Vec::new();
            if !self.rest.starts_with(']') {
                loop {
                    items.push(self.value(depth + 1)?);
                    self.list_space();
                    if let Some(rest) = self.rest.strip_prefix(',') {
                        self.rest = rest;
                        self.list_space();
                    } else {
                        break;
                    }
                }
            }
            self.rest = self
                .rest
                .strip_prefix(']')
                .ok_or_else(|| self.error("expected ]"))?;
            return Ok(Value::Array(items));
        }
        let s = self.word()?;
        Ok(match s {
            "true" => true.into(),
            "false" => false.into(),
            "null" => Value::Null,
            _ => {
                if let Ok(n) = s.parse::<i64>() {
                    n.into()
                } else if let Ok(n) = s.parse::<u64>() {
                    n.into()
                } else if let Some(n) = s.parse::<f64>().ok().and_then(Number::from_f64) {
                    n.into()
                } else {
                    s.into()
                }
            }
        })
    }
    pub(crate) fn script(mut self) -> Result<Vec<Step>, Syntax> {
        let mut steps = Vec::new();
        loop {
            self.spaces();
            self.skip_comment();
            if self.rest.is_empty() {
                break;
            }
            if self.rest.starts_with([';', '\n']) {
                self.rest = &self.rest[1..];
                continue;
            }
            let target = self.target()?;
            self.gap()?;
            let action = self.word()?;
            let mut args = Vec::new();
            self.gap()?;
            while !self.boundary() {
                args.push(self.value(0)?);
                self.gap()?;
            }
            steps.push(step(target, action, args));
        }
        if steps.is_empty() {
            return Err(self.error("expected an action step"));
        }
        Ok(steps)
    }
    pub(crate) fn query(mut self) -> Result<(Ref, Option<String>, usize), Syntax> {
        let target = self.target()?;
        self.gap()?;
        let mut text = None;
        if !self.rest.is_empty() && !self.rest.starts_with("from=") {
            text = Some(self.text()?);
            self.gap()?;
        }
        let from = if self.rest.is_empty() {
            0
        } else {
            self.rest
                .strip_prefix("from=")
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| self.error("expected from=<offset>"))?
        };
        Ok((target, text, from))
    }

    fn text(&mut self) -> Result<String, Syntax> {
        if self.rest.starts_with('"') {
            self.json()
        } else {
            self.word().map(str::to_owned)
        }
    }

    pub(crate) fn search(
        mut self,
    ) -> Result<(Option<String>, Option<String>, SnapshotOptions), Syntax> {
        let flag = self.word()?;
        self.gap()?;
        let (kind, text) = match flag {
            "--kind" => {
                let kind = self.text()?;
                self.gap()?;
                let text = if self.rest.is_empty() || self.rest.starts_with("--") {
                    None
                } else {
                    Some(self.text()?)
                };
                (Some(kind), text)
            }
            "--text" => (None, Some(self.text()?)),
            _ => return Err(self.error("expected --kind or --text")),
        };
        self.gap()?;
        Ok((kind, text, self.page_options(false)?))
    }

    pub(crate) fn page_options(mut self, verbose: bool) -> Result<SnapshotOptions, Syntax> {
        let mut opts = SnapshotOptions::default();
        while !self.rest.is_empty() {
            let flag = self.word()?;
            self.gap()?;
            match flag {
                "--verbose" if verbose => opts.docs = true,
                "--from" => {
                    let id = self.word()?;
                    opts.from = Some(
                        id.strip_prefix('@')
                            .and_then(|_| id.parse().ok())
                            .ok_or_else(|| self.error("expected @ followed by a component id"))?,
                    );
                }
                "--limit" => {
                    opts.limit = self
                        .word()?
                        .parse()
                        .map_err(|_| self.error("expected a nonnegative limit"))?;
                }
                _ => return Err(self.error(format!("unexpected option `{flag}`"))),
            }
            self.gap()?;
        }
        Ok(opts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compact_values_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let mut samples = vec![
            json!(null),
            json!(false),
            json!(true),
            json!(u64::MAX),
            json!(-17),
            json!(-1.25),
            json!({"nested":[null,"雪"]}),
            json!([]),
        ];
        for s in [
            "bare",
            "",
            "true",
            "001",
            "+10",
            "1e-3",
            "a b",
            "雪",
            "a#b;c",
            "\"\\\n\t\r\0",
            "x,y",
            "[x]",
        ] {
            samples.push(s.into());
        }
        let mut seed = 827_usize;
        for _ in 0..64 {
            seed = seed.wrapping_mul(65599).wrapping_add(1);
            samples.push(json!([
                samples[seed % samples.len()],
                samples[(seed >> 8) % samples.len()]
            ]));
        }
        for sample in samples {
            let mut encoded = String::new();
            crate::format::value(&mut encoded, &sample)?;
            let mut parser = Parser::new(&encoded);
            assert_eq!(parser.value(0)?, sample);
            assert!(parser.rest.is_empty());
        }
        Ok(())
    }
}
