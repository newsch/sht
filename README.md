# sht
_an early, unfinished terminal csv editor_

Current features:
- editing cells
- adding/removing columns/rows
- command palette automatically generated from available actions
- delta-based Undo/Redo system
- chorded keybindings
  - in-progress chords show kakoune-style pop-up menus
- `tui-rs` + `crossterm` based UI
- built-in logger similar to [`cursive`'s debug view](https://docs.rs/cursive/latest/cursive/views/struct.DebugView.html)
- [proper terminal cleanup handling](https://werat.dev/blog/pretty-rust-backtraces-in-raw-terminal-mode/)
- all program state can be serialized to disk on panic and reloaded

[![asciicast](https://asciinema.org/a/cRD2rBd0Cq8ytVBIOx09DXYqj.svg)](https://asciinema.org/a/cRD2rBd0Cq8ytVBIOx09DXYqj)
