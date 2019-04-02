//! Use [`typed-html`] with Euca for rendering.
//!
//! This module implements `From<DOMTree<Euca<Message>>>` for `Dom<Message>` enabling usage of
//! `typed-html` for rendring in a euca app.
//!
//! [typed-html]: https://docs.rs/crate/typed-html/

use crate::dom::*;
use std::fmt::Display;
use std::marker::PhantomData;
use typed_html::events::Events;
use typed_html::dom::{VNode, DOMTree};
use typed_html::OutputType;

impl<Message: Display + Send + Clone + 'static, Command> From<DOMTree<Euca<Message>>> for Dom<Message, Command> {
    fn from(mut node: DOMTree<Euca<Message>>) -> Self {
        (&node.vnode()).into()
    }
}

impl<'a, Message: Display + Send + Clone + 'static, Command> From<&VNode<'a, Euca<Message>>> for Dom<Message, Command> {
    fn from(vnode: &VNode<'a, Euca<Message>>) -> Self {
        match vnode {
            VNode::Text(text) => {
                Dom::text(*text)
            }
            VNode::UnsafeText(text) => {
                unsafe { Dom::elem("span").inner_html(*text) }
            }
            VNode::Element(elem) => {
                let mut e = Dom::elem(elem.name);
                e.attributes.extend(
                    elem.attributes.iter().map(|(name, value)|
                        Attr::new(name, value)
                    )
                );
                for (trigger, handler) in elem.events.iter() {
                    e = e.on(trigger, handler.clone());
                }
                e.extend(elem.children.iter().map(|child| child.into()))
            }
        }
    }
}

/// Euca OutputType for typed-html macro.
pub struct Euca<Message>(PhantomData<Message>);

impl<Message: Clone + Display + Send + 'static> OutputType for Euca<Message> {
    type Events = Events<Handler<Message>>;
    type EventTarget = ();
    type EventListenerHandle = ();
}
