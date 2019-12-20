//! Detach an app from the DOM.

/// Detach an app from the DOM.
pub trait Detach<Message> {
    /// Detach an app from the DOM.
    fn detach(&self);
}
