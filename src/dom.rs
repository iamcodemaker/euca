use std::fmt;
use std::cmp;
use wasm_bindgen::prelude::*;

#[derive(Debug, PartialEq, Copy, Clone)] pub enum EventHandler<'a, Message> {
    Msg(&'a Message),
    Fn(fn(web_sys::Event) -> Message),
}

pub enum Storage<'a, T> {
    Read(Box<FnMut() -> T + 'a>),
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
    Element { element: &'a str, node: Storage<'a, web_sys::Element> },
    /// A text node in the tree.
    Text { text: &'a str, node: Storage<'a, web_sys::Text> },
    /// An attribute of the last node we saw.
    Attr { name: &'a str, value: &'a str },
    /// An event handler from the last node we saw.
    Event { trigger: &'a str, handler: EventHandler<'a, Message>, closure: Storage<'a, Closure<FnMut(web_sys::Event)>> },
    /// We are finished processing children nodes, the next node is a sibling.
    Up,
}

pub trait DomIter<Message: Clone> {
    fn dom_iter<'a>(&'a mut self) -> Box<Iterator<Item = DomItem<'a, Message>> + 'a>;
}
