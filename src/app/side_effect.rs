//! SideEffects and code to Process them.

use crate::app::Dispatcher;

/// Side effecting commands to be executed.
#[non_exhaustive]
pub struct Commands<Command> {
    /// Commands to be executed immediately after the model update.
    pub immediate: Vec<Command>,
    /// Commands to be executed after rendering.
    pub post_render: Vec<Command>,
}

impl<Command> Default for Commands<Command> {
    fn default() -> Self {
        Commands {
            immediate: vec![],
            post_render: vec![],
        }
    }
}

impl<Command> Commands<Command> {
    /// Add a command to be immediately executed after the model update.
    pub fn push(&mut self, cmd: Command) {
        self.immediate.push(cmd);
    }

    /// Returns true if there are no commands stored in the structure.
    pub fn is_empty(&self) -> bool {
        self.immediate.is_empty()
        && self.post_render.is_empty()
    }
}

/// The effect of a side-effecting command.
pub trait SideEffect<Message> {
    /// Process a side-effecting command.
    fn process(self, dispatcher: &Dispatcher<Message, Self>) where Self: Sized;
}

/// A processor for commands.
pub trait Processor<Message, Command>
where
    Command: SideEffect<Message>,
{
    /// Proccess a command.
    fn process(&self, cmd: Command, dispatcher: &Dispatcher<Message, Command>);
}

/// Default processor for commands, it just executes all side effects.
pub struct DefaultProcessor<Message, Command>
where
    Command: SideEffect<Message>,
{
    message: std::marker::PhantomData<Message>,
    command: std::marker::PhantomData<Command>,
}

impl<Message, Command> Default for DefaultProcessor<Message, Command>
where
    Command: SideEffect<Message>,
{
    fn default() -> Self {
        DefaultProcessor {
            message: std::marker::PhantomData,
            command: std::marker::PhantomData,
        }
    }
}

impl<Message, Command> Processor<Message, Command> for DefaultProcessor<Message, Command>
where
    Command: SideEffect<Message>,
{
    fn process(&self, cmd: Command, dispatcher: &Dispatcher<Message, Command>) {
        cmd.process(dispatcher);
    }
}
