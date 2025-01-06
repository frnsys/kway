This is a heavily modified version of [ptazithos/wkeys](https://github.com/ptazithos/wkeys).

[N-grams are sourced from here](https://github.com/orgtre/google-books-ngram-frequency/tree/main/ngrams), derived from the Google Books Ngram Corpus on books published in 2010-2019.

## Dependencies

```bash
zypper in enchant-devel
```

## Features

- Written for Wayland
- Virtual split, multi-layer keyboard
- Multiple gestures per key:
  - Tap
  - Hold-repeat
  - 4-direction swipe
    - Swipe-hold
    - Swipe-release
    - Swipe-drag
- Layout configured in YAML
- Special swipe actions:
  - Move cursor (swipe-drag to move cursor in that direction)
  - Select text (swipe-drag to select text)
  - Delete text (swipe-drag to delete text)
    - NOTE: A problem with the current implementation is that if the selection is empty, one character will still be deleted.
  - Activate layer (while swipe is held)
  - Fire tapped key with a modifier (e.g. swipe up to send shifted key)


### Roadmap

- [ ] Autocorrect
- [ ] Pointer emulation
  - [ ] Mouse wheel up/down, for applications that don't support touch-drag (e.g. `kitty`)
  - [ ] Click and drag
  - [ ] Movement scaling (i.e. Quick Cursor)
- [ ] Auto-rotation
