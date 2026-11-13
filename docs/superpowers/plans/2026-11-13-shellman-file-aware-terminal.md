# File-Aware Terminal (v0.2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the ShellMan terminal act on the files and folders selected in Nautilus: folder/file context-menu actions, drag-and-drop insertion, a selection status line, and new/duplicate terminal actions.

**Architecture:** Keep the existing Nautilus widget-tree injection and `NautilusMenuProvider` FFI. Add a pure `FileActions` decision table and a pure status-summary function so the behavior is unit-tested without GTK. Extend `Panel` with an explicit-directory spawn path, a one-shot follow skip, a status label, and clipboard/insert helpers. Extend `Pane` with safe insertion, drag-and-drop, and three new actions. No Nautilus internals beyond the existing menu provider and file-info FFI.

**Tech Stack:** Rust 2021, gtk4-rs 0.10 (GTK 4.14+), libadwaita 0.8 (1.5+), VTE GTK4 through the existing FFI, GNU gettext, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-13-shellman-file-aware-terminal-design.md`

## Global Constraints

- License is GPL-3.0-or-later. Keep `LICENSE` unchanged, preserve upstream copyright notices, and mark fork modifications with the date 2026-11-13.
- Do not change the app ID, package name, installed `.so` name, icon, or metainfo in v0.2. Do not publish binary packages.
- Minimums stay GTK 4.14, libadwaita 1.5, and VTE GTK4.
- Do not add preferences in v0.2. New actions appear in Preferences but have no default accelerator.
- Every path fed to a terminal must go through `shell::words` or `shell::quote`. Paths must be absolute. No new code may call `terminal.feed()` with a path.
- All new user-visible strings use `tr()` or `ntr()`, and `po/shellman.pot` plus `po/fr.po` are updated.
- Do not run `cargo fmt` repository-wide. Upstream is hand-formatted (170 hunks); new files use rustfmt style, and CI has no fmt gate.
- CI runs `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `cargo build --release --locked`.
- Manual testing is on Debian 13 with Nautilus 48.3 only. Record that in the PR; do not claim broader coverage.
- Remote/non-local items are never inserted. They are skipped with visible feedback.
- The status line is always visible while the panel is shown.

## Review Focus

1. **Hostile file names** (`;`, quotes, newline, `$(...)`, bidi/control characters): insertion from the menu, drag-and-drop, and `$SEL` must stay quoted or be refused, never executed.
2. **Paths that vanish between menu construction and activation**: a vanished directory must not be opened; vanished files must be skipped with a count, never inserted.
3. **Mixed local/remote selections**: local paths are inserted, remote items are skipped with feedback; `Open Terminal Here` is omitted unless the whole selection is one local folder.
4. **Follow-folder with explicit-directory spawns**: a pane opened at a subfolder must stay there after its first prompt; ordinary splits keep following the tab folder.
5. **Malformed `text/uri-list` payloads**: CRLF, comments, duplicates, and non-file URIs must be ignored or deduplicated without panicking.

---

## File Structure

- `src/file_actions.rs` (create): pure `FileActions` decision table, path filtering, deduplication, URI-list parsing, and clipboard text formatting.
- `src/status.rs` (create): pure selection-summary text.
- `src/style.rs` (create): drop-target CSS provider.
- `src/lib.rs` (modify): module list and the `get_file_items` menu provider.
- `src/nautilus.rs` (modify): safe `GList<NautilusFileInfo>` iteration.
- `src/panel.rs` (modify): status line, active pane, explicit-directory spawn/split, insertion and clipboard entry points, active-window helpers.
- `src/pane.rs` (modify): safe insertion, drag-and-drop, `goto-selected-folder`, `new-at-tab-dir`, `duplicate`.
- `src/selection.rs` (modify): `folder_target` predicate.
- `src/config.rs` (modify): three new actions.
- `src/util.rs` (modify): clipboard helper.
- `src/i18n.rs` (modify): deterministic locale test.
- `src/prefs.rs` (modify): About credits.
- `po/update.sh`, `po/shellman.pot`, `po/fr.po` (modify): extraction and translations.
- `Cargo.toml`, `Cargo.lock`, `Makefile`, `README.md`, `FORK.md`, `.github/workflows/ci.yml` (modify/create): baseline and attribution.

---

### Task 1: Baseline, CI, and fork attribution

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `FORK.md`
- Modify: `Makefile`
- Modify: `README.md`
- Modify: `Cargo.toml`, `Cargo.lock`
- Modify: `src/i18n.rs:36-49`
- Modify: `src/prefs.rs:300-314`
- Modify: `po/update.sh:12`

**Interfaces:**
- Produces: a CI workflow named `CI`, a `make test` target, and fork metadata used by the About dialog and the PR.

- [ ] **Step 1: Reproduce the locale test failure**

Run: `LANGUAGE=en_US:en cargo test --lib i18n::tests::follows_the_locale`
Expected: FAIL with `left: "Copy"`, `right: "Copier"`.

- [ ] **Step 2: Make the test deterministic**

Add this as the first line of `i18n::tests::follows_the_locale`:

```rust
std::env::remove_var("LANGUAGE");
```

GNU gettext gives `LANGUAGE` priority over `LC_MESSAGES`, so the test must clear it before calling `init()`.

- [ ] **Step 3: Verify the test passes with the hostile environment**

Run: `LANGUAGE=en_US:en cargo test --lib i18n::tests::follows_the_locale`
Expected: PASS.

- [ ] **Step 4: Add the CI workflow**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  check:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v7
      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libgtk-4-dev libadwaita-1-dev \
            libvte-2.91-gtk4-dev libnautilus-extension-dev gettext locales
          sudo locale-gen fr_FR.UTF-8
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
      - name: Test
        run: cargo test --all-targets
      - name: Release build
        run: cargo build --release --locked
```

Do not add `cargo fmt --check` or Xvfb. The unit tests do not create GTK widgets or open a display. Generating `fr_FR.UTF-8` makes the French assertions in `i18n::tests::follows_the_locale` run instead of being skipped.

- [ ] **Step 5: Add the `make test` target**

Append to `Makefile`:

```make
test:
	cargo clippy --all-targets -- -D warnings
	cargo test --all-targets
	cargo build --release --locked
```

Keep the existing `all` and `install` targets unchanged.

- [ ] **Step 6: Add fork metadata**

`Cargo.toml`:

```toml
version = "0.2.0"
authors = ["Sylvain Cresto", "Himath Rajapaksha"]
repository = "https://github.com/Himath-Rajapaksha/shellman"
```

Keep `homepage = "https://github.com/scresto09/shellman"` until the fork has its own page. Run `cargo check` once so `Cargo.lock` records 0.2.0.

Create `FORK.md` with these facts:

- Upstream: `https://github.com/scresto09/shellman`, v0.1.0.
- Fork: `https://github.com/Himath-Rajapaksha/shellman`, maintained by Himath Rajapaksha.
- Fork date: 2026-11-13.
- License: GPL-3.0-or-later; upstream copyright Sylvain Cresto; fork modifications copyright Himath Rajapaksha.
- Change summary: file-aware terminal (context menu, drag-and-drop, status line, new/duplicate terminal).
- Warning: upstream and fork binaries must not be installed at the same time; both inject a panel into Nautilus.
- Sync policy: rebase feature branches on `upstream/main`; never merge upstream into a feature branch without a review.
- v0.2 is source-only; no binary packages.

