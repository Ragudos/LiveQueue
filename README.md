# LiveQueue

An application that can use web hooks to display the current ticket number for registrars, so students can do their stuff while waiting for their turn during busy days and long lines.

## Design Choices

### Why HTMX?

HTMX is lightweight, and all our application needs to do is display text and nothing else. This will be friendly to students who are low on internet data or are in low network coverage areas.

### Why SSE?

The website only needs to listen to data sent by the server, and it doesn't need to send anything to the server frequently that is realtime.

### Why Web Hooks?

Well, web hooks are just endpoints on a server that can be posted to by a sender (registrar), to which it notifies the receivers (SSE).

### Why Rust?

I am not sure if it's relevant, but I like it.
