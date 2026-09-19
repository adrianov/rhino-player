# Continue strip scroll pan

---
status: done
priority: p1
layers: [ui, input]
related: [23, 33]
scope: portable
---

## Use cases
- Pan the continue strip from anywhere on the browse screen, without needing the pointer over the cards.

## Description
On the browse (continue) screen, scrolling — a two-finger touchpad gesture or a mouse wheel — glides the horizontal continue card strip along exactly as it does over the cards themselves. It works above the cards, below them, and over the transport row; the cards' own interactions (hover toolbar, buttons) are unaffected, and once the strip reaches either end the scroll is left unconsumed.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:recent
Feature: Continue strip scroll pan

  Background:
    Given the browse screen shows the continue card strip with more cards than fit

  Scenario: Scrolling an empty area pans the strip
    Given the pointer is over an empty part of the browse screen, away from the card strip
    When the user scrolls with a two-finger touchpad gesture or a mouse wheel notch
    Then the strip glides the same distance, in the same direction, as it does over the cards

  Scenario: Direction and magnitude match on every surface
    Given the pointer is anywhere on the browse screen
    When the user scrolls vertically or horizontally
    Then a downward or rightward scroll carries the strip toward later cards
    And an upward or leftward scroll brings it back toward earlier cards
    And a wheel notch advances the strip a fixed step, while a touchpad gesture tracks the finger's travel

  Scenario: Cards keep their own interactions
    Given the pointer is over a continue card
    When the user scrolls or clicks
    Then the scroll drives the strip through the same pan path as the rest of the screen, with no double-stepping
    And card clicks and hover toolbars work exactly as before

  Scenario: A strip that cannot move does not consume the scroll
    Given the strip rests at either end of its travel, or no strip is visible
    When the user scrolls
    Then the strip does not move
    And the scroll passes through instead of being swallowed
```

## Notes
- Implemented in `src/recent_view/strip_stack.rs`: `wire_strip_scroll_everywhere` attaches a `gtk::EventControllerScroll` (`BOTH_AXES | CAPTURE`) to the whole browse band (`recent_scrl`) in capture phase and drives the strip's own h-adjustment directly — a single pan path, so travel over the cards matches the surrounding band by construction; the strip's `ScrolledWindow` only receives what the pan refuses (hidden strip, edge rest, collapsed range — states in which it cannot move either). `strip_scroll_delta` sums `dx + dy` onto the one horizontal axis: the strip scrolls horizontally only, so vertical deltas must map onto it.
- `strip_scroll_delta` classifies by the event's `GdkScrollUnit` (GTK 4.8+): a `Wheel` notch steps a fixed stride (`STRIP_WHEEL_STEP`, `src/recent_view/strip_stack.rs`), a `Surface` event (touchpad gesture) tracks finger travel 1:1; `pan_strip` clamps the target to `[lower, upper-page_size]` and reports no movement — the event keeps bubbling — when the strip is hidden, already at an edge, the delta is zero, or the adjustment range is collapsed (`upper - page_size <= lower`; `f64::clamp` would panic). Never classify by delta fractionality: smooth deltas can arrive integral, and `DISCRETE` quantizes accumulated smooth deltas to integers (it also makes touchpad pans feel chunky).
- The controller checks the strip's visibility, so a hidden band never intercepts events during playback.
