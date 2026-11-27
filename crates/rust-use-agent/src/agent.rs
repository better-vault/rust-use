use crate::{
    ActError, ActOutcome, Diff, QueryOutcome, Script, Snapshot, SnapshotOptions, parse::Parser,
    types::Batch,
};
use rust_use_core::{ActionRequest, ChangeSource, Query, Ref, ResolveError, Subscription, Ui};
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
    /// Describe the screen using the requested documentation setting.
    pub fn snapshot_with(&self, opts: SnapshotOptions) -> Snapshot {
        loop {
            let revision = self.ui.revision();
            let components = self.ui.components();
            if revision == self.ui.revision() {
                self.advance(revision);
                return Snapshot {
                    revision,
                    components,
                    docs: opts.docs,
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
            let delta = self.ui.changes_since(revision);
            let changed: Vec<_> = delta
                .changed
                .into_iter()
                .filter_map(|id| self.ui.component(id).ok())
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
    ///
    /// # Errors
    /// Returns a parse diagnostic or the failing step with the partial diff.
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
        let steps = match script.into().0 {
            Batch::Text(s) => Parser::new(&s).script()?,
            Batch::Steps(v) => v,
        };
        if steps.is_empty() {
            return Err(ActError::Parse {
                line: 1,
                column: 1,
                message: "expected an action step".into(),
            });
        }
        let before = self.ui.revision();
        let mut values = Vec::new();
        for (i, s) in steps.into_iter().enumerate() {
            let index = i + 1;
            match self.ui.invoke(
                s.target.clone(),
                &s.action,
                s.args.clone(),
                ChangeSource::Agent,
            ) {
                Ok(v) => {
                    if !v.is_null() {
                        values.push((index, v));
                    }
                }
                Err(source) => {
                    return Err(ActError::Step {
                        index,
                        step: s.to_string(),
                        source,
                        diff: self.finish_diff(before),
                    });
                }
            }
        }
        Ok(ActOutcome {
            values,
            diff: self.finish_diff(before),
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
            page: self.ui.query(target, &query)?,
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
            "snapshot" if rest.is_empty() || rest == "--verbose" => self
                .snapshot_with(SnapshotOptions {
                    docs: !rest.is_empty(),
                })
                .to_string(),
            "snapshot" => "err expected snapshot [--verbose]".into(),
            "diff" if rest.is_empty() => self.diff().to_string(),
            "diff" => rest.parse::<u64>().map_or_else(
                |_| "err expected diff [revision]".into(),
                |rev| self.finish_diff(rev).to_string(),
            ),
            "act" => self
                .act(rest)
                .map_or_else(|e| e.to_reply(), |out| out.to_string()),
            "query" => Parser::new(rest).query().map_or_else(
                |e| format!("err {e}"),
                |(target, text, from)| {
                    self.query(target, text.as_deref(), from)
                        .map_or_else(|e| format!("err {e}"), |out| out.to_string())
                },
            ),
            _ => format!("err unknown command `{cmd}` (try: snapshot, diff, act, query)"),
        }
    }
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
