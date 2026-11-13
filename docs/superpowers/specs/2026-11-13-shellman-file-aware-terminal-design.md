# ShellMan Fork — File-Aware Terminal (v0.2 design)

- Status: draft for builder review
- Date: 2026-11-13
- Upstream: https://github.com/scresto09/shellman (v0.1.0, GPL-3.0-or-later)
- Fork: https://github.com/Himath-Rajapaksha/shellman
- Working branch: `feat/file-aware-terminal`

## 1. Intent

ShellMan already knows two things most terminals do not: the folder Nautilus is
showing, and the files selected in it. Today that knowledge is mostly invisible.
`$SEL` exists, but a user has to know about it; there is no file context menu, no
drag-and-drop, and no visible selection state.

This fork makes the terminal feel like part of the file manager:

> A terminal that works with your files, not just next to them.

The first release is deliberately narrow. It adds file-aware actions on top of
the existing selection and quoting code, without rewriting the fragile Nautilus
integration.

## 2. Users and success

Target user: someone who keeps Nautilus open while working in a terminal and
wants to act on files without typing paths.

Success for v0.2:

- Right-click a folder and open a terminal in it.
- Select files, right-click or drag them, and get their paths inserted safely.
- See, while the terminal is open, how many files are selected and how to insert
  them.
- Never have an unquoted path typed into the shell.

## 3. Scope

### Goals (v0.2)

1. File/folder context menu:
   - `Open Terminal Here` for a single local folder.
   - `Insert into Terminal` for local files and folders.
   - `Copy Path` and `Copy as URI`.
2. Drag and drop files/folders from Nautilus onto the terminal to insert quoted
   paths.
3. A slim selection status line in the terminal panel, plus a
   `Go to Selected Folder` action.
4. `New Terminal at Tab Folder` and a `Duplicate Terminal` menu entry.
5. Engineering baseline: CI for formatting, clippy, tests, and a release build;
   fork notice and GPL attribution.

### Non-goals (v0.2)

- Terminal tabs inside the panel, session restore, or saved layouts.
- Scrollback search, font zoom, safe-paste confirmation, or palette editing.
- Support for file managers other than Nautilus.
- New shell integrations beyond bash, zsh, and fish.
- Replacing the Nautilus widget-tree injection strategy.
- Root terminals, AI features, telemetry, or network services.

### Anti-goals

- Never feed an unquoted or relative path into a shell.
- Never change the shell directory while the user is typing a command, except
  through the existing guarded `go_to` path.
- Never let a menu action silently target a different window than the one the
  user invoked it from, without at least logging the mismatch.
- Never add a menu item that appears usable but does nothing for remote files.
- Never publish binary packages under the upstream `shellman` package or app ID.

## 4. Constraints

- License is GPL-3.0-or-later. Keep `LICENSE`, preserve upstream copyright
  notices, mark modifications with a date, and keep complete source available.
- The Nautilus extension API and widget tree are unstable. New work should use
  the existing `NautilusMenuProvider` and GTK APIs where possible, not new
  undocumented Nautilus internals.
- Only local files have a usable `GFile::path()`. Remote items must be skipped
  with visible feedback.
- The terminal may be hidden or busy. Actions must define what happens in both
  cases.
- GTK 4.14, libadwaita 1.5, and VTE GTK4 are the minimums already declared by
  upstream.
- The fork must be usable alongside a source checkout of upstream for
  development, but upstream and fork binaries must not be installed at the same
  time (both would inject a panel). Document this.

## 5. Decisions made (my calls)

These are decisions I made while writing the design. They are marked so they can
be checked and overridden.

1. **`Open Terminal Here` always creates a new pane.** It never reuses or
   re-directs an existing shell, so a running command is not disturbed. If the
   panel is hidden and has no panes, it is shown with a first pane in the target
   folder. If it is hidden but already has panes, it is shown and the active
   pane is split. If visible, the active pane is split horizontally. The new
   pane spawns in the target folder.
2. **`Insert into Terminal` opens and focuses the panel if hidden.** It inserts
   only local paths and reports skipped remote items.
3. **Mixed selections insert the local paths.** The current code refuses the
   whole insertion if any selected item has no local path. v0.2 changes that to
   insert the local paths and show how many were skipped.
4. **`Copy Path` copies raw absolute paths, one per line.** `Copy as URI` copies
   `file://` URIs, one per line. These are clipboard actions, not shell input, so
   they are not quoted.