Add a fork banner at the top of `README.md`:

```markdown
> **Fork:** [Himath-Rajapaksha/shellman](https://github.com/Himath-Rajapaksha/shellman),
> maintained by Himath Rajapaksha. Based on
> [scresto09/shellman](https://github.com/scresto09/shellman) v0.1.0.
> GPL-3.0-or-later. This fork is source-only for v0.2; see [FORK.md](FORK.md).
```

Keep the upstream description, screenshot, features, and credits below the banner. Add one line to the installation section: the fork does not publish packages in v0.2; build from source.

- [ ] **Step 7: Update the About dialog and translation bug address**

In `src/prefs.rs:300-314`, set:

```rust
.developer_name("Sylvain Cresto, Himath Rajapaksha")
.developers(["Sylvain Cresto", "Himath Rajapaksha"])
.copyright("© 2026 Sylvain Cresto · fork modifications © 2026 Himath Rajapaksha")
```

Keep `license_type(gtk::License::Gpl30)`; GTK's `Gpl30` means GPL-3.0-or-later, while `Gpl30Only` would be wrong. Keep the website and issue URL using `CARGO_PKG_HOMEPAGE` / `CARGO_PKG_REPOSITORY`.

In `po/update.sh`, change the `--msgid-bugs-address` value to `https://github.com/Himath-Rajapaksha/shellman/issues`.

- [ ] **Step 8: Run the baseline checks**

Run: `cargo test --all-targets`
Expected: 12 tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS.

Run: `make test`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/i18n.rs
git commit -m "test: make the locale test independent of LANGUAGE"

git add .github/workflows/ci.yml Makefile
git commit -m "ci: add clippy, tests, and release build"

git add Cargo.toml Cargo.lock README.md FORK.md src/prefs.rs po/update.sh
git commit -m "docs: add fork attribution and v0.2 metadata"
```

---

### Task 2: Pure file-action decision table

**Files:**
- Create: `src/file_actions.rs`
- Modify: `src/lib.rs` (add `pub mod file_actions;`)
- Test: `src/file_actions.rs` (unit tests)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `FileActions { open_here: Option<PathBuf>, insert: Vec<PathBuf>, copy: Vec<PathBuf> }`
  - `FileActions::from_paths(paths: Vec<PathBuf>, skipped: usize) -> FileActions`
  - `existing(paths: &[PathBuf]) -> (Vec<PathBuf>, usize)`
  - `dedupe(paths: Vec<PathBuf>) -> Vec<PathBuf>`
  - `parse_uri_list(text: &str) -> Vec<PathBuf>`
  - `path_text(paths: &[PathBuf]) -> String`
  - `uri_text(paths: &[PathBuf]) -> String`

- [ ] **Step 1: Write the failing tests**

Create `src/file_actions.rs` with only the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_folder_can_open_a_terminal() {
        let a = FileActions::from_paths(vec![PathBuf::from("/")], 0);
        assert_eq!(a.open_here, Some(PathBuf::from("/")));
        assert_eq!(a.insert, vec![PathBuf::from("/")]);
    }

    #[test]
    fn two_folders_cannot_open_a_terminal() {
        let a = FileActions::from_paths(vec![PathBuf::from("/"), PathBuf::from("/tmp")], 0);
        assert_eq!(a.open_here, None);
        assert_eq!(a.insert.len(), 2);
    }

    #[test]
    fn a_file_and_a_folder_cannot_open_a_terminal() {
        let a = FileActions::from_paths(
            vec![PathBuf::from("/nope-file"), PathBuf::from("/")], 0);
        assert_eq!(a.open_here, None);
        assert_eq!(a.insert.len(), 2);
    }

    #[test]
    fn a_remote_item_blocks_open_terminal_here() {
        let a = FileActions::from_paths(vec![PathBuf::from("/")], 1);
        assert_eq!(a.open_here, None);
        assert_eq!(a.insert, vec![PathBuf::from("/")]);
    }

    #[test]
    fn empty_selection_has_no_actions() {
        let a = FileActions::from_paths(Vec::new(), 0);
        assert_eq!(a.open_here, None);
        assert!(a.insert.is_empty());
        assert!(a.copy.is_empty());
    }

    #[test]
    fn vanished_paths_stay_in_insert_and_copy() {
        let p = PathBuf::from("/definitely-not-here-shellman");
        let a = FileActions::from_paths(vec![p.clone()], 0);
        assert_eq!(a.open_here, None);
        assert_eq!(a.insert, vec![p.clone()]);
        assert_eq!(a.copy, vec![p]);
    }

    #[test]
    fn existing_splits_paths_and_counts_missing() {
        let present = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let missing = present.with_extension("toml.shellman-missing");
        let (found, skipped) = existing(&[present.clone(), missing]);
        assert_eq!(found, vec![present]);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn dedupe_keeps_first_seen_order() {
        let a = PathBuf::from("/a");
        let b = PathBuf::from("/b");
        assert_eq!(dedupe(vec![a.clone(), b.clone(), a.clone()]), vec![a, b]);
    }

    #[test]
    fn parse_uri_list_ignores_comments_duplicates_and_non_files() {
        let text = "file:///tmp/a\n# comment\r\nfile:///tmp/a\nhttps://example.com/x\n";
        assert_eq!(parse_uri_list(text), vec![PathBuf::from("/tmp/a")]);
        assert_eq!(parse_uri_list("file:///tmp/a%20b"), vec![PathBuf::from("/tmp/a b")]);
    }

    #[test]
    fn clipboard_text_is_newline_separated() {
        let paths = vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")];
        assert_eq!(path_text(&paths), "/tmp/a\n/tmp/b");
        assert_eq!(uri_text(&paths), "file:///tmp/a\nfile:///tmp/b");
    }
}
```

Add `use std::path::PathBuf;` above the test module and `pub mod file_actions;` to `src/lib.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib file_actions`
Expected: FAIL with `cannot find function` / `cannot find type FileActions`.

- [ ] **Step 3: Implement the module**

At the top of `src/file_actions.rs`:

```rust
use std::path::PathBuf;

use gtk::gio;
use gtk::gio::prelude::FileExt;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FileActions {
    pub open_here: Option<PathBuf>,
    pub insert: Vec<PathBuf>,
    pub copy: Vec<PathBuf>,
}
```

