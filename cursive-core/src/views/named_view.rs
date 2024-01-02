use crate::{
    event::{AnyCb, EventResult},
    view::{Selector, View, ViewNotFound, ViewWrapper},
};
use ouroboros::self_referencing;
use std::sync::{Arc, Mutex, MutexGuard};

/// Wrapper around a view to make it identifiable.
///
/// This lets other views refer to this one using a string identifier.
///
/// See [`Nameable`](crate::view::Nameable) for an easy way to wrap any view with it.
pub struct NamedView<V> {
    view: Arc<Mutex<V>>,
    name: String,
}

/// Mutable reference to a view.
///
/// This behaves like a [`MutexGuard`], but without being tied to a lifetime.
///
/// [`MutexGuard`]: std::sync::MutexGuard
//pub type ViewRef<V> = OwningHandle<ArcRef<Mutex<V>>, MutexGuard<'static, V>>;
#[self_referencing]
pub struct ViewRef<V: 'static> {
    owner: Arc<Mutex<V>>,

    #[borrows(owner)]
    #[covariant]
    guard: MutexGuard<'this, V>,
}

impl<V> std::ops::Deref for ViewRef<V> {
    type Target = V;
    fn deref(&self) -> &Self::Target {
        self.borrow_guard()
    }
}

impl<V> ViewRef<V> {
    /// Run the given closure on the targetted view.
    pub fn run<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut V) -> R,
    {
        self.with_guard_mut(|guard| f(guard))
    }
}

impl<V> NamedView<V> {
    /// Wraps `view` in a new `NamedView`.
    pub fn new<S: Into<String>>(name: S, view: V) -> Self {
        NamedView {
            view: Arc::new(Mutex::new(view)),
            name: name.into(),
        }
    }

    /// Gets mutable access to the inner view.
    ///
    /// This returns a `ViewRef<V>`, which implement `Deref<Target = V>` for read-only accesses,
    /// and [`ViewRef::run()`] for mutable access.
    ///
    /// # Panics
    ///
    /// Panics if another reference for this view already exists.
    pub fn get_mut(&mut self) -> ViewRef<V> {
        let owner = Arc::clone(&self.view);

        ViewRefBuilder {
            owner,
            guard_builder: |owner| owner.lock().unwrap(),
        }
        .build()
    }

    /// Returns the name attached to this view.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Changes the name attached to this view.
    pub fn set_name<S: Into<String>>(&mut self, name: S) {
        self.name = name.into();
    }
}

impl<T: View + 'static> ViewWrapper for NamedView<T> {
    type V = T;

    fn with_view<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&Self::V) -> R,
    {
        self.view.try_lock().ok().map(|v| f(&*v))
    }

    fn with_view_mut<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut Self::V) -> R,
    {
        self.view.try_lock().ok().map(|mut v| f(&mut *v))
    }

    fn into_inner(mut self) -> Result<Self::V, Self>
    where
        Self::V: Sized,
    {
        match Arc::try_unwrap(self.view) {
            Err(rc) => {
                // Whoops! Abort! Undo!
                self.view = rc;
                Err(self)
            }
            Ok(cell) => Ok(cell.into_inner().unwrap()),
        }
    }

    fn wrap_call_on_any(&mut self, selector: &Selector, callback: AnyCb) {
        match selector {
            &Selector::Name(name) if name == self.name => callback(self),
            s => {
                self.with_view_mut(|v| v.call_on_any(s, callback));
            }
        }
    }

    fn wrap_focus_view(&mut self, selector: &Selector) -> Result<EventResult, ViewNotFound> {
        match selector {
            &Selector::Name(name) if name == self.name => Ok(EventResult::Consumed(None)),
            s => self
                .view
                .try_lock()
                .map_err(|_| ViewNotFound)
                .and_then(|mut v| v.focus_view(s)),
        }
    }
}

#[crate::recipe(NamedView::new(name, view))]
struct Recipe {
    name: String,
    view: crate::views::BoxedView,
}

crate::raw_recipe!(with name, |config, context| {
    let name: String = context.resolve(config)?;
    Ok(|view| NamedView::new(name, view))
});

/*
crate::raw_recipe!(NamedView, |config, context| {
    let name: String = context.resolve(&config["name"])?;
    let view: crate::views::BoxedView = context.resolve(&config["view"])?;
    Ok(NamedView::new(name, view))
});
*/