5. **The status line is always visible while the panel is shown.** With no
   selection it says `No files selected`; otherwise it shows the count and the
   insert shortcut. It is not a transient toast.
6. **`Go to Selected Folder` requires exactly one selected local directory.**
   Additional selected files or remote items do not disable it; a second local
   directory does. When ineligible, the action is disabled and the terminal
   bell/status line explains why.
7. **`New Terminal at Tab Folder` uses the Nautilus folder**, not the active
   shell's current directory, and spawns a new pane there.
8. **`Duplicate Terminal` reuses the existing `split-auto` behavior.** It spawns
   at the active shell's current directory, or the tab folder if unknown.
9. **No new preferences in v0.2.** The behavior is fixed and predictable; the
   only configurable thing is the existing insert shortcut.
10. **No binary release in v0.2.** Source and CI only until a distinct fork name
    and app ID are chosen.
11. **Explicit-directory spawns suppress follow for their first prompt.** The
    existing `Pane::spawn` calls `Panel::follow()` after the shell starts, and
    follow is on by default, so a pane opened at a subfolder would immediately
    `cd` back to the Nautilus tab folder. Panes spawned at an explicit directory
    (`Open Terminal Here`, `Duplicate Terminal`, and `New Terminal at Tab
    Folder` when the target differs from the tab folder) consume a one-shot
    skip so they stay where they were started. Ordinary splits keep the current
    follow behavior. This is a per-spawn override, not a preference change and
    not a change to the pane's visible follow toggle.
12. **Local paths are filtered at the FFI boundary; existence is re-checked at
    activation.** `FileActions::from_paths` receives only local paths. It
    requires `is_dir()` for `open_here`; insert and copy include every local
    path. At activation, a directory that has disappeared is not opened, and a
    vanished file is skipped during insertion with feedback. `Copy Path` and
    `Copy as URI` copy the selected path strings even if the file vanished
    between right-click and activation.

## 6. Feature specifications

### F0. Engineering baseline

Add `.github/workflows/ci.yml` running on pull requests and pushes to `main`:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `cargo build --release --locked`

Install the system dependencies on `ubuntu-24.04`:

```text
libgtk-4-dev libadwaita-1-dev libvte-2.91-gtk4-dev
libnautilus-extension-dev gettext
```

If `cargo test` needs a display in CI, wrap it with `xvfb-run -a`. The existing
unit tests should not need one, but the i18n test calls `setlocale` and the GTK
types may initialize lazily; verify and add Xvfb only if required.

Add a `make test` target that runs the same Rust checks, and keep
`tests/pty.sh` as the manual shell-integration test.

Add `FORK.md` at the repository root:

- upstream URL and version
- fork URL and maintainer
- date of fork
- summary of changes
- statement that the fork remains GPL-3.0-or-later
- note that upstream and fork binaries must not be installed together

Update `README.md` with a short fork banner at the top. Keep the upstream
description and credits.

### F1. File/folder context menu

`src/lib.rs` currently registers `get_file_items` but returns `NULL`. Implement
it.

The callback receives a `GList` of `NautilusFileInfo` objects. Convert each item
to a local `PathBuf` through the existing `nautilus::file_path` helper. Items
without a local path are remote/non-file items and are not actionable.

Build the menu from these rules:

| Action | Shown when | Behavior |
| --- | --- | --- |
| `Open Terminal Here` | Exactly one selected item, it is a local directory | Show the panel and create a new pane at that directory |
| `Insert into Terminal` | At least one selected item has a local path | Show/focus the panel, insert shell-quoted paths followed by a space |
| `Copy Path` | At least one selected item has a local path | Copy newline-separated absolute paths |
| `Copy as URI` | At least one selected item has a local path | Copy newline-separated `file://` URIs |

Details:

- Do not add disabled items. If a condition is not met, omit the item.
- If `Open Terminal Here` is invoked on a folder while the panel is hidden and
  has no panes, the panel is shown and the first pane spawns in that folder.
- If the panel is hidden but already has panes, it is shown and the active pane
  is split, so the action still produces a new pane at the folder.
- If the panel is visible, split the active pane horizontally and spawn the new
  pane in the folder. Focus the new pane.
- The new pane suppresses follow for its first prompt (decision 11), so it does
  not immediately `cd` back to the Nautilus tab folder.