Implement `from_paths` as: `open_here` is `Some` only when `skipped == 0`, `paths.len() == 1`, and that path `is_dir()`; `insert` and `copy` are the full input vector. Do not check existence beyond `is_dir()`.

Implement `existing` with `partition(|p| p.exists())`; return the present paths and the missing count.

Implement `dedupe` by walking the input and pushing only paths not already present, preserving first-seen order.

Implement `parse_uri_list` by iterating `text.lines()`, trimming whitespace and a trailing `\r`, skipping empty lines and lines starting with `#`, calling `gio::File::for_uri(line).path()`, keeping only absolute paths, and returning `dedupe(paths)`. Filtering to absolute paths keeps a `x-special/gnome-copied-files` style first line (`copy`) from becoming a relative path.

Implement `path_text` with `p.display().to_string()` joined by `\n`, and `uri_text` with `gio::File::for_path(p).uri()` joined by `\n`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib file_actions`
Expected: 10 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/file_actions.rs
git commit -m "feat: add file action decision table"
```

---

### Task 3: Selection status line

**Files:**
- Create: `src/status.rs`
- Modify: `src/lib.rs` (add `pub mod status;`)
- Modify: `src/panel.rs:43-55` (struct fields), `src/panel.rs:190-231` (show/reveal/conceal/hide), `src/panel.rs:405-412` (`refresh_selection`), `src/panel.rs:425-444` (`build`), `src/panel.rs:258-264` (`apply_settings`), `src/panel.rs:163-188` (`tab_changed`)
- Test: `src/status.rs`

**Interfaces:**
- Consumes: `selection::Selection`, `settings::accels`.
- Produces:
  - `status::summary(files: usize, skipped: usize, shortcut: Option<&str>) -> String`
  - `Panel::message(self: &Rc<Self>, text: String)`
  - `Panel::update_status(&self)`

- [ ] **Step 1: Write the failing status-summary tests**

Create `src/status.rs` with the i18n import and only the tests:

```rust
use crate::i18n::{ntr, tr};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_selection_says_so() {
        assert_eq!(summary(0, 0, Some("Ctrl+Shift+S")), tr("No files selected"));
    }

    #[test]
    fn one_file_uses_the_singular_form() {
        let s = summary(1, 0, Some("Ctrl+Shift+S"));
        assert_eq!(s, ntr("{n} file selected", "{n} files selected", 1)
            .replace("{n}", "1") + " · " + &tr("{shortcut} to insert")
            .replace("{shortcut}", "Ctrl+Shift+S"));
    }

    #[test]
    fn many_files_use_the_plural_form_and_shortcut() {
        let s = summary(3, 0, Some("Ctrl+Shift+S"));
        assert!(s.contains('3'));
        assert!(s.contains("Ctrl+Shift+S"));
    }

    #[test]
    fn disabled_shortcut_is_omitted() {
        let s = summary(2, 0, None);
        assert!(!s.contains("to insert"));
        assert!(s.contains('2'));
    }

    #[test]
    fn skipped_items_are_reported() {
        let s = summary(2, 1, Some("Ctrl+Shift+S"));
        assert!(s.contains('2'));
        assert!(s.contains('1'));
        assert!(s.contains(" · "));
    }

    #[test]
    fn skipped_only_selection_still_reports_them() {
        let s = summary(0, 2, None);
        assert_eq!(s, tr("No files selected") + " · " +
            &ntr("{n} remote item skipped", "{n} remote items skipped", 2).replace("{n}", "2"));
    }
}
```

Add `use std::path::PathBuf;` only if needed; these tests need `tr`/`ntr`, which the module imports. Add `pub mod status;` to `src/lib.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib status`
Expected: FAIL with `cannot find function summary`.

- [ ] **Step 3: Implement `status::summary`**

Keep the existing `use crate::i18n::{ntr, tr};` at the top of `src/status.rs`
and add:

```rust
pub fn summary(files: usize, skipped: usize, shortcut: Option<&str>) -> String
```

Build the base text with `tr("No files selected")` for zero files, otherwise `ntr("{n} file selected", "{n} files selected", files as u32).replace("{n}", &files.to_string())`. When `files > 0` and `shortcut` is `Some`, append `" · "` plus `tr("{shortcut} to insert").replace("{shortcut}", shortcut)`. When `skipped > 0`, append `" · "` plus `ntr("{n} remote item skipped", "{n} remote items skipped", skipped as u32).replace("{n}", &skipped.to_string())`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib status`
Expected: 6 tests pass.

- [ ] **Step 5: Add the status label to `Panel`**

Add fields to `Panel`:

```rust
body: gtk::Box,
status: gtk::Label,
status_message: RefCell<Option<(u64, String)>>,
status_seq: Cell<u64>,
```

In `build`, replace `paned.set_end_child(Some(&stack))` with:

```rust
let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
body.set_visible(false);
stack.set_vexpand(true);
body.append(&stack);

let status = gtk::Label::new(None);
status.set_xalign(0.0);
status.set_hexpand(true);
status.set_ellipsize(gtk::pango::EllipsizeMode::End);
status.set_focusable(false);
status.set_selectable(false);
status.set_margin_start(6);
status.set_margin_end(6);
status.set_margin_top(2);
status.set_margin_bottom(2);
status.add_css_class("dim-label");
body.append(&status);

paned.set_end_child(Some(&body));
```

Add `body`, `status`, `status_message: RefCell::new(None)`, and `status_seq: Cell::new(0)` to the `Panel` initializer. Keep `stack.set_visible(false)`.

- [ ] **Step 6: Wire the status label**

Implement:

```rust
fn update_status(&self) {
    if let Some((_, msg)) = self.status_message.borrow().as_ref() {
        self.status.set_text(msg);
        return;
    }
    let sel = self.selected();
    let shortcut = settings::accels("insert").first()
        .and_then(gtk::accelerator_parse)
        .map(|(k, m)| gtk::accelerator_get_label(k, m).to_string());
    self.status.set_text(&status::summary(
        sel.paths.len(), sel.skipped, shortcut.as_deref()));
}

pub fn message(self: &Rc<Self>, text: String) {
    let seq = self.status_seq.get() + 1;
    self.status_seq.set(seq);
    self.status.set_text(&text);
    self.status_message.replace(Some((seq, text)));
    let weak = Rc::downgrade(self);
    glib::timeout_add_local_once(Duration::from_secs(3), move || {
        if let Some(p) = weak.upgrade() {
            if p.status_seq.get() == seq {
                p.status_message.replace(None);
                p.update_status();
            }
        }
    });
}
```

Call `self.update_status()` at the end of `refresh_selection`, `apply_settings`, and `tab_changed`. `refresh_selection` already runs from `show` and `pane_focused`.

- [ ] **Step 7: Make the new body the measured/hidden child**

In `reveal`, replace `self.stack.set_visible(true)` with:

```rust
self.body.set_visible(true);
self.stack.set_visible(true);
```

and measure `self.body` instead of `self.stack`:

```rust
let min = self.body.measure(gtk::Orientation::Vertical, -1).0;
```

In `conceal` and `hide`, replace `self.stack.set_visible(false)` with:

```rust
self.body.set_visible(false);
self.stack.set_visible(false);
```

Keep the focus checks on `self.stack`; the status label is not focusable.

- [ ] **Step 8: Verify**

Run: `cargo test --all-targets`
Expected: all tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/lib.rs src/status.rs src/panel.rs
git commit -m "feat: add selection status line"
```

