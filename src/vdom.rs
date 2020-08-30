//! Flexible generic virtual dom representation.
//!
//! The types here can be used to generically represent a virtual dom tree. This generic
//! representation can be used to plug various concrete virtual dom representations into the
//! [`diff`] and [`patch`] algorithms implemented in this crate.
//!
//! [`diff`]: ../diff/fn.diff.html
//! [`patch`]: ../patch/enum.Patch.html

use std::fmt;
use std::mem;
use wasm_bindgen::prelude::*;
pub use crate::component::Component;
pub use crate::app::Dispatcher;

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
    Fn(fn(web_sys::Event) -> Option<Message>),

    /// A callback that will convert a [`web_sys::Event`] into a message.
    ///
    /// This variation accepts a message to pass data into the callback.
    ///
    /// [`web_sys::Event`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Event.html
    FnMsg(&'a Message, fn(Message, web_sys::Event) -> Option<Message>),

    /// This callback will recieve the value of a form input and convert it to a message.
    InputValue(fn(String) -> Option<Message>),

    /// A function that will convert a [`web_sys::InputEvent`] event to a Message.
    ///
    /// [`web_sys::InputEvent`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.InputEvent.html
    InputEvent(fn(web_sys::InputEvent) -> Option<Message>),
}

/// A DOM node or JS closure created when applying a patch.
pub enum WebItem<Message> {
    /// A DOM element.
    Element(web_sys::Element),
    /// A DOM text node.
    Text(web_sys::Text),
    /// A JS closure.
    Closure(Closure<dyn FnMut(web_sys::Event)>),
    /// A component.
    Component(Box<dyn Component<Message>>),
    /// A previously occupied, now empty storage entry.
    Taken,
    /// The end of a node.
    ///
    /// Used for tracking the depth in the tree. We need this so we can find the top level elements
    /// in the storage vec.
    Up,
}

impl<Message> WebItem<Message> {
    /// Swap this WebItem with WebItem::Taken and return the item.
    pub fn take(&mut self) -> Self {
        let mut taken = WebItem::Taken;
        mem::swap(self, &mut taken);
        taken
    }

    /// Possibly get a reference to the web_sys::Element in this WebItem.
    pub fn as_element(&self) -> Option<&web_sys::Element> {
        match self {
            WebItem::Element(node) => Some(node),
            _ =>  None,
        }
    }

    /// Possibly get a reference to the web_sys::Text in this WebItem.
    pub fn as_text(&self) -> Option<&web_sys::Text> {
        match self {
            WebItem::Text(node) => Some(node),
            _ =>  None,
        }
    }

    /// Possibly get a reference to the js_sys::Closure in this WebItem.
    pub fn as_closure(&self) -> Option<&Closure<dyn FnMut(web_sys::Event)>> {
        match self {
            WebItem::Closure(closure) => Some(closure),
            _ =>  None,
        }
    }

    /// Possibly get a reference to the Component in this WebItem.
    pub fn as_component(&self) -> Option<&Box<dyn Component<Message>>> {
        match self {
            WebItem::Component(c) => Some(c),
            _ =>  None,
        }
    }
}

impl<Message> fmt::Debug for WebItem<Message> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WebItem::Element(node) => write!(f, "Element({:?})", node),
            WebItem::Text(text) => write!(f, "Text({:?})", text),
            WebItem::Closure(_) => write!(f, "Closure(_)"),
            WebItem::Component(_) => write!(f, "Component(_)"),
            WebItem::Taken => write!(f, "Taken"),
            WebItem::Up => write!(f, "Up"),
        }
    }
}

/// A list of [`WebItem`]s.
///
/// The list should match the traversal order of the vDOM tree we are operating on.
///
/// [`WebItem`]: enum.WebItem.html
pub type Storage<Message> = Vec<WebItem<Message>>;

/// Items representing all of the data in the DOM tree.
///
/// This is the struct emitted from the `Iterator` passed to our `diff` function. The items emitted
/// should always be in the same order, given the same input. Each entry in the enum represents
/// some aspect of a DOM node. The idea here is the sequence of items will be the same sequence of
/// things seen if we were to walk the DOM tree depth first going through all nodes and their
/// various attributes and events.
#[derive(Debug, PartialEq)]
pub enum DomItem<'a, Message, Command, K> {
    /// An element in the tree.
    Element {
        /// The element name.
        name: &'a str,
        /// An optional key for this element. Should have been generated from a type implementing
        /// [`Hash`] using a [`Hasher`].
        ///
        /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
        /// [`Hasher`]: https://doc.rust-lang.org/std/hash/trait.Hasher.html
        key: Option<&'a K>,
    },
    /// A text node in the tree.
    Text(&'a str),
    /// Raw HTML code to be rendered using innerHTML. Use with caution as this can be used as an
    /// attack vector to execute arbitrary code in the client's browser.
    UnsafeInnerHtml(&'a str),
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
    },
    /// We are finished processing children nodes, the next node is a sibling.
    Up,
    /// A component.
    Component {
        /// An optional key for this component.
        ///
        /// This is necessary if a component has internal state that must be maintained between dom
        /// updates.
        key: Option<&'a K>,
        /// A message to send to the component.
        // XXX msg: &'a Message,
        msg: Message,
        /// A function to create the component if necessary.
        create: fn(Dispatcher<Message, Command>) -> Box<dyn Component<Message>>,
    },
    /// For internal use. This is a reference to a keyed item.
    Key(&'a K),
}

/// This trait provides a way to iterate over a virtual dom representation.
pub trait DomIter<Message: Clone, Command, K> {
    /// Return an iterator over the virtual dom.
    fn dom_iter<'a>(&'a self) -> Box<dyn Iterator<Item = DomItem<'a, Message, Command, K>> + 'a>;
}
