//! Traits to implement on a model to allow it to interact with an application.

use crate::app::side_effect::Commands;

/// Process a message that updates the model.
pub trait Update<Message, Command = ()> {
    /// Update the model using the given message. Implement this to describe the behavior of your
    /// app.
    fn update(&mut self, msg: Message, _commands: &mut Commands<Command>) {
        self.simple_update(msg);
    }

    /// Update the model using the given message. Implement this if your app does not need to use
    /// side effecting commands.
    fn simple_update(&mut self, _msg: Message) { }
}

/// Render (or view) the model as a virtual dom.
pub trait Render<DomTree> {
    /// Render the model as a virtual dom.
    fn render(&self) -> DomTree;
}