---

### Task 4: Explicit-directory spawn, split, and follow skip

**Files:**
- Modify: `src/panel.rs:109-131` (`ensure_group`), `src/panel.rs:190-208` (`show`), `src/panel.rs:266-301` (`follow`, `split`), `src/panel.rs:741-747` (active-panel helpers)
- Modify: `src/pane.rs:20-34` (fields), `src/pane.rs:260-285` (`spawn`)

**Interfaces:**
- Consumes: `Panel::message` from Task 3.
- Produces:
  - `Panel::active_pane(&self) -> Option<Rc<Pane>>`
  - `Panel::ensure_group(self: &Rc<Self>, start: Option<PathBuf>) -> usize`
  - `Panel::split_at(self: &Rc<Self>, pane: &Rc<Pane>, orientation: gtk::Orientation, dir: Option<PathBuf>)`
  - `Panel::split(self: &Rc<Self>, pane: &Rc<Pane>, orientation: gtk::Orientation)`
  - `Panel::show_with_start(self: &Rc<Self>, start: Option<PathBuf>)`
  - `Panel::open_terminal_at(self: &Rc<Self>, dir: &Path)`
  - `panel::open_terminal_at(dir: &Path)` (active window)
  - `Pane::spawn_at(self: &Rc<Self>, dir: &Path, suppress_follow: bool)`
  - `Pane::take_follow_skip(&self) -> bool`

- [ ] **Step 1: Add the one-shot follow skip to `Pane`**

Add `skip_follow: Cell<bool>` to `Pane` and initialize it to `Cell::new(false)`.

Rename the body of `spawn` to `spawn_at`:

```rust
pub fn spawn(self: &Rc<Self>, dir: &Path) {
    self.spawn_at(dir, false);
}

pub fn spawn_at(self: &Rc<Self>, dir: &Path, suppress_follow: bool) {
    if self.pid.get().is_some() || self.spawning.get() {
        return;
    }
    if suppress_follow {
        self.skip_follow.set(true);
    }
    // existing spawn body unchanged
}
```

Set the flag only after the early-return guard; otherwise a call on an already-running pane would leave a stale skip behind. In the spawn callback, if `pid` is `None`, clear `skip_follow` so a failed spawn does not suppress a later follow.

Add:

```rust
pub fn take_follow_skip(&self) -> bool {
    self.pid.get().is_some() && self.skip_follow.replace(false)
}
```

The `pid.is_some()` guard prevents `show()` or `tab_changed()` from consuming the skip while the shell is still spawning.

- [ ] **Step 2: Skip follow once in `Panel::follow`**

In the per-pane loop, compute the target as before, then:

```rust
let target = if is_current && p.follows() { dir.clone() } else { None };
if target.is_some() && p.take_follow_skip() {
    continue;
}
p.go_to(target);
```

Consume the skip only when there is a real follow target. Consuming it on a `None` target (panel hidden or follow disabled) would make the pane follow the tab folder later, which is exactly what the explicit spawn must avoid.

- [ ] **Step 3: Refactor group creation and splitting**

Change `ensure_group` to `ensure_group(self: &Rc<Self>, start: Option<PathBuf>) -> usize`. Keep the existing-group early return. For a new pane:

```rust
let start = start.filter(|d| d.is_dir());
let dir = start.clone().unwrap_or_else(|| self.start_dir());
pane.spawn_at(&dir, start.is_some());
```

Change `show_current_group` to take `start: Option<PathBuf>` and call `self.ensure_group(start)`. Update `tab_changed` to call `self.show_current_group(None)`; it has no explicit start directory.

Add `active_pane`:

```rust
pub fn active_pane(&self) -> Option<Rc<Pane>> {
    let i = self.current_group()?;
    let g = &self.groups.borrow()[i];
    g.active.upgrade().or_else(|| g.panes.first().cloned())
}
```

Change `split` to `split_at(self: &Rc<Self>, pane: &Rc<Pane>, orientation: gtk::Orientation, dir: Option<PathBuf>)`. Keep the existing body, but compute the spawn target as:

```rust
let explicit = dir.as_ref().is_some_and(|d| d.is_dir());
let target = dir.clone()
    .filter(|d| d.is_dir())
    .or_else(|| pane.shell_dir().filter(|d| d.is_dir()))
    .unwrap_or_else(|| self.start_dir());
new.spawn_at(&target, explicit);
```

Pass `explicit`, not `dir.is_some()`: a vanished explicit directory falls back to another directory and must not suppress follow.

Keep `pub fn split(...)` as a wrapper that calls `split_at(pane, orientation, None)`.

- [ ] **Step 4: Add `show_with_start` and `open_terminal_at`**

Rename the body of `show` to `show_with_start(self: &Rc<Self>, start: Option<PathBuf>)`, and call `self.show_current_group(start)` inside it. Keep `show()` as a wrapper calling `show_with_start(None)`. `show_current_group` should take `start: Option<PathBuf>`.

Implement:

```rust
pub fn open_terminal_at(self: &Rc<Self>, dir: &Path) {
    if !dir.is_dir() {
        log!("open terminal here: {} is gone", dir.display());
        self.win.error_bell();
        self.message(tr("Folder no longer exists"));
        return;
    }
    let dir = dir.to_path_buf();
    if let Some(pane) = self.active_pane() {
        if !self.shown() {
            self.show();
        }
        self.split_at(&pane, gtk::Orientation::Horizontal, Some(dir));
    } else {
        self.show_with_start(Some(dir));
    }
}
```

This is the decision-1 behavior: hidden with no panes creates the first pane at `dir`; hidden with panes shows and splits; visible splits. In all three paths the result is a new pane at `dir`.

- [ ] **Step 5: Add the active-window entry point**

Add a private helper next to `toggle_active`:

```rust
fn active_panel() -> Option<Rc<Panel>> {
    let win = app()?.active_window()?;
    let panel = unsafe { win.data::<Rc<Panel>>(DATA_KEY) }?;
    Some(Rc::clone(panel))
}

pub fn open_terminal_at(dir: &Path) {
    let Some(panel) = active_panel() else {
        log!("open terminal here: no terminal in the active window");
        return;
    };
    panel.open_terminal_at(dir);
}
```

Import `std::path::Path`.

- [ ] **Step 6: Verify**

