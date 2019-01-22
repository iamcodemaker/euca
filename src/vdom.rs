//! Flexible generic virtual dom representation.
//!
//! The types here can be used to generically represent a virtual dom tree. This generic
//! representation can be used to plug various concrete virtual dom representations into the
//! [`diff`] and [`patch`] algorithms implemented in this crate.
//!
//! [`diff`]: ../diff/fn.diff.html
//! [`patch`]: ../patch/enum.Patch.html

use std::fmt;
use std::cmp;
use wasm_bindgen::prelude::*;

/// This represents an event handler. The handler can either always map to a specific message, or a
/// function can be provided that will transform the given [`web_sys::Event`] into a message. This
/// function must be a plain fn pointer and cannot capture any state from the environment.
///
/// [`web_sys::Event`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Event.html
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EventHandler<'a, Message> {
    /// A message that will be generated when this event associated with this handler fires.
    Msg(&'a Message),

    /// A callback that will convert a [`web_sys::Event`] into a message.
    ///
    /// [`web_sys::Event`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Event.html
    Fn(fn(web_sys::Event) -> Message),
}

/// Callbacks used to store and retrieve dom nodes and closures.
pub enum Storage<'a, T> {
    /// This will be called one or zero times and should take the stored object from the virtual
    /// dom and return it.
    Read(Box<FnMut() -> T + 'a>),
    /// This will be called one or zero times and should store the given object in the virtual dom.
    Write(Box<FnMut(T) + 'a>),
}

impl<'a, T> fmt::Debug for Storage<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Storage::Read(_) => write!(f, "Read(_)"),
            Storage::Write(_) => write!(f, "Write(_)"),
        }
    }
}

impl<'a, T> cmp::PartialEq for Storage<'a, T> {
    fn eq(&self, _: &Self) -> bool {
        // can't compare these closures, and we don't care if the actual closures are equal anyway.  They are only used for storage.
        true
    }
}

/// Items representing all of the data in the DOM tree.
///
/// This is the struct emitted from the `Iterator` passed to our `diff` function. The items emitted
/// should always be in the same order, given the same input. Each entry in the enum represents
/// some aspect of a DOM node. The idea here is the sequence of items will be the same sequence of
/// things seen if we were to walk the DOM tree depth first going through all nodes and their
/// various attributes and events.
#[derive(Debug, PartialEq)]
pub enum DomItem<'a, Message> {
    /// An element in the tree.
    Element {
        /// The element name/type.
        element: &'a str,
        /// Storage for this element.
        node: Storage<'a, web_sys::Element>,
    },
    /// A text node in the tree.
    Text {
        /// The text value of the node.
        text: &'a str,
        /// Storage for this node.
        node: Storage<'a, web_sys::Text>,
    },
    /// An attribute of the last node we saw.
    Attr {
        /// The attribute name.
        name: &'a str,
        /// The attribute value.
        value: &'a str,
    },
    /// An event handler from the last node we saw.
    Event {
        /// The trigger for this event.
        trigger: &'a str,
        /// The handler for this event.
        handler: EventHandler<'a, Message>,
        /// Storage for the closure associated with this event.
        closure: Storage<'a, Closure<FnMut(web_sys::Event)>>,
    },
    /// We are finished processing children nodes, the next node is a sibling.
    Up,
}

/// This trait provides a way to iterate over a virtual dom representation.
pub trait DomIter<Message: Clone> {
    /// Return an iterator over the virtual dom.
    fn dom_iter<'a>(&'a mut self) -> Box<Iterator<Item = DomItem<'a, Message>> + 'a>;
}
