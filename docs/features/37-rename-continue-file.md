# Rename file from continue card

---
status: done
priority: p1
layers: [ui, storage, persistence]
related: [21, 27, 33, 34, 38]
---

## Use cases
- Correct a wrong or cryptic filename without leaving the continue strip or opening a file manager.

## Description
On hover, each continue (or search / Lucky) card that points at an existing local file shows a **Rename file** control to the left of Move to Trash and Remove. Choosing it opens a dialog whose field holds the current name without the extension, already selected so typing replaces it at once. Confirming renames the file in its folder, keeps that extension, updates the persistent store and catalog, and refreshes the strip so the card title matches. Cancel leaves the file and store untouched.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:recent
Feature: Rename file from continue card

  Scenario: Rename control sits left of trash and remove
    Given a continue card references an existing local file
    When the pointer enters that card
    Then a Rename file control is visible to the left of Move to Trash and Remove

  Scenario: Dialog shows the name without extension already selected
    Given a continue card references an existing local file
    When the user activates Rename file
    Then a rename dialog appears
    And the text field shows the current name without the file extension
    And that text is selected so typing replaces it

  Scenario: Confirm renames on disk and refreshes the card
    Given the rename dialog is open for a local file
    When the user enters a new name without path separators and confirms
    Then the file is renamed in its folder with the original extension kept
    And the persistent store and catalog use the new path
    And the continue strip shows the card under the new human-readable title
    And resume progress and thumbnail for that title are kept

  Scenario: Cancel leaves the file unchanged
    Given the rename dialog is open
    When the user cancels
    Then the file path on disk is unchanged
    And the continue strip is unchanged

  Scenario: Empty or invalid name does not rename
    Given the rename dialog is open
    When the user confirms an empty name or a name that contains a path separator
    Then the file is not renamed
    And the dialog stays open with an error the user can correct

  Scenario: Chosen name already taken does not overwrite
    Given the rename dialog is open
    And another file in that folder already uses the chosen name with the same extension
    When the user confirms
    Then the original file is not renamed
    And the other file is unchanged
    And the dialog stays open with an error the user can correct

  Scenario: Library update failure restores the file name
    Given the rename dialog is open
    And the on-disk rename would succeed
    And updating the persistent store would fail
    When the user confirms
    Then the file keeps its original name on disk
    And the dialog stays open with an error the user can correct

  Scenario: Missing file has no rename control
    Given a continue card is marked missing on disk
    When the pointer enters that card
    Then no Rename file control appears
```

## Notes
- Control: `document-edit-symbolic` hover button in `fill_history_card/card_actions.rs`, left of Trash / Remove; same `rp-recent-action` chrome as those controls. Bundled under `data/icons/hicolor/scalable/actions/` so macOS Homebrew GTK (empty Adwaita action set) still resolves it.
- Dialog: `adw::AlertDialog` with an `gtk::Entry` `extra_child`; editable part from `Path::file_stem`, extension kept via `Path::extension`. Entry and dialog widths come from a Pango measure of the stem so long / non-Latin names stay visible.
- Owner: `recent_view/card_rename_apply.rs` (+ `card_rename.rs` dialog) under neighbour-search state — disk rename, store update, strip retarget. Failed attempts keep the dialog open with an inline error under the entry; Adw’s response close is undone with `present`.
- Store: `db::rekey_renamed_path` → `Result` runs `files` + optional `history` / `media` in one `BEGIN IMMEDIATE` transaction. On store failure the rename flow restores the original path on disk when possible.
- Strip refresh: retarget search hits / catalog index, `record_history` for the new path, then `apply_strip`. An active Lucky session is closed so the continue strip can show the rekeyed file.
- Always-on `[rhino] rename:` lines on failure (plain `cargo run`); the dialog also shows the user-facing message.