Run: `cargo test --all-targets`
Expected: all tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS.

Manual check (record for Task 10): with follow enabled, open a terminal at a subfolder and confirm the first prompt stays in that subfolder.

- [ ] **Step 7: Commit**

```bash
git add src/panel.rs src/pane.rs
git commit -m "refactor: spawn panes at explicit directories"
```

---

### Task 5: File context menu, insertion, and clipboard

**Files:**
- Modify: `src/lib.rs:29-31` (`get_file_items`)
- Modify: `src/nautilus.rs` (add `file_paths`)
- Modify: `src/panel.rs` (insert/copy helpers)
- Modify: `src/pane.rs:352-369` (`insert_selection`), add `insert_paths`
- Modify: `src/util.rs` (add `copy_text`)
- Modify: `src/shell.rs` (add a multi-path regression test)

**Interfaces:**
- Consumes: `FileActions`, `existing`, `path_text`, `uri_text` from Task 2; `Panel::message` from Task 3; `panel::open_terminal_at` from Task 4.
- Produces:
  - `nautilus::file_paths(list: *mut GList) -> (Vec<PathBuf>, usize)`
  - `util::copy_text(text: &str) -> bool`
  - `Pane::insert_paths(&self, paths: &[PathBuf]) -> Option<usize>`
  - `Panel::insert_paths(self: &Rc<Self>, paths: &[PathBuf], remote_skipped: usize)`
  - `panel::insert_paths(paths: &[PathBuf], remote_skipped: usize)`
  - `panel::copy_paths(paths: &[PathBuf])`
  - `panel::copy_uris(paths: &[PathBuf])`

- [ ] **Step 1: Add the multi-path quoting regression test**

In `src/shell.rs` tests, add:

```rust
#[test]
fn words_join_hostile_paths() {
    let paths = vec![
        PathBuf::from("/tmp/a b"),
        PathBuf::from("/tmp/x';$(id)'"),
    ];
    assert_eq!(
        words(Shell::Bash, &paths, true).as_deref(),
        Some("'/tmp/a b' '/tmp/x'\\'';$(id)'\\''' ")
    );
}
```

Run: `cargo test --lib shell::tests::words_join_hostile_paths`
Expected: PASS. This is a regression pin: it proves multi-path insertion keeps one quoting path and separates arguments with a single space.

- [ ] **Step 2: Add safe GList iteration**

In `src/nautilus.rs`:

```rust
pub fn file_paths(list: *mut GList) -> (Vec<PathBuf>, usize) {
    let mut paths = Vec::new();
    let mut skipped = 0;
    let mut node = list;
    while !node.is_null() {
        let data = unsafe { (*node).data };
        let path = if data.is_null() {
            None
        } else {
            let obj: glib::Object =
                unsafe { from_glib_none(data as *mut gobject_ffi::GObject) };
            file_path(&obj)
        };
        match path {
            Some(p) => paths.push(p),
            None => skipped += 1,
        }
        node = unsafe { (*node).next };
    }
    (paths, skipped)
}
```

This counts remote/non-file items in `skipped`; the paths vector contains only local paths.

- [ ] **Step 3: Add the clipboard helper**

In `src/util.rs`:

```rust
pub fn copy_text(text: &str) -> bool {
    use gtk::prelude::*;
    let Some(display) = gtk::gdk::Display::default() else {
        return false;
    };
    display.clipboard().set_text(text);
    true
}
```

- [ ] **Step 4: Add safe insertion to `Pane`**

Add `pending_feed: RefCell<String>` to `Pane`, initialize it with `RefCell::new(String::new())`, and clear it in `stop()`.

Add:

```rust
/// Inserts quoted paths, dropping paths that no longer exist.
/// Returns the number of skipped paths, or None if quoting was refused.
pub fn insert_paths(&self, paths: &[PathBuf]) -> Option<usize> {
    let (present, missing) = crate::file_actions::existing(paths);
    if present.is_empty() {
        self.term.widget().error_bell();
        return Some(missing);
    }
    let Some(text) = shell::words(self.shell, &present, shell::utf8_locale()) else {
        log!("insertion refused: a file name cannot be written safely for this shell");
        self.term.widget().error_bell();
        return None;
    };
    if self.pid.get().is_some() {
        self.term.feed(&text);
    } else {
        // `Insert into Terminal` can create this pane and insert immediately;
        // `feed` writes to the PTY, so it must wait for the spawn callback.
        self.pending_feed.borrow_mut().push_str(&text);
    }
    Some(missing)
}
```

In the spawn callback, replace the pid handling with:

```rust
p.spawning.set(false);
p.pid.set(pid);
if pid.is_none() {
    p.skip_follow.set(false);
    return;
}
let text = std::mem::take(&mut *p.pending_feed.borrow_mut());
if !text.is_empty() {
    p.term.feed(&text);
}
if let Some(panel) = p.panel.upgrade() {
    panel.follow();
}
```

`vte_terminal_feed_child` sends bytes to the child PTY; feeding before the shell exists would be lost. The pending buffer preserves an insertion made against a just-created pane.

Replace `insert_selection` with:

```rust
fn insert_selection(self: &Rc<Self>) {
    let Some(panel) = self.panel.upgrade() else { return };
    let sel = panel.selected();
    if sel.paths.is_empty() && sel.skipped == 0 {
        self.term.widget().error_bell();
        return;
    }
    panel.insert_paths(&sel.paths, sel.skipped);
}
```

- [ ] **Step 5: Add the panel insertion and copy entry points**

In `panel.rs`:

```rust
pub fn insert_paths(self: &Rc<Self>, paths: &[PathBuf], remote_skipped: usize) {
    self.show();
    let Some(pane) = self.active_pane() else { return };
    pane.focus();
    let Some(missing) = pane.insert_paths(paths) else {
        self.message(tr("Cannot insert: a file name cannot be written safely for this shell"));
        return;
    };
    let mut parts = Vec::new();
    if remote_skipped > 0 {
        parts.push(ntr("{n} remote item skipped", "{n} remote items skipped",
                       remote_skipped as u32).replace("{n}", &remote_skipped.to_string()));
    }
    if missing > 0 {
        parts.push(ntr("{n} missing item skipped", "{n} missing items skipped",
                       missing as u32).replace("{n}", &missing.to_string()));
    }
    if !parts.is_empty() {
        self.message(parts.join(" · "));
    }
}

pub fn insert_paths(paths: &[PathBuf], remote_skipped: usize) {
    let Some(panel) = active_panel() else {
        log!("insert into terminal: no terminal in the active window");
        return;
    };
    panel.insert_paths(paths, remote_skipped);
}

pub fn copy_paths(paths: &[PathBuf]) {
    if !paths.is_empty() && !util::copy_text(&file_actions::path_text(paths)) {
        log!("clipboard: cannot copy paths");
    }
}

pub fn copy_uris(paths: &[PathBuf]) {
    if !paths.is_empty() && !util::copy_text(&file_actions::uri_text(paths)) {
        log!("clipboard: cannot copy URIs");
    }
}
```

