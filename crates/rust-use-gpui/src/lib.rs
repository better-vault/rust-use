//! GPUI adapter for `rust-use` state, behaviors, and agent integration.
//! Most users should depend on the `rust-use` facade with the `gpui` feature.

use futures::{StreamExt, channel::mpsc};
use gpui::{App, ClickEvent, Context, MouseDownEvent, Task, Window};
use rust_use_core::{Args, ChangeSource, ComponentId, Handle, Primitive, Subscription, Ui};

/// Keeps a GPUI entity notified of changes in a [`Ui`].
///
/// Store the binding in the view so it lasts as long as the view. Dropping it stops observation.
///
/// ```
/// use gpui::{App, AppContext};
/// use rust_use_core::Ui;
/// use rust_use_gpui::prelude::*;
///
/// struct View {
///     _binding: UiBinding,
/// }
///
/// fn mount_view(ui: &Ui, cx: &mut App) -> gpui::Entity<View> {
///     cx.new(|cx| View { _binding: ui.bind(cx) })
/// }
/// ```
#[must_use = "dropping the binding stops observation"]
pub struct UiBinding {
    _subscription: Subscription,
    _task: Task<()>,
}

/// Observe registry changes from a GPUI entity with [`Self::bind`] or [`Self::bind_to`].
pub trait UiExt {
    /// Notify the entity owning `cx` about changes from any thread or source.
    ///
    /// Changes queued before the GPUI task handles them produce one notification.
    ///
    /// ```
    /// use gpui::Context;
    /// use rust_use_core::Ui;
    /// use rust_use_gpui::prelude::*;
    ///
    /// fn new_binding<V: 'static>(ui: &Ui, cx: &mut Context<'_, V>) -> UiBinding {
    ///     ui.bind(cx)
    /// }
    /// ```
    fn bind<V: 'static>(&self, cx: &mut Context<'_, V>) -> UiBinding;

    /// Like [`Self::bind`], with notifications limited to the given component IDs.
    ///
    /// ```
    /// use gpui::Context;
    /// use rust_use_core::{Handle, Primitive};
    /// use rust_use_gpui::prelude::*;
    ///
    /// fn watch<P: Primitive, V: 'static>(handle: &Handle<P>, cx: &mut Context<'_, V>) -> UiBinding {
    ///     handle.ui().bind_to(&[handle.id()], cx)
    /// }
    /// ```
    fn bind_to<V: 'static>(&self, ids: &[ComponentId], cx: &mut Context<'_, V>) -> UiBinding;
}

impl UiExt for Ui {
    fn bind<V: 'static>(&self, cx: &mut Context<'_, V>) -> UiBinding {
        bind_changes(self, cx, |_| true)
    }

    fn bind_to<V: 'static>(&self, ids: &[ComponentId], cx: &mut Context<'_, V>) -> UiBinding {
        let ids = ids.to_vec();
        bind_changes(self, cx, move |id| ids.contains(&id))
    }
}

