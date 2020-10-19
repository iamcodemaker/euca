# Euca • [TodoMVC](http://todomvc.com)

This is the classic TodoMVC app implemented using Euca.

## Implementation

This implementation uses sever Euca features of note. A router is used to
handle navigation changes. Side effects are used to interact with browser
storage and focus the cursor. All standard TodoMVC functionality should be
present.

## Building and Running

Run the following commands to serve this example using live-server. Any
modifications to the app will be detected, causing the app to automatically
rebuild and reload.

This depends on the [`fd`] and [`entr`] utilities.

```
npm install
script/watch
```

## Credit

Created by [Matthew Nicholson](https://github.com/iamcodemaker)

[`fd`]: https://github.com/sharkdp/fd
[`entr`]: https://github.com/clibs/entr