Import `ntr`, `crate::file_actions`, and `crate::util` in `panel.rs`. `Panel::insert_paths` must be `pub` because `Pane::insert_selection` calls it.

- [ ] **Step 6: Implement `get_file_items`**

Replace the stub in `src/lib.rs`:

```rust
unsafe extern "C" fn get_file_items(_p: gpointer, files: *mut GList) -> *mut GList {
    guard("file menu", ptr::null_mut(), || {
        let (paths, remote) = nautilus::file_paths(files);
        let actions = file_actions::FileActions::from_paths(paths, remote);
        let mut items = Vec::new();

        if let Some(dir) = actions.open_here.clone() {
            let item = MenuItem::new("ShellMan::open-terminal-here",
                &tr("Open Terminal Here"),
                &tr("Open a new terminal in this folder"));
            item.on_activate(move || panel::open_terminal_at(&dir));
            items.push(item);
        }

        if !actions.insert.is_empty() {
            let paths = actions.insert.clone();
            let item = MenuItem::new("ShellMan::insert-into-terminal",
                &tr("Insert into Terminal"),
                &tr("Insert the selected paths into the terminal"));
            item.on_activate(move || panel::insert_paths(&paths, remote));
            items.push(item);
        }

        if !actions.copy.is_empty() {
            let paths = actions.copy.clone();
            let item = MenuItem::new("ShellMan::copy-path",
                &tr("Copy Path"),
                &tr("Copy the selected paths"));
            item.on_activate(move || panel::copy_paths(&paths));
            items.push(item);

            let paths = actions.copy.clone();
            let item = MenuItem::new("ShellMan::copy-uri",
                &tr("Copy as URI"),
                &tr("Copy the selected paths as file:// URIs"));
            item.on_activate(move || panel::copy_uris(&paths));
            items.push(item);
        }

        nautilus::menu_list(items)
    })
}
```

Add `use crate::file_actions;` to `src/lib.rs`.

- [ ] **Step 7: Verify**

Run: `cargo test --all-targets`
Expected: all tests pass, including `words_join_hostile_paths`.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/lib.rs src/nautilus.rs src/panel.rs src/pane.rs src/util.rs src/shell.rs
git commit -m "feat: add file context menu actions"
```

---

### Task 6: Drag and drop onto the terminal

**Files:**
- Create: `src/style.rs`
- Modify: `src/lib.rs` (add `pub mod style;`)
- Modify: `src/panel.rs:779-792` (`try_start`)
- Modify: `src/pane.rs:47-73` (`Pane::new`)

**Interfaces:**
- Consumes: `file_actions::parse_uri_list`, `dedupe`, `Pane::insert_paths`.
- Produces: `style::install()`, a `shellman-drop-target` CSS class, and a drop target on every pane.

- [ ] **Step 1: Add the drop-target style**

Create `src/style.rs`:

```rust
pub fn install() {
    let Some(display) = gtk::gdk::Display::default() else { return };
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".shellman-drop-target { box-shadow: inset 0 0 0 2px @accent_color; }",
    );
    gtk::style_context_add_provider_for_display(
        &display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}
```

Add `pub mod style;` to `src/lib.rs`. In `try_start`, call `crate::style::install();` next to `crate::icons::install();`.

- [ ] **Step 2: Add the drop target in `Pane::new`**

After the pane `Rc` is created, add two drop targets: one for `gdk::FileList`, one for `text/uri-list`. Use a shared installer and drop handler:

```rust
fn install_drop_target(pane: &Rc<Pane>, ty: glib::Type, uri_only: bool,
                       extract: fn(&glib::Value) -> Vec<PathBuf>) {
    let target = gtk::DropTarget::new(ty, gdk::DragAction::COPY);
    target.set_propagation_phase(gtk::PropagationPhase::Capture);
    if uri_only {
        target.connect_accept(|_, d| d.formats().contain_mime_type("text/uri-list"));
    }

    let weak = Rc::downgrade(pane);
    target.connect_enter(move |_, _, _| {
        if let Some(p) = weak.upgrade() {
            p.widget.add_css_class("shellman-drop-target");
        }
        gdk::DragAction::COPY
    });

    let weak = Rc::downgrade(pane);
    target.connect_leave(move |_| {
        if let Some(p) = weak.upgrade() {
            p.widget.remove_css_class("shellman-drop-target");
        }
    });

    let weak = Rc::downgrade(pane);
    target.connect_drop(move |_, value, _, _| {
        let paths = extract(value);
        let Some(p) = weak.upgrade() else { return false };
        p.widget.remove_css_class("shellman-drop-target");
        guard("drop", false, || drop_paths(&p, paths))
    });

    pane.widget.add_controller(target);
}

fn drop_paths(p: &Rc<Pane>, paths: Vec<PathBuf>) -> bool {
    let Some(panel) = p.panel.upgrade() else { return false };
    if paths.is_empty() {
        p.term.widget().error_bell();
        panel.message(tr("Nothing inserted: no local file in the drop"));
        return true;
    }
    p.focus();
    match p.insert_paths(&paths) {
        Some(0) => {}
        Some(n) => panel.message(
            ntr("{n} missing item skipped", "{n} missing items skipped", n as u32)
                .replace("{n}", &n.to_string())),
        None => panel.message(
            tr("Cannot insert: a file name cannot be written safely for this shell")),
    }
    true
}

install_drop_target(&pane, gdk::FileList::static_type(), false, |value| {
    value.get::<gdk::FileList>()
        .map(|list| file_actions::dedupe(
            list.files().into_iter().filter_map(|f| f.path()).collect()))
        .unwrap_or_default()
});
install_drop_target(&pane, glib::Type::STRING, true, |value| {
    value.get::<String>()
        .map(|text| file_actions::parse_uri_list(&text))
        .unwrap_or_default()
});
```

Add imports: `use crate::file_actions;` and `use gtk::gio::prelude::FileExt;` (for `f.path()`). The `FileList` target accepts by type; the string target accepts only `text/uri-list`, so ordinary text drops still reach VTE. Return `true` for every recognized drop, including a remote-only drop, so the drag source does not fall through.

- [ ] **Step 3: Verify**

Run: `cargo test --all-targets`
Expected: all tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS.

Manual check (record for Task 10): drag one file, many files, and a folder onto the terminal; drag a remote-only item and confirm the bell plus status message.

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/style.rs src/panel.rs src/pane.rs
git commit -m "feat: accept file drops on the terminal"
```

---

### Task 7: `Go to Selected Folder`

**Files:**
- Modify: `src/config.rs:7-21` (`ACTIONS`)
- Modify: `src/selection.rs` (add `folder_target`)
- Modify: `src/pane.rs:128-184` (actions), `src/pane.rs:371-439` (menu)

