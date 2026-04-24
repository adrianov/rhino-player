# Video options: aspect, crop, zoom, filters

**Name:** Per-video options (options menu)

**Implementation status:** Not started

**Use cases:** Fix letterboxing, wrong aspect, or color; align subtitles and audio; slow down or speed up temporarily—without re-encoding the file.

**Short description:** A popover (or Adwaita menu) to adjust `video-aspect-override`, crop, zoom, contrast, brightness, gamma, saturation, hue, `sub-delay`, `audio-delay`, and `speed`, with reset. Optional flip/rotate when not conflicting with hardware decode copy path.

**Long description:** Expose presets for aspect and crop, controls for numeric properties, and resets. Flip controls may hide when hardware decode is on with a missing `-copy` path (vendor-specific). All values map to mpv properties or commands.

**Specification:**

- On menu open, read current property values and sync widgets.
- On change, set mpv property or command asynchronously.
- “Reset all” returns documented defaults.
- Rotation/crop/flip: align with mpv `vf` usage documented in the implementation.