- Re-check `is_dir()` at activation. If the folder disappeared, log, ring the
  bell, show a status message, and do not spawn.
- `Insert into Terminal` must not execute anything. It feeds text into the
  terminal input, exactly like `Ctrl+Shift+S` does today.
- `Copy Path` / `Copy as URI` do not require an injected panel; they only need
  the clipboard. If no display clipboard is available, log the failure.
- The menu action targets the active Nautilus window. This is the same
  limitation as the existing background menu: the provider callback does not
  receive the window. Log a warning if the active window has no panel. This is
  acceptable for v0.2 because right-click normally activates the window first.
- Menu labels go through `tr()`.

New FFI needed in `src/nautilus.rs` only if sensitivity is needed. Because
inapplicable items are omitted, sensitivity is not required for v0.2. If the
implementation wants to show a disabled `Open Terminal Here` for multiple
folders, add `nautilus_menu_item_set_sensitive` and use it; otherwise skip.

### F2. Drag and drop

Add a drop target to the terminal pane's scrolled window in `src/pane.rs`.

Accept both formats:

1. `gdk::FileList` (the GDK file-transfer format).
2. `text/uri-list` (the plain-text URI list Nautilus may provide).

On drop:

- Convert every entry to a local path. Ignore non-local URIs.
- If at least one local path remains, insert the shell-quoted paths with a
  trailing space and focus the terminal. Do not execute.
- If none remain, ring the terminal bell and show a status-line message such as
  `Nothing inserted: no local file in the drop`.
- Use the existing `shell::words` and `shell::quote` logic. Do not create a
  second quoting path.

Visual feedback:

- Add a CSS class to the pane on drag enter and remove it on leave/drop.
- Use a subtle inset accent outline, for example
  `box-shadow: inset 0 0 0 2px @accent_color;` on `.shellman-drop-target`.
- Do not replace the terminal's own selection or focus behavior.

Implementation notes:

- Add the controller to the scrolled window, not the VTE widget, so the whole
  pane is a drop target.
- Set the controller's propagation phase to capture so VTE does not consume the
  drop first.
- Return `true` from `connect_drop` when the drop was handled, even if all items
  were skipped, so the drag source does not fall through to another handler.

### F3. Selection status and `Go to Selected Folder`

Add a slim status line at the bottom of the terminal panel, inside the paned
end child and below the stack of panes.

Text rules:

- No selection: `No files selected`
- One local item: `1 file selected · <shortcut> to insert`
- N local items: `N files selected · <shortcut> to insert`
- If some selected items were skipped: append ` · 1 remote item skipped` or
  ` · N remote items skipped`
- If the insert shortcut is disabled: omit the shortcut hint.

The shortcut label comes from `settings::accels("insert")` and
`gtk::accelerator_get_label`, so it follows user remapping.

Update the status line when:

- the panel is shown
- the terminal pane gains focus
- the Nautilus tab/folder changes
- settings change (shortcut label)
- an action produces a transient message

Live updates while focus stays in the file view are a stretch goal. If a clean
selection-model hook is available, connect to `selection-changed` on the active
view's model and reattach when the location slot changes. Do not add a polling
timer.

Add a `Go to Selected Folder` action:

- Action name: `goto-selected-folder`
- Available in the terminal context menu and in Preferences (no default
  shortcut).
- Enabled when exactly one selected local item is a directory. Additional
  selected files or remote items do not disable it.
- On activation, call the existing `Pane::go_to` with that directory. This keeps
  the existing safety rule: the shell changes folder only when it is at a prompt
  and no command is running.

### F4. Terminal creation actions

Add two actions:

1. `new-at-tab-dir` — `New Terminal at Tab Folder`
   - Find the active pane, split it automatically (along the longer side), and
     spawn the new pane at `panel.current_dir()`.
   - If the current folder is unavailable, fall back to the active shell's
     directory, then home.
2. `duplicate` — `Duplicate Terminal`
   - Reuse the existing `split-auto` behavior: split along the longer side and
     spawn at the active shell's current directory, falling back to the tab
     folder.

Both actions appear in the terminal context menu's layout section. Add them to
`config::ACTIONS` with no default accelerator so they appear in Preferences and
can be assigned keys.

Both spawns pass an explicit directory, so the new pane suppresses follow for
its first prompt (decision 11). Without that, `Duplicate Terminal` would spawn
at the shell's directory and then immediately `cd` back to the Nautilus tab
folder.

