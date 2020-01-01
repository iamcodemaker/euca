//! A concrete dom representation.
//!
//! This is a sample, but functional concrete DOM representation that demonstrates how a DOM
//! structure works with other parts of this library.

use std::iter;
use crate::vdom::*;

/// A DOM event handler.
#[derive(PartialEq, Debug)]
pub enum Handler<Message> {
    /// The message that will result from the event this handler is attached to.
    Msg(Message),
    /// A function that will convert a [`web_sys::Event`] event to a Message.
    ///
    /// [`web_sys::Event`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Event.html
    Event(fn(web_sys::Event) -> Message),
    /// A function that will convert a [`web_sys::Event`] event to a Message.
    ///
    /// This variation allows passing data to the event handler via a Message.
    ///
    /// [`web_sys::Event`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Event.html
    MsgEvent(Message, fn(Message, web_sys::Event) -> Message),
    /// A function that will convert a String from an input element into a Message.
    InputValue(fn(String) -> Message),
    /// A function that will convert a [`web_sys::InputEvent`] event to a Message.
    ///
    /// [`web_sys::InputEvent`]: https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.InputEvent.html
    InputEvent(fn(web_sys::InputEvent) -> Message),
}

/// A DOM event.
#[derive(Debug)]
pub struct Event<Message> {
    /// The event trigger (e.g. click, change, etc.).
    trigger: &'static str,
    /// The handler for this event.
    handler: Handler<Message>,
}

/// Representation of a DOM node.
#[derive(Debug)]
pub enum Node<Message, Command> {
    /// A DOM element node.
    Elem {
        /// The element name/type.
        name: &'static str,
    },
    /// A DOM text node.
    Text {
        /// The text of this node.
        text: String,
    },
    /// A component.
    Component {
        /// A message to pass to the component.
        msg: Message,
        /// A function to create the component.
        create: fn(web_sys::Element, Dispatcher<Message, Command>) -> Box<dyn Component<Message>>,
    }
}

impl<Message, Command> Node<Message, Command> {
    /// Generate an element node of the given type.
    pub fn elem(name: &'static str) -> Self {
        Node::Elem { name }
    }

    /// Generate a text node with the given value.
    pub fn text(value: String) -> Self {
        Node::Text { text: value }
    }

    /// Generate a component.
    pub fn component(msg: Message, create: fn(web_sys::Element, Dispatcher<Message, Command>) -> Box<dyn Component<Message>>) -> Self {
        Node::Component { msg, create }
    }
}

/// An attribute on a node.
#[derive(PartialEq, Debug)]
pub struct Attr {
    /// The name of the attribute.
    name: &'static str,
    /// The value of the attribute.
    value: String,
}

impl Attr {
    fn new(name: &'static str, value: &str) -> Self {
        Attr { name, value: value.into() }
    }
}

