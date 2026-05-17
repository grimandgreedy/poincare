# Changelog

All notable changes to this crate will be documented in this file.

This project uses internal pre-release versioning. Until `1.0.0`, breaking changes may be released in minor versions.

## [Unreleased]

### Features
- Added document-level undo/redo with menu items, command-palette actions, and `Cmd/Ctrl+Z` / `Cmd/Ctrl+Shift+Z` shortcuts.

### Fixes
- Focus on plot properties tab by default.

## [0.3.0]

- Added a dedicated `Camera` tab as a second bottom `grimdock` tab alongside plot properties.
- Added animated camera controls for named views, framing, projection switching, and saved camera slots.
- Routed camera actions through shared commands so the bottom camera tab, View menu, command palette, axis indicator, and shortcuts use consistent behaviour.
- Added a `Help` menu with a `Keyboard Shortcuts` modal documenting the app's current shortcuts.
- Refactored expression-parameter sweep controls into a reusable scalar control component shared with domain range editing.
- Cleaned up plot property layout for parameter/domain controls, including improved alignment, compact sizing, and better behaviour in wide inspector panes.

## [0.2.0] - Initial pre-release

- Initial crate release.