## 7. Technical design

### 7.1 Modules and responsibilities

- `src/lib.rs`
  - Implement `get_file_items`.
  - Convert the `GList` of `NautilusFileInfo` to local paths.
  - Build and return the context menu items.
- `src/nautilus.rs`
  - Add `MenuItem::set_sensitive` only if needed.
  - Add a helper to iterate a `GList` of file-info objects safely.
- `src/panel.rs`
  - Add `active_pane()`.
  - Refactor `ensure_group()` to accept an optional start directory.
  - Refactor `split()` into `split_at(pane, orientation, dir)`.
  - Add `show_with_start(dir)`.
  - Add `open_terminal_at(dir)`.
  - Add `insert_paths(paths)`.
  - Add `copy_text(text)`.
  - Add the status line and `update_status()`.
  - Add `message(text)` for transient status messages.
- `src/pane.rs`
  - Add `insert_paths(paths)`, which filters vanished paths, reports the skipped
    count, and reuses the existing quoting; call it from `insert_selection()`.
  - Add drag-and-drop controllers.
  - Add `goto-selected-folder`, `new-at-tab-dir`, and `duplicate` actions.
  - Add the corresponding context-menu entries and enable/disable logic.
- `src/config.rs`
  - Add `goto-selected-folder`, `new-at-tab-dir`, and `duplicate` to `ACTIONS`.
- `src/util.rs`
  - Add `copy_text` if it does not belong in `panel.rs`.
- `po/shellman.pot`, `po/fr.po`
  - Add the new strings and French translations.

### 7.2 Menu provider flow

```text
Nautilus right-click
  -> get_file_items(provider, GList<NautilusFileInfo>)
  -> Vec<PathBuf> via nautilus::file_path
  -> FileActions::from_paths(paths)
  -> build MenuItem list
  -> return GList

activate "Open Terminal Here"
  -> panel::open_terminal_at(dir)
  -> active Nautilus window -> Panel
  -> show + new pane at dir

activate "Insert into Terminal"
  -> panel::insert_paths(paths)
  -> active panel -> active pane
  -> shell::words(paths) -> terminal.feed(text)
```

`FileActions` should be a small pure struct so the decision table can be unit
tested without GTK:

```rust
struct FileActions {
    open_here: Option<PathBuf>,
    insert: Vec<PathBuf>,
    copy: Vec<PathBuf>,
}
```

`from_paths` receives paths that the FFI layer has already confirmed are local.
It computes `open_here` with `is_dir()`; `insert` and `copy` contain every input
path. It does not check existence and does not create GTK objects. The FFI
callback creates `MenuItem`s from the result.

### 7.3 Panel and pane changes

Current structure:

```text
GtkPaned (vertical)
├─ start: tab view (or sidebar layout)
└─ end:   GtkStack of group holders
          └─ holder: AdwBin
             └─ pane.widget (scrolled terminal) or nested GtkPaned
```

New structure:

```text
GtkPaned (vertical)
├─ start: tab view
└─ end:   GtkBox (vertical)
          ├─ GtkStack of group holders
          └─ GtkLabel (status line)
```

The stack remains the focus ancestor used by `show`/`hide`; the status label is
not focusable. `conceal`/`hide` hide the box or both children. `reveal` measures
the box, not just the stack, so the minimum height includes the status line.

`ensure_group(Option<PathBuf>)`:

- If the current group exists and has panes, return it.
- Otherwise create the holder, group, and first pane, and spawn the pane at
  `start.unwrap_or_else(start_dir)`.
- A pane created with an explicit `start` suppresses follow for its first
  prompt.

`split_at(pane, orientation, dir)`:

- Same as the existing `split`, except the new pane spawns at
  `dir.or_else(|| pane.shell_dir()).unwrap_or_else(start_dir)`.
- When `dir` is `Some`, the new pane suppresses follow for its first prompt.
- The existing `split` becomes a wrapper with `dir = None`.

`show_with_start(start)`:

- Same as `show`, but calls `ensure_group(start)`.
- `show()` becomes `show_with_start(None)`.

`open_terminal_at(dir)`:

- If the panel is visible and has an active pane, split it horizontally at
  `dir`.
- If the panel is hidden and the current group has panes, show the panel and
  then split the active pane at `dir`.
- If the panel is hidden and the current group has no panes, show it with
  `show_with_start(Some(dir))`, which creates the first pane at `dir`.
