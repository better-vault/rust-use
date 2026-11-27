use crate::{
    ActError, ActOutcome, Diff, QueryOutcome, Script, Snapshot, SnapshotMore, SnapshotOptions,
    Step, parse::Parser, types::Batch,
};
use rust_use_core::{
    ActionRequest, ChangeSource, ComponentInfo, InvokeError, Query, Ref, ResolveError,
    Subscription, Ui, Value,
};
use std::sync::atomic::{AtomicU64, Ordering};

/// A protocol session with its own revision cursor and interceptor guards.
pub struct Agent {
    ui: Ui,
    last_seen: AtomicU64,
    guards: Vec<Subscription>,
}

impl Agent {
    /// Open a session on a registry, initially tracking changes since revision zero.
    pub fn new(ui: &Ui) -> Self {
        Self {
            ui: ui.clone(),
            last_seen: AtomicU64::new(0),
            guards: Vec::new(),
        }
    }

    /// Require this predicate for every agent-originated invocation on this registry
    /// for the lifetime of this session. Chained predicates must all accept.
    #[must_use]
    pub fn allow(mut self, f: impl Fn(&ActionRequest<'_>) -> bool + Send + Sync + 'static) -> Self {
        self.guards.push(
            self.ui
                .intercept(move |req| (req.source != ChangeSource::Agent || f(req)).into()),
        );
        self
    }

    /// Describe the screen and advance the session cursor.
    ///
    /// ```
    /// use rust_use_agent::UiAgentExt;
    /// use rust_use_core::Ui;
    /// assert_eq!(Ui::new().agent().snapshot().to_string(), "rev 0");
    /// ```
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot_with(SnapshotOptions::default())
    }

    /// Describe a page of the screen and advance the session cursor.
    pub fn snapshot_with(&self, opts: SnapshotOptions) -> Snapshot {
        let page = self.component_page(opts, |_| true);
        self.advance(page.revision);
        page
    }

    /// Find components by exact kind and optional case-insensitive text.
    ///
    /// Text matches the label, kind, or primary value. The session cursor is preserved.
    pub fn search(
        &self,
        kind: Option<&str>,
        text: Option<&str>,
        opts: SnapshotOptions,
    ) -> Snapshot {
        let text = text.map(str::to_lowercase);
        self.component_page(opts, |component| {
            kind.is_none_or(|kind| component.name == kind)
                && text.as_deref().is_none_or(|text| {
                    let contains = |value: &str| value.to_lowercase().contains(text);
                    contains(component.name)
                        || component.label.as_deref().is_some_and(contains)
                        || component
                            .describe
                            .primary()
                            .filter(|value| !value.is_null())
                            .is_some_and(|value| {
                                value
                                    .as_str()
                                    .map_or_else(|| contains(&value.to_string()), contains)
                            })
                })
        })
    }

    fn component_page(
        &self,
        opts: SnapshotOptions,
        matches: impl Fn(&ComponentInfo) -> bool,
    ) -> Snapshot {
        loop {
            let revision = self.ui.revision();
            let mut components = self.ui.components_for(ChangeSource::Agent);
            if revision == self.ui.revision() {
                components.retain(|c| opts.from.is_none_or(|from| c.id >= from) && matches(c));
                let more = components.get(opts.limit).map(|next| SnapshotMore {
                    remaining: components.len() - opts.limit,
                    from: next.id,
                });
                components.truncate(opts.limit);
                return Snapshot {
                    revision,
                    components,
                    docs: opts.docs,
                    more,
                };
            }
        }
    }

    fn advance(&self, revision: u64) {
        self.last_seen.fetch_max(revision, Ordering::Relaxed);
    }

    /// Return changes since the session cursor and advance it.
    pub fn diff(&self) -> Diff {
        self.finish_diff(self.last_seen.load(Ordering::Relaxed))
    }

    fn finish_diff(&self, since: u64) -> Diff {
        let diff = self.diff_since(since);
        self.advance(diff.revision);
        diff
    }

    /// Describe changes after an explicit revision, preserving the cursor.
    pub fn diff_since(&self, revision: u64) -> Diff {
        loop {
            let delta = self.ui.agent_changes_since(revision);
            let changed: Vec<_> = delta
                .changed
                .into_iter()
                .filter_map(|id| self.ui.component_for(id, ChangeSource::Agent).ok())
                .collect();
            if delta.revision == self.ui.revision() {
                return Diff {
                    revision: delta.revision,
                    changed,
                    removed: delta.removed,
                };
            }
        }
    }