impl From<(&'static str, &str)> for Attr {
    fn from(data: (&'static str, &str)) -> Self {
        let (name, value) = data;
        Attr::new(name, value)
    }
}

impl From<(&'static str, String)> for Attr {
    fn from(data: (&'static str, String)) -> Self {
        let (name, value) = data;
        Attr { name, value }
    }
}

/// A node in the DOM.
#[derive(Debug)]
pub struct Dom<Message, Command> {
    /// The element for this node.
    element: Node<Message, Command>,
    /// Attributes on this node.
    pub attributes: Vec<Attr>,
    /// Event handlers associated with this node.
    pub events: Vec<Event<Message>>,
    /// Children of this node.
    pub children: Vec<Dom<Message, Command>>,
}

impl<Message, Command> Dom<Message, Command> {
    /// Create a new DOM element node.
    pub fn elem(element: &'static str) -> Self {
        Dom {
            element: Node::elem(element),
            events: vec![],
            attributes: vec![],
            children: vec![],
        }
    }

    /// Create a new DOM text node.
    pub fn text(value: impl Into<String>) -> Self {
        Dom {
            element: Node::text(value.into()),
            events: vec![],
            attributes: vec![],
            children: vec![],
        }
    }

    /// Create a component.
    pub fn component(msg: Message, create: fn(web_sys::Element, Dispatcher<Message, Command>) -> Box<dyn Component<Message>>) -> Self {
        Dom {
            element: Node::component(msg, create),
            events: vec![],
            attributes: vec![],
            children: vec![],
        }
    }

    /// Add an attribute to this DOM element.
    pub fn attr(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.attributes.push(Attr { name, value: value.into() });
        self
    }

    /// Add an event listener to this DOM element.
    pub fn event(self, trigger: &'static str, msg: Message) -> Self {
        self.on(trigger, Handler::Msg(msg))
    }

    /// Add an event listener to this DOM element.
    pub fn on(mut self, trigger: &'static str, handler: Handler<Message>) -> Self {
        self.events.push(
            Event {
                trigger: trigger,
                handler: handler,
            }
        );
        self
    }

    /// Add a change event listener to this DOM element.
    pub fn onchange(self, handler: fn(String) -> Message) -> Self {
        self.on("change", Handler::InputValue(handler))
    }

    /// Add an input event listener to this DOM element.
    pub fn oninput(self, handler: fn(web_sys::InputEvent) -> Message) -> Self {
        self.on("input", Handler::InputEvent(handler))
    }

    /// Append the given element as a child on this DOM element.
    pub fn push(mut self, child: impl Into<Dom<Message, Command>>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Append the elements returned by the given iterator as children on this DOM element.
    pub fn extend(mut self, iter: impl IntoIterator<Item = Dom<Message, Command>>) -> Self {
        self.children.extend(iter);
        self
    }
}

impl<Message, Command> Into<Dom<Message, Command>> for String {
    fn into(self) -> Dom<Message, Command> {
        Dom::text(self)
    }
}

impl<Message, Command> Into<Dom<Message, Command>> for &str {
    fn into(self) -> Dom<Message, Command> {
        Dom::text(self)
    }
}

impl<Message: Clone, Command> DomIter<Message, Command> for Dom<Message, Command> {
    fn dom_iter<'a>(&'a self) -> Box<dyn Iterator<Item = DomItem<'a, Message, Command>> + 'a>
    {
        let iter = iter::once(&self.element)
            .map(|node| match node {
                Node::Elem { name } => DomItem::Element(name),
                Node::Text { text } => DomItem::Text(text),
                Node::Component { msg, create } => DomItem::Component { msg: msg.clone(), create: *create },
            })
            .chain(self.attributes.iter()
                .map(|attr| DomItem::Attr {
                    name: attr.name,
                    value: &attr.value
                })
            )
            .chain(self.events.iter()
                .map(|Event { trigger, handler }|
                     DomItem::Event {
                         trigger: trigger,
                         handler: match handler {
                             Handler::Msg(m) => EventHandler::Msg(m),
                             Handler::Event(h) => EventHandler::Fn(*h),
                             Handler::MsgEvent(m, h) => EventHandler::FnMsg(m, *h),
                             Handler::InputValue(h) => EventHandler::InputValue(*h),
                             Handler::InputEvent(h) => EventHandler::InputEvent(*h),
                         },
                     }
                 )
            )
            .chain(self.children.iter()
               .flat_map(|c| c.dom_iter())
            )
            .chain(iter::once(DomItem::Up));

        Box::new(iter)
    }
}

/// A sequence of DOM entries.
///
/// This structure allows a top level sequence of DOM entries to be represented without requiring a
/// containing DOM element.
#[derive(Debug)]
pub struct DomVec<Message, Command>(Vec<Dom<Message, Command>>);

impl<Message, Command> DomIter<Message, Command> for DomVec<Message, Command> where
    Message: Clone + PartialEq,
{
    fn dom_iter<'a>(&'a self) -> Box<dyn Iterator<Item = DomItem<'a, Message, Command>> + 'a> {
        Box::new(self.0.iter().flat_map(|i| i.dom_iter()))
    }
}

impl<Message, Command> From<Vec<Dom<Message, Command>>> for DomVec<Message, Command> {
    fn from(v: Vec<Dom<Message, Command>>) -> Self {
        DomVec(v)
    }
}

impl<Message, Command> Into<Vec<Dom<Message, Command>>> for DomVec<Message, Command> {
    fn into(self) -> Vec<Dom<Message, Command>> {
        self.0
    }
}

impl<Message, Command> IntoIterator for DomVec<Message, Command> {
    type Item = Dom<Message, Command>;
    type IntoIter = ::std::vec::IntoIter<Dom<Message, Command>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
