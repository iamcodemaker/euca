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
pub enum Node {
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
}

impl Node {
    /// Generate an element node of the given type.
    pub fn elem(name: &'static str) -> Self {
        Node::Elem { name }
    }

    /// Generate a text node with the given value.
    pub fn text(value: String) -> Self {
        Node::Text { text: value }
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
    fn new(name: &'static str, value: &'static str) -> Self {
        Attr { name, value: value.into() }
    }
}

impl From<(&'static str, &'static str)> for Attr {
    fn from(data: (&'static str, &'static str)) -> Self {
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
pub struct Dom<Message> {
    /// The element for this node.
    element: Node,
    /// Attributes on this node.
    pub attributes: Vec<Attr>,
    /// Event handlers associated with this node.
    pub events: Vec<Event<Message>>,
    /// Children of this node.
    pub children: Vec<Dom<Message>>,
}

impl<Message> Dom<Message> {
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

    /// Append the given element as a child on this DOM element.
    pub fn push(mut self, child: impl Into<Dom<Message>>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Append the elements returned by the given iterator as children on this DOM element.
    pub fn extend(mut self, iter: impl IntoIterator<Item = Dom<Message>>) -> Self {
        self.children.extend(iter);
        self
    }
}

impl<Message> Into<Dom<Message>> for String {
    fn into(self) -> Dom<Message> {
        Dom::text(self)
    }
}

impl<Message> Into<Dom<Message>> for &str {
    fn into(self) -> Dom<Message> {
        Dom::text(self)
    }
}

impl<Message: Clone> DomIter<Message> for Dom<Message> {
    fn dom_iter<'a>(&'a self) -> Box<Iterator<Item = DomItem<'a, Message>> + 'a>
    {
        let iter = iter::once(&self.element)
            .map(|node| match node {
                Node::Elem { name } => DomItem::Element(name),
                Node::Text { text } => DomItem::Text(text),
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
pub struct DomVec<Message>(Vec<Dom<Message>>);

impl<Message> DomIter<Message> for DomVec<Message> where
    Message: Clone + PartialEq,
{
    fn dom_iter<'a>(&'a self) -> Box<Iterator<Item = DomItem<'a, Message>> + 'a> {
        Box::new(self.0.iter().flat_map(|i| i.dom_iter()))
    }
}

impl<Message> From<Vec<Dom<Message>>> for DomVec<Message> {
    fn from(v: Vec<Dom<Message>>) -> Self {
        DomVec(v)
    }
}

impl<Message> Into<Vec<Dom<Message>>> for DomVec<Message> {
    fn into(self) -> Vec<Dom<Message>> {
        self.0
    }
}

impl<Message> IntoIterator for DomVec<Message> {
    type Item = Dom<Message>;
    type IntoIter = ::std::vec::IntoIter<Dom<Message>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
