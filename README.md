# Euca

Euca is an experimental front end web development library designed to be modular. The
diffing, patching, and main application code all interact with each other via a set
of traits that allow alternative implementations to be substituted as desired.

## Design

The user facing portions of the library mostly follow [The Elm Architecture] design,
in which there is a model which contains data, an update function which accepts
events and updates the model, and a view function (we call it render) which renders
the model into a virtual dom. In Elm, everything is immutable, so the update function
accepts a model and returns changes to that model as a new model. In Rust, we don't
need this limitation so our update function here directly mutates the model. This is
more ergonomic and intuitive for those not accustom to function programming. 

Internally, the vDOM is represented by an iterator facade which is utilized by the
diffing algorithm to traverse the vDOM. The diff operation results in a series of
patches that describe the changes to make to the DOM and the patching algorithm
applies these changes to the browser's DOM. All of these operations are loosely
coupled and can be modified and replaced independently.

### Testing

> Code without tests is bad code. It doesn't matter how well written it is; it
> doesn't matter how pretty or object-oriented or well-encapsulated it is. With
> tests, we can change the behavior of our code quickly and verifiably. Without them,
> we really don't know if our code is getting better or worse.

― Michael Feathers, Working Effectively with Legacy Code 

Euca was designed with testability in mind. The design of Euca ([The Elm
Architecture]) allows very straight forward unit testing. Because messages/events are
the only way to update the model, verifying the behavior of the application can be
centered around the update function. Given a model, processing an event should result
in certain changes to the model. Similarly, the render function cannot modify the
model and given a certain model, the function should produce a specific vDOM.

Side effects like http requests and interacting with storage or browser history are
isolated to commands returned from the update function. This way users can verify the update
function produces the correct side effects, with out actually executing the side
effects.

## Motivation

There are [many web development frameworks written in Rust]. Most of them
experimental and in the early stages, and they all represent the virtual DOM in a
unique way. I had the thought that the representation of the DOM isn't important for
users of libraries. Users should be able to represent the DOM in the way that is most
comfortable to them. If that's a declarative, JSX
[macro](https://github.com/bodil/typed-html) style or a function oriented elm style,
the framework should accept anything as an input.  Similarly, each framework
implements it's own vDOM diffing and patching algorithms, this is redundant. Why
don't all the frameworks just use the best algorithms.

I thought, why do we have frameworks at all? What if we could break out each part,
vDOM representation, diffing, patching and compose them as we saw fit. Euca an
experiment to explore this space.

## Inspiration

This library was heavily inspired by [Elm] and [Willow] with additional inspiration
from [Draco] and [Seed].

## Limitations

### No closures

Because closures do not implement [`PartialEq`], which is used when diffing vDOM
nodes, closures are not supported when handling events. Simple function pointers are
supported, but these cannot capture any state and only operate on the arguments
provided. In my limited use of the library, I haven't found this to be a major issue,
but most other web frameworks in Rust support closures, so integrating Euca with
those libraries isn't possible without significant modification. This is a major
limitation, as the whole idea was to integrate with existing libraries.

### Unoptimized

The diff algorithm is completely unoptimized. My intention was to demonstrate that
building a modular framework was possible, not to implement the fastest algorithm.

### Composition

It isn't currently possible to compose two applications that use different message
types. In order to do this, both applications have to be converted to use the same
message type. There may be a better way to address this, more exploration is needed.

## TODO

- Targeted DOM matching for testing. When testing the render function, it would
  be useful to compare just a small subsection of the DOM for validation (maybe using
css [selectors](https://docs.rs/selectors/0.21.0/selectors/)).

[The Elm Architecture]: https://guide.elm-lang.org/architecture/
[many web development frameworks written in Rust]: https://github.com/flosse/rust-web-framework-comparison#frontend-frameworks-wasm
[Elm]: https://elm-lang.org/
[Willow]: https://github.com/sindreij/willow
[Draco]: https://github.com/utkarshkukreti/draco
[Seed]: https://github.com/David-OConnor/seed
[`PartialEq`]: https://doc.rust-lang.org/std/cmp/trait.PartialEq.html