**Interfaces:**
- Consumes: `Selection`, `Panel::selected`, `Panel::message`, `Pane::go_to`.
- Produces:
  - `selection::folder_target(sel: &Selection) -> Option<PathBuf>`
  - Pane action `goto-selected-folder`
  - Config entry `("goto-selected-folder", N_!("Go to Selected Folder"))`

- [ ] **Step 1: Write the failing predicate tests**

In `src/selection.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_target_requires_exactly_one_directory() {
        let one = Selection { paths: vec![PathBuf::from("/")], skipped: 0 };
        assert_eq!(folder_target(&one), Some(PathBuf::from("/")));

        let with_file = Selection {
            paths: vec![PathBuf::from("/"), PathBuf::from("/nope-file")],
            skipped: 1,
        };
        assert_eq!(folder_target(&with_file), Some(PathBuf::from("/")));

        let file_only = Selection { paths: vec![PathBuf::from("/nope-file")], skipped: 0 };
        assert_eq!(folder_target(&file_only), None);

        let two = Selection {
            paths: vec![PathBuf::from("/"), PathBuf::from("/tmp")],
            skipped: 0,
        };
        assert_eq!(folder_target(&two), None);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib selection::tests::folder_target_requires_exactly_one_directory`
Expected: FAIL with `cannot find function folder_target`.

- [ ] **Step 3: Implement the predicate**

```rust
pub fn folder_target(sel: &Selection) -> Option<PathBuf> {
    let mut dirs = sel.paths.iter().filter(|p| p.is_dir());
    let first = dirs.next()?;
    if dirs.next().is_some() {
        return None;
    }
    Some(first.clone())
}
```

Additional files and skipped remote items do not matter because only directories are counted.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib selection::tests::folder_target_requires_exactly_one_directory`
Expected: PASS.

- [ ] **Step 5: Add the action**

Add to `config::ACTIONS`, after `goto-tab-dir`:

```rust
("goto-selected-folder", N_!("Go to Selected Folder")),
```

Add the pane action:

```rust
add("goto-selected-folder", |p| p.goto_selected_folder());
```

Implement:

```rust
fn goto_selected_folder(self: &Rc<Self>) {
    let Some(panel) = self.panel.upgrade() else { return };
    let Some(dir) = selection::folder_target(&panel.selected()) else {
        self.term.widget().error_bell();
        panel.message(tr("Select exactly one folder"));
        return;
    };
    if !self.integrated.get() {
        self.term.widget().error_bell();
        return;
    }
    self.go_to(Some(dir));
}
```

Import `crate::selection` in `pane.rs`.

- [ ] **Step 6: Add the menu entry and enable rule**

In `popup_menu`, read the selection once:

```rust
let sel = self.panel.upgrade().map(|p| p.selected()).unwrap_or_default();
let n = sel.paths.len();
```

Add:

```rust
enable("goto-selected-folder", selection::folder_target(&sel).is_some());
```

In the `follow` menu section, after `Go to Tab Folder`:

```rust
follow.append_item(&item(&tr("Go to Selected Folder"), "goto-selected-folder"));
```

Add `"goto-selected-folder"` to the `connect_closed` re-enable list.

- [ ] **Step 7: Verify**

Run: `cargo test --all-targets`
Expected: all tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/config.rs src/selection.rs src/pane.rs
git commit -m "feat: go to the selected folder"
```

---

### Task 8: `New Terminal at Tab Folder` and `Duplicate Terminal`

**Files:**
- Modify: `src/config.rs:7-21` (`ACTIONS`)
- Modify: `src/pane.rs:128-184` (actions), `src/pane.rs:371-439` (menu), `src/pane.rs` (helper and tests)

**Interfaces:**
- Consumes: `Panel::split_at`, `Panel::current_dir`, `Pane::shell_dir`.
- Produces:
  - `fn auto_orientation(w: i32, h: i32) -> gtk::Orientation`
  - Pane actions `new-at-tab-dir` and `duplicate`
  - Config entries `("new-at-tab-dir", N_!("New Terminal at Tab Folder"))` and `("duplicate", N_!("Duplicate Terminal"))`

- [ ] **Step 1: Write the failing orientation test**

At the end of `src/pane.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_orientation_picks_the_longer_side() {
        assert_eq!(auto_orientation(100, 50), gtk::Orientation::Horizontal);
        assert_eq!(auto_orientation(50, 100), gtk::Orientation::Vertical);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib pane::tests::auto_orientation_picks_the_longer_side`
Expected: FAIL with `cannot find function auto_orientation`.

- [ ] **Step 3: Implement the helper and actions**

```rust
fn auto_orientation(w: i32, h: i32) -> gtk::Orientation {
    if w >= h { gtk::Orientation::Horizontal } else { gtk::Orientation::Vertical }
}
```

Use it in the existing `split-auto` action instead of the inline comparison.

Add to `config::ACTIONS`:

```rust
("new-at-tab-dir", N_!("New Terminal at Tab Folder")),
("duplicate", N_!("Duplicate Terminal")),
```

Add pane actions:

```rust
add("new-at-tab-dir", |p| p.new_at_tab_dir());
add("duplicate", |p| p.duplicate());
```

Implement:

```rust
fn new_at_tab_dir(self: &Rc<Self>) {
    let Some(panel) = self.panel.upgrade() else { return };
    let dir = panel.current_dir().filter(|d| d.is_dir())
        .or_else(|| self.shell_dir().filter(|d| d.is_dir()))
        .unwrap_or_else(glib::home_dir);
    let orientation = auto_orientation(self.widget.width(), self.widget.height());
    panel.split_at(self, orientation, Some(dir));
}

fn duplicate(self: &Rc<Self>) {
    let Some(panel) = self.panel.upgrade() else { return };
    let dir = self.shell_dir().filter(|d| d.is_dir())
        .or_else(|| panel.current_dir().filter(|d| d.is_dir()))
        .unwrap_or_else(glib::home_dir);
    let orientation = auto_orientation(self.widget.width(), self.widget.height());
    panel.split_at(self, orientation, Some(dir));
}
```

Both pass `Some(dir)`, so the new pane consumes the one-shot follow skip.

- [ ] **Step 4: Add the menu entries**

In the `layout` section, before `Split Right`:

```rust
layout.append_item(&item(&tr("New Terminal at Tab Folder"), "new-at-tab-dir"));
layout.append_item(&item(&tr("Duplicate Terminal"), "duplicate"));
```

Both actions are always enabled in the terminal context menu; they fall back to the shell directory and home.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --lib pane::tests::auto_orientation_picks_the_longer_side`
Expected: PASS.

Run: `cargo test --all-targets`
Expected: all tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/pane.rs
git commit -m "feat: add terminal creation actions"
```

---

### Task 9: Translation catalog

