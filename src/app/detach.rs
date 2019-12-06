//! Detach an app from the DOM.

use std::rc::Rc;
use std::cell::RefCell;

/// Detach an app from the DOM.
pub trait Detach<Message> {
    /// Detach an app from the DOM.
    fn detach(app_rc: Rc<RefCell<Self>>);
}