    /// Dispatch a batch in order and report changes made during the batch.
    /// Use [`Script::atomic`] or a leading `atomic` directive to restore primitive
    /// state if a step fails.
    /// With an approver installed, an action needing a person blocks the calling
    /// thread. Drive agents from a background thread so the GUI can answer.
    /// IPC already dispatches on a connection thread.
    ///
    /// # Errors
    /// Returns a parse diagnostic, a validation error, or the failing step with its diff.
    ///
    /// ```
    /// use rust_use_agent::UiAgentExt;
    /// use rust_use_core::Ui;
    /// let agent = Ui::new().agent();
    /// assert!(agent.act("Mute toggle").is_err());
    /// ```
    #[allow(
        clippy::result_large_err,
        reason = "The public API exposes the core error and partial diff directly."
    )]
    pub fn act(&self, script: impl Into<Script>) -> Result<ActOutcome, ActError> {
        let script = script.into();
        let (header, steps) = match script.batch {
            Batch::Text(s) => Parser::new(&s).script()?,
            Batch::Steps(v) => (false, v),
        };
        if steps.is_empty() {
            return Err(ActError::Parse {
                line: 1,
                column: 1,
                message: "expected an action step".into(),
            });
        }
        let before = self.ui.revision();
        let atomic = header || script.atomic;
        let result = if atomic {
            self.act_atomic(&steps)
        } else {
            run_steps(&steps, |s| {
                self.ui.invoke(
                    s.target.clone(),
                    &s.action,
                    s.args.clone(),
                    ChangeSource::Agent,
                )
            })
        };
        let values = result.map_err(|failure| {
            let (i, source) = match failure {
                DispatchError::Prepare(i, InvokeError::NotCheckpointable { id, component }) => {
                    return ActError::NotAtomic {
                        index: i + 1,
                        step: steps[i].to_string(),
                        id,
                        component,
                    };
                }
                DispatchError::Invoke(i, source) if atomic => {
                    return ActError::RolledBack {
                        index: i + 1,
                        step: steps[i].to_string(),
                        source,
                        diff: self.finish_diff(before),
                    };
                }
                DispatchError::Prepare(i, source) | DispatchError::Invoke(i, source) => (i, source),
            };
            ActError::Step {
                index: i + 1,
                step: steps[i].to_string(),
                source,
                diff: self.finish_diff(before),
            }
        })?;
        Ok(ActOutcome {
            values,
            diff: self.finish_diff(before),
        })
    }

    fn act_atomic(&self, steps: &[Step]) -> Result<Vec<(usize, Value)>, DispatchError> {
        self.ui.transaction(ChangeSource::Agent, |tx| {
            let ids = steps
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    tx.prepare(s.target.clone())
                        .map_err(|source| DispatchError::Prepare(i, source))
                })
                .collect::<Result<Vec<_>, _>>()?;
            run_steps(steps.iter().zip(ids), |(s, id)| {
                tx.invoke(id, &s.action, s.args.clone())
            })
        })
    }

    /// Search options starting at the supplied offset.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous targets.
    pub fn query(
        &self,
        target: impl Into<Ref>,
        text: Option<&str>,
        from: usize,
    ) -> Result<QueryOutcome, ResolveError> {
        let query = Query {
            text: text.map(str::to_owned),
            from,
            ..Query::default()
        };
        Ok(QueryOutcome {
            page: self.ui.query_for(target, &query, ChangeSource::Agent)?,
        })
    }

    /// Execute one protocol request and render its reply.
    ///
    /// ```
    /// use rust_use_agent::UiAgentExt;
    /// use rust_use_core::Ui;
    /// assert_eq!(Ui::new().agent().handle("diff"), "rev 0");
    /// ```
    pub fn handle(&self, request: &str) -> String {
        let (cmd, rest) = request
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((request.trim(), ""));
        let rest = rest.trim();
        match cmd {
            "hello" if rest.is_empty() => format!(
                "rust-use {} session {:016x} rev {}",
                env!("CARGO_PKG_VERSION"),
                self.ui.session(),
                self.ui.revision()
            ),
            "hello" => "err expected hello".into(),
            "snapshot" => Parser::new(rest).page_options(true).map_or_else(
                |e| format!("err {e}"),
                |opts| self.snapshot_with(opts).to_string(),
            ),
            "diff" if rest.is_empty() => self.diff().to_string(),
            "diff" => rest.parse::<u64>().map_or_else(
                |_| "err expected diff [revision]".into(),
                |rev| self.finish_diff(rev).to_string(),
            ),
            "act" => self
                .act(rest)
                .map_or_else(|e| e.to_reply(), |out| out.to_string()),
            "query" if rest.starts_with("--") => Parser::new(rest).search().map_or_else(
                |e| format!("err {e}"),
                |(kind, text, opts)| {
                    self.search(kind.as_deref(), text.as_deref(), opts)
                        .to_string()
                },
            ),
            "query" => Parser::new(rest).query().map_or_else(
                |e| format!("err {e}"),
                |(target, text, from)| {
                    self.query(target, text.as_deref(), from)
                        .map_or_else(|e| format!("err {e}"), |out| out.to_string())
                },
            ),
            _ => format!("err unknown command `{cmd}` (try: hello, snapshot, diff, act, query)"),
        }
    }
}

enum DispatchError {
    Prepare(usize, InvokeError),
    Invoke(usize, InvokeError),
}

fn run_steps<T>(
    steps: impl IntoIterator<Item = T>,
    mut invoke: impl FnMut(T) -> Result<Value, InvokeError>,
) -> Result<Vec<(usize, Value)>, DispatchError> {
    let mut values = Vec::new();
    for (i, step) in steps.into_iter().enumerate() {
        let value = invoke(step).map_err(|source| DispatchError::Invoke(i, source))?;
        if !value.is_null() {
            values.push((i + 1, value));
        }
    }
    Ok(values)
}

/// Open an agent session directly on a UI registry.
pub trait UiAgentExt {
    /// Create an independent protocol session.
    fn agent(&self) -> Agent;
}

impl UiAgentExt for Ui {
    fn agent(&self) -> Agent {
        Agent::new(self)
    }
}
