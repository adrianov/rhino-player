# Reveal file from continue card

---
status: done
priority: p1
layers: [ui, os-integration]
related: [21, 27, 37]
---

## Use cases
- Open the file’s folder in the system file manager with that item already selected, without copying the path by hand.

## Description
On hover, each continue (or search / Lucky) card that points at an existing local file shows a **Reveal in folder** control between Rename file and Move to Trash. Choosing it brings the system file manager forward on that file’s folder and selects the file. Cards marked missing on disk omit the control.

## Behavior

```gherkin
@status:done @priority:p1 @layer:os-integration @area:recent
Feature: Reveal file from continue card

  Scenario: Reveal control sits between rename and trash
    Given a continue card references an existing local file
    When the pointer enters that card
    Then a Reveal in folder control is visible between Rename file and Move to Trash

  Scenario: Reveal selects the file in its folder
    Given a continue card references an existing local file
    When the user activates Reveal in folder
    Then the system file manager shows that file’s folder
    And that file is selected

  Scenario: Missing file has no reveal control
    Given a continue card is marked missing on disk
    When the pointer enters that card
    Then no Reveal in folder control appears
```

## Notes
- Control: hover button in `fill_history_card/card_actions.rs` between Rename and Trash; icon `folder-symbolic` (bundled under `data/icons/hicolor/scalable/actions/`); tooltip **Reveal in Finder** (macOS) / **Show in Files** (Linux).
- Owner: `reveal_file.rs` — macOS `NSWorkspace::activateFileViewerSelectingURLs:`; Linux async session-bus `org.freedesktop.FileManager1.ShowItems` (GNOME Files and other FileManager1 clients; does not block the GTK loop). Always-on `[rhino] reveal:` on failure.