**Files:**
- Modify: `po/shellman.pot`
- Modify: `po/fr.po`

**Interfaces:**
- Consumes: every `tr()`/`ntr()` string from Tasks 1-8.
- Produces: a complete French catalog with no untranslated or fuzzy entries.

- [ ] **Step 1: Regenerate the catalog**

Run: `po/update.sh`
Expected: `po/shellman.pot` and `po/fr.po` are updated; new msgids appear with empty `msgstr`.

- [ ] **Step 2: Add the French translations**

Fill these entries in `po/fr.po` (keep the existing header and existing translations):

| msgid | msgstr |
| --- | --- |
| `Open Terminal Here` | `Ouvrir un terminal ici` |
| `Open a new terminal in this folder` | `Ouvrir un nouveau terminal dans ce dossier` |
| `Insert into Terminal` | `Insérer dans le terminal` |
| `Insert the selected paths into the terminal` | `Insérer les chemins sélectionnés dans le terminal` |
| `Copy Path` | `Copier le chemin` |
| `Copy the selected paths` | `Copier les chemins sélectionnés` |
| `Copy as URI` | `Copier comme URI` |
| `Copy the selected paths as file:// URIs` | `Copier les chemins sélectionnés comme URI file://` |
| `No files selected` | `Aucun fichier sélectionné` |
| `{n} file selected` / `{n} files selected` | `msgstr[0]`: `{n} fichier sélectionné`; `msgstr[1]`: `{n} fichiers sélectionnés` |
| `{shortcut} to insert` | `{shortcut} pour insérer` |
| `{n} remote item skipped` / `{n} remote items skipped` | `msgstr[0]`: `{n} élément distant ignoré`; `msgstr[1]`: `{n} éléments distants ignorés` |
| `{n} missing item skipped` / `{n} missing items skipped` | `msgstr[0]`: `{n} élément manquant ignoré`; `msgstr[1]`: `{n} éléments manquants ignorés` |
| `Nothing inserted: no local file in the drop` | `Rien n'a été inséré : aucun fichier local` |
| `Select exactly one folder` | `Sélectionnez exactement un dossier` |
| `Folder no longer exists` | `Le dossier n'existe plus` |
| `Cannot insert: a file name cannot be written safely for this shell` | `Insertion impossible : un nom de fichier ne peut pas être écrit sans risque pour ce shell` |
| `Go to Selected Folder` | `Aller au dossier sélectionné` |
| `New Terminal at Tab Folder` | `Nouveau terminal dans le dossier de l'onglet` |
| `Duplicate Terminal` | `Dupliquer le terminal` |

Entries that come from `ntr()` are plural entries in the PO file. They have one
`msgid` plus `msgid_plural`, and French needs both plural forms:

```po
msgid "{n} file selected"
msgid_plural "{n} files selected"
msgstr[0] "{n} fichier sélectionné"
msgstr[1] "{n} fichiers sélectionnés"
```

`msgstr[0]` is the singular form and `msgstr[1]` is the plural form for French
(`nplurals=2; plural=(n > 1);`). Fill both, keep `{n}` exactly as written, and
do not split a plural pair into two independent entries. The same applies to
the remote-skipped and missing-item pairs in the table.

If `po/update.sh` produces additional new msgids from Tasks 1-8, translate them in the same style; do not leave an empty or fuzzy entry.

- [ ] **Step 3: Verify the catalog**

Run: `msgfmt --check -o /dev/null po/fr.po`
Expected: PASS.

Run: `msgattrib --untranslated po/fr.po | grep '^msgid'`
Expected: no output.

Run: `msgattrib --only-fuzzy po/fr.po | grep '^msgid'`
Expected: no output.

- [ ] **Step 4: Run the test suite with the new catalog**

Run: `cargo test --all-targets`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add po/shellman.pot po/fr.po
git commit -m "i18n: translate the file-aware actions"
```

---

### Task 10: Full verification and pull request

**Files:**
- No source changes expected. If verification finds a defect, fix it in the owning task's files and commit separately.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: a green branch and a PR against the fork's `main`.

- [ ] **Step 1: Run the full local gate**

Run each command and require success:

```bash
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
make test
git diff --check
```

Expected: no warnings, all tests pass, release build succeeds, no whitespace errors.

- [ ] **Step 2: Run the manual matrix**

On Debian 13 with Nautilus 48.3, run spec §8 cases 1-16:

1. `Open Terminal Here` with panel hidden and no panes.
2. `Open Terminal Here` with panel hidden but an existing pane; old pane untouched, new pane at the folder.
3. `Open Terminal Here` with a command running; command not interrupted.
4. `Open Terminal Here` on a subfolder with follow enabled; pane stays after its first prompt.
5. `Insert into Terminal` on one file; quoted, not executed.
6. Multiple hostile names (`;`, quotes, newline, `$(...)`, bidi); safe.
7. Remote plus local selection; local inserted, skipped count shown.
8. Drag one file, many files, and a folder.
9. Drag a remote-only item; nothing inserted, feedback shown.
10. `Copy Path` and `Copy as URI` paste correctly.
11. Status line updates after selecting files and refocusing the terminal.
12. `Go to Selected Folder` with one folder, one folder plus one file, a file only, and two folders.
13. `New Terminal at Tab Folder` while the shell is elsewhere.
14. `Duplicate Terminal` preserves the shell directory with follow enabled.
15. Existing F4/follow/`$SEL`/split/close/colors/preferences/shell-exit behavior.
16. Two Nautilus windows: actions target the active window; warning logged if no panel.

Record the exact distribution and `nautilus --version` output in the PR. Mark any case that could not be run as unverified; do not claim broader Nautilus coverage.

- [ ] **Step 3: Push the branch**

```bash
git push origin feat/file-aware-terminal
```

- [ ] **Step 4: Open the PR against the fork's main**

```bash
gh pr create --repo Himath-Rajapaksha/shellman \
  --base main --head feat/file-aware-terminal \
  --title "v0.2: file-aware terminal" \
  --body "## Summary
- file/folder context menu: Open Terminal Here, Insert into Terminal, Copy Path, Copy as URI
- drag-and-drop insertion with safe quoting
- selection status line and Go to Selected Folder
- New Terminal at Tab Folder and Duplicate Terminal
- CI (clippy, tests, release build) and GPL fork attribution

## Testing
- cargo clippy --all-targets -- -D warnings
- cargo test --all-targets
- cargo build --release --locked
- manual matrix on Debian 13 / Nautilus 48.3 (record results below)

## Release
Source-only v0.2. No binary packages, no app ID or package-name changes.

## Manual results
<paste the matrix results here>"
```

- [ ] **Step 5: Confirm CI**

Run: `gh pr checks --repo Himath-Rajapaksha/shellman --watch`
Expected: the `CI / check` job passes.

Do not tag a release or publish packages. The first binary release waits for a distinct fork name and app ID.