fn bind_changes<V: 'static>(
    ui: &Ui,
    cx: &mut Context<'_, V>,
    matches: impl Fn(ComponentId) -> bool + Send + Sync + 'static,
) -> UiBinding {
    let (subscription, mut rx) = subscribe_changes(ui, matches);
    let task = cx.spawn(async move |this, cx| {
        while next_change(&mut rx).await {
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    });
    UiBinding {
        _subscription: subscription,
        _task: task,
    }
}

fn subscribe_changes(
    ui: &Ui,
    matches: impl Fn(ComponentId) -> bool + Send + Sync + 'static,
) -> (Subscription, mpsc::UnboundedReceiver<()>) {
    let (tx, rx) = mpsc::unbounded();
    let subscription = ui.subscribe(move |event| {
        if matches(event.id) {
            let _ = tx.unbounded_send(());
        }
    });
    (subscription, rx)
}

async fn next_change(rx: &mut mpsc::UnboundedReceiver<()>) -> bool {
    if rx.next().await.is_none() {
        return false;
    }
    // A single notify covers all changes already queued for this GPUI turn.
    while rx.try_recv().is_ok() {}
    true
}

/// Turn GPUI input into registry actions via [`Self::on_click`] or [`Self::on_mouse_down`].
///
/// Both listeners report invocation errors to stderr in debug builds and discard them in release.
pub trait HandleExt<P: Primitive> {
    /// Invoke an action with the given arguments and [`ChangeSource::User`] on click.
    ///
    /// ```
    /// use gpui::{IntoElement, div, prelude::*};
    /// use rust_use_core::{Handle, Primitive};
    /// use rust_use_gpui::prelude::*;
    ///
    /// fn button<P: Primitive>(handle: &Handle<P>) -> impl IntoElement {
    ///     div().id("button").on_click(handle.on_click("press", ()))
    /// }
    /// ```
    fn on_click(
        &self,
        action: &'static str,
        args: impl Into<Args>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static;

    /// Invoke an action with the given arguments and [`ChangeSource::User`] on mouse down.
    ///
    /// ```
    /// use gpui::{IntoElement, MouseButton, div, prelude::*};
    /// use rust_use_core::{Handle, Primitive};
    /// use rust_use_gpui::prelude::*;
    ///
    /// fn board_square<P: Primitive>(handle: &Handle<P>) -> impl IntoElement {
    ///     div().on_mouse_down(MouseButton::Left, handle.on_mouse_down("select", ()))
    /// }
    /// ```
    fn on_mouse_down(
        &self,
        action: &'static str,
        args: impl Into<Args>,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static;
}

impl<P: Primitive> HandleExt<P> for Handle<P> {
    fn on_click(
        &self,
        action: &'static str,
        args: impl Into<Args>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        action_listener(self, action, args.into())
    }

    fn on_mouse_down(
        &self,
        action: &'static str,
        args: impl Into<Args>,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        action_listener(self, action, args.into())
    }
}

fn action_listener<P: Primitive, E>(
    handle: &Handle<P>,
    action: &'static str,
    args: Args,
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    let handle = handle.clone();
    move |_, _, _| {
        let result = handle.invoke(action, args.clone(), ChangeSource::User);
        #[cfg(debug_assertions)]
        if let Err(error) = result {
            eprintln!("rust-use GPUI action {action} on {}: {error}", handle.id());
        }
        #[cfg(not(debug_assertions))]
        drop(result);
    }
}

/// Common imports for binding views and dispatching GPUI input.
pub mod prelude {
    pub use crate::{HandleExt, UiBinding, UiExt};
}

#[cfg(test)]
mod tests {
    use std::thread;

    use futures::FutureExt;
    use rust_use_core::{ActionError, ActionSpec, Behavior, Describe, Press, Value};

    use super::*;

    #[derive(Default)]
    struct Button(Press);

    impl Primitive for Button {
        const NAME: &'static str = "button";
    }

    impl Behavior for Button {
        fn describe(&self, out: &mut Describe) {
            self.0.describe(out);
        }

        fn actions(&self) -> Vec<ActionSpec> {
            self.0.actions()
        }

        fn invoke(&mut self, action: &str, args: &Args) -> Option<Result<Value, ActionError>> {
            self.0.invoke(action, args)
        }
    }

    #[test]
    fn coalesces_cross_thread_changes_and_filters_components()
    -> Result<(), Box<dyn std::error::Error>> {
        let ui = Ui::new();
        let button = ui.mount(Button::default());
        let other = ui.mount(Button::default());
        let id = button.id();
        let (subscription, mut rx) = subscribe_changes(&ui, move |changed| changed == id);
        let worker_ui = ui.clone();
        thread::spawn(move || {
            for source in [
                ChangeSource::Agent,
                ChangeSource::User,
                ChangeSource::Program,
            ] {
                worker_ui.invoke(id, "press", (), source)?;
            }
            Ok::<_, rust_use_core::InvokeError>(())
        })
        .join()
        .map_err(|_| "invocation thread panicked")??;

        assert_eq!(next_change(&mut rx).now_or_never(), Some(true));
        assert_eq!(next_change(&mut rx).now_or_never(), None);

        other.invoke("press", (), ChangeSource::Agent)?;
        assert_eq!(next_change(&mut rx).now_or_never(), None);

        button.invoke("press", (), ChangeSource::Program)?;
        assert_eq!(next_change(&mut rx).now_or_never(), Some(true));
        drop(subscription);
        button.invoke("press", (), ChangeSource::Agent)?;
        assert_eq!(next_change(&mut rx).now_or_never(), Some(false));
        Ok(())
    }
}