- Focus the new pane.

`insert_paths(paths)`:

- If the panel is hidden, `show()`.
- Focus the active pane.
- Call `pane.insert_paths(paths)`, which drops paths that no longer exist and
  reports how many were skipped.
- If any were skipped, show `N missing item(s) skipped` in the status line.
- If quoting fails, log + bell + status message; do not insert a partial or
  unsafe result.

### 7.4 Drag and drop

Add two `gtk::DropTarget`s to the pane's scrolled window:

- `DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY)`
- `DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY)` for
  `text/uri-list`

Both use capture phase. Each returns `true` on drop if it recognized the format.

Path extraction:

- `gdk::FileList`: iterate `file_list.files()`, call `file.path()`.
- `text/uri-list`: split on newlines, ignore `#` comments, parse each line with
  `gio::File::for_uri`, call `.path()`.

Deduplicate paths while preserving order. Then call the same
`pane.insert_paths(&paths)` used by the context menu. That function drops paths
that no longer exist and reports the skipped count.

### 7.5 Status line

Add to `Panel`:

```rust
status: gtk::Label,
status_message: RefCell<Option<String>>,
```

`update_status()` builds the summary from `self.selected()` and the insert
shortcut. `message(text)` stores a transient message, updates the label, and
uses `glib::timeout_add_local_once(Duration::from_secs(3), ...)` to clear it and
restore the summary. The timeout closure holds a weak reference to the panel.

Call `update_status()` from `refresh_selection()` and from the end of
`tab_changed()`. `refresh_selection()` already runs on show and pane focus.

If live selection updates are implemented, connect to the active view's
`selection-changed` signal. Keep the handler id in the panel so it can be
disconnected when the location slot changes.

### 7.6 Quoting and safety

All shell insertion goes through `shell::words`:

- Paths must be absolute.
- Non-UTF-8 names are handled by the existing per-shell quoting.
- Control characters and bidi overrides are rejected.
- A failure inserts nothing and reports why.

`Copy Path` and `Copy as URI` are not shell input; they use raw text.

No new code may call `terminal.feed()` with a path unless it came from
`shell::words` or is a fixed key sequence.

### 7.7 Error handling and feedback

- Menu actions with no active panel: log and return.
- Remote-only selections: omit the action; if a mixed selection reaches insert,
  insert locals and show `N remote item(s) skipped` in the status line.
- Paths that vanished between selection and activation: skip them during
  insertion and show `N missing item(s) skipped`.
- Quoting failure: bell + status message, no insertion.
- Clipboard failure: log; no crash.
- Drag with no local files: bell + status message.
- `Open Terminal Here` with a folder that disappeared: log + bell; do not spawn
  in a stale path.

### 7.8 i18n and accessibility

- All new user-visible strings use `tr()` or `ntr()`.
- Update `po/shellman.pot`; add French entries to `po/fr.po`.
- Menu items are text-only; the terminal button keeps its existing label.
- The status label uses `dim-label` and `ellipsize` so long messages do not
  resize the panel.
- Do not make the status label focusable or a tab stop.
- The drag target is an enhancement; every drag action also has a menu
  equivalent, so keyboard users are not blocked.

## 8. Testing

### Unit tests

- `FileActions::from_paths`:
  - one local folder -> `open_here` set
  - two folders -> `open_here` none
  - one file + one folder -> `open_here` none, insert both
  - empty -> all empty
  - nonexistent path -> `open_here` none, but still present in insert/copy
- Status summary:
  - 0, 1, and N files
  - skipped count
  - shortcut present/absent
- Existing `shell::quote` tests must keep passing.

The FFI path filter (local vs remote) is manual-tested because it needs
`NautilusFileInfo` objects.

### Manual test matrix

Run at least on the Nautilus version available on the development machine. The
human can test on this machine only, so record its distribution and
`nautilus --version` in the PR and do not claim broader coverage.

Cases:

1. Right-click a folder -> `Open Terminal Here` with panel hidden and no
   previous panes.
2. Right-click a folder -> `Open Terminal Here` with panel hidden but an
   existing pane; the old pane must be untouched and a new pane must open at the
   folder.
3. Right-click a folder -> `Open Terminal Here` with panel visible and a command
   running in the active pane; the running command must not be interrupted.
