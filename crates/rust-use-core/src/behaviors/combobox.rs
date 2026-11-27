use super::{List, action, dispatch, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Page, Query, Value};
use serde::{Serialize, de::DeserializeOwned};

/// A searchable set of options with one selection or optional free text.
///
/// Agent picks prefer exact matches, then match strings case-insensitively.
/// Rejected picks list up to five options matching the input as query text.
#[derive(Debug)]
pub struct Combobox<T> {
    options: List<T>,
    value: Option<T>,
    free_text: Option<String>,
    allow_free_text: bool,
    open: bool,
}

impl<T> Combobox<T> {
    /// Create a closed combobox with no selection and free text disabled.
    pub fn new(options: Vec<T>) -> Self {
        Self {
            options: List::new(options),
            value: None,
            free_text: None,
            allow_free_text: false,
            open: false,
        }
    }

    /// Configure whether the agent can enter free text.
    #[must_use]
    pub fn free_text(mut self, allowed: bool) -> Self {
        self.allow_free_text = allowed;
        self
    }

    /// Borrow the options in display order.
    pub fn options(&self) -> &List<T> {
        &self.options
    }

    /// Borrow the picked option, if any.
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Borrow the free text, if any.
    pub fn text(&self) -> Option<&str> {
        self.free_text.as_deref()
    }

    /// Show the options popup.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Hide the options popup.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the options popup is visible.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Reset the picked option and free text.
    pub fn clear(&mut self) {
        self.value = None;
        self.free_text = None;
    }

    fn select(&mut self, option: T) {
        self.value = Some(option);
        self.free_text = None;
        self.close();
    }
}

impl<T: PartialEq> Combobox<T> {
    /// Pick an option by exact equality and close the popup.
    ///
    /// # Errors
    /// Returns a rejection if the supplied value is absent from the options.
    pub fn pick(&mut self, option: T) -> Result<(), ActionError> {
        if !self.options.items.contains(&option) {
            return Err(ActionError::Rejected("option not found".into()));
        }
        self.select(option);
        Ok(())
    }
}

const ACTIONS: [ActionSpec; 3] = [
    action("pick", "Pick an option.", &[param("option", "any")]),
    action("type", "Enter free text.", &[param("text", "str")]),
    action("clear", "Clear the current value.", &[]),
];

impl<T: Serialize + DeserializeOwned + PartialEq + Send + 'static> Behavior for Combobox<T> {
    fn describe(&self, out: &mut Describe) {
        if let Some(value) = self.value() {
            out.value(value);
        } else if let Some(text) = self.text() {
            out.value(text);
        }
        out.attr("n", self.options.len())
            .flag("open", self.is_open());
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        ACTIONS.iter().any(|spec| spec.name == action)
    }

    fn is_enabled(&self, name: &str) -> bool {
        match name {
            "pick" => true,
            "type" => self.allow_free_text,
            "clear" => self.value.is_some() || self.free_text.is_some(),
            _ => false,
        }
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            if !self.is_enabled(name) {
                return Err(ActionError::Disabled(name.into()));
            }
            match name {
                "pick" => {
                    let input = args
                        .as_slice()
                        .first()
                        .ok_or(ActionError::MissingArg("option"))?;
                    self.pick_agent(input)?;
                }
                "type" => {
                    self.free_text = Some(args.get(0, "text")?);
                    self.value = None;
                }
                _ => self.clear(),
            }
            Ok(())
        })
    }

    fn queryable(&self) -> bool {
        true
    }

    fn query(&self, q: &Query) -> Option<Page> {
        self.options.query(q)
    }
}

impl<T: Serialize + DeserializeOwned + PartialEq + Send + 'static> Combobox<T> {
    fn pick_agent(&mut self, input: &Value) -> Result<(), ActionError> {
        if let Ok(option) = T::deserialize(input)
            && self.pick(option).is_ok()
        {
            return Ok(());
        }
        if let Some(text) = input.as_str() {
            let needle = text.to_lowercase();
            let option = self.options.iter().find_map(|option| {
                let value = serde_json::to_value(option).ok()?;
                let text = value.as_str()?;
                if text.to_lowercase() == needle {
                    T::deserialize(&value)
                        .ok()
                        .filter(|decoded| option == decoded)
                } else {
                    None
                }
            });
            if let Some(option) = option {
                self.select(option);
                return Ok(());
            }
        }
        let query = Query {
            text: Some(
                input
                    .as_str()
                    .map_or_else(|| input.to_string(), str::to_owned),
            ),
            limit: 5,
            ..Query::default()
        };
        let suggestions = self.options.query(&query).unwrap_or_default().items;
        let reason = if suggestions.is_empty() {
            "option not found".into()
        } else {
            format!(
                "option not found; matching options: {}",
                Value::Array(suggestions)
            )
        };
        Err(ActionError::Rejected(reason))
    }
}