4. `Open Terminal Here` on a subfolder with follow enabled: the new pane must
   stay in that subfolder after its first prompt, not `cd` back to the tab
   folder.
5. Select one file -> `Insert into Terminal`; the path appears quoted and is not
   executed.
6. Select multiple files including hostile names (`;`, quotes, newline, `$(...)`,
   bidi override) -> insertion is safe.
7. Select a remote/non-local item plus a local file -> local inserted, skipped
   count shown.
8. Drag one file, many files, and a folder onto the terminal.
9. Drag a remote-only item -> nothing inserted, feedback shown.
10. `Copy Path` and `Copy as URI` paste correctly into a text editor.
11. Status line updates after selecting files and refocusing the terminal.
12. `Go to Selected Folder` with one folder, with one folder plus one file
    (enabled), with a file only, and with multiple folders (disabled).
13. `New Terminal at Tab Folder` while the active shell is in a different
    directory.
14. `Duplicate Terminal` preserves the active shell's directory with follow
    enabled.
15. Existing behavior: F4 toggle, follow folder, `$SEL`, splits, close, colors,
    preferences, shell exit.
16. Two Nautilus windows: actions target the active window; log a warning if no
    panel is found.

### CI

The CI workflow must pass on a clean checkout. If `cargo clippy -D warnings`
surfaces existing warnings, fix them in separate commits before enabling the
gate, or start with `-D warnings` only for the new modules and document the
remaining allow list. Prefer fixing.

## 9. Fork identity, license, docs

- Keep `LICENSE` unchanged.
- Add `FORK.md` with upstream attribution, fork maintainer, date, and change
  summary.
- Update `README.md` with a fork banner and a link to `FORK.md`.
- Update `Cargo.toml`:
  - `repository` -> fork URL
  - `authors` -> `["Sylvain Cresto", "Himath Rajapaksha"]`
  - `homepage` -> keep upstream until a fork page exists (or point to fork
    README)
- Update the About dialog to credit both the original author and the fork
  maintainer. Keep `License::Gpl30` or switch to the "or later" variant if the
  GTK enum supports it; the project license is GPL-3.0-or-later.
- Do not change the app ID, package name, or installed `.so` name in v0.2. Do
  not publish binary packages. Before the first binary release, choose a
  distinct fork name, app ID, package name, `.so` name, icon, and metainfo.
- Add an `upstream` remote and document the sync policy: rebase feature branches
  on `upstream/main`; never merge upstream into a feature branch without a
  review.

## 10. Release plan

1. `feat/file-aware-terminal` branch: F0 + F1 + F2 + F3 + F4, tests, docs.
2. Open a PR against the fork's `main`.
3. CI must pass; manual test notes in the PR.
4. Merge to `main`; tag a source-only `v0.2.0-fork` if desired.
5. Only after the fork name/app ID decision: packaging and binary release.

## 11. Resolved with the human (2026-11-13)

1. **Fork identity:** source-only for v0.2. Keep the `shellman` name, app ID, and
   package names; no binary packages. Choose a distinct name before the first
   binary release.
2. **`Open Terminal Here`:** always create a new pane. Never reuse or re-direct
   an existing shell.
3. **Status line:** always visible while the panel is shown.
4. **Manual testing:** development machine only. Record its distribution and
   `nautilus --version` in the PR; do not claim broader Nautilus coverage.

## 12. Acceptance criteria

- `.github/workflows/ci.yml` exists and passes `fmt`, `clippy`, `test`, and
  release build.
- `FORK.md` and the README fork banner exist; GPL notices are intact.
- Right-click folder -> `Open Terminal Here` opens a new pane at that folder,
  including when the panel was hidden with existing panes.
- A pane opened at an explicit directory stays there after its first prompt
  when follow is enabled.
- Right-click files -> `Insert into Terminal` inserts quoted paths, no execution.
- Drag and drop inserts quoted local paths and reports skipped remote items.
- Vanished paths are skipped with feedback, never inserted.
- Status line shows the selection count and the current insert shortcut.
- `Go to Selected Folder` works when exactly one selected item is a directory,
  even if files are also selected, and refuses when there is no single folder.
- `New Terminal at Tab Folder` uses the Nautilus folder.
- `Duplicate Terminal` uses the active shell's folder.
- No unquoted path is ever fed to the terminal.
- Existing F4/follow/`$SEL`/split/close/colors/preferences behavior is unchanged.
- No binary package or app-ID change is published in v0.2.
