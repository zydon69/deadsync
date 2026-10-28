# Confirmed remaining work — 28 October 2026

This is a live list of confirmed gaps, not an exhaustive coverage certificate.
Reference: GrooveAuthor 48e18f71655d0f0d86144c63239331833501fda2.

## Integration and validation in progress

- OutputPolicy durable Save/SaveAs/recovery: prepared patches; publication epoch and
  exact native recovery payload framing added during review; Rust validation pending.
- Source keyboard profile/import plus unavailable longer-chord protection: prepared
  patches, not yet applied. Existing 80 native actions / 35 source IDs are distinct
  from the catalog of 164 source callbacks.
- Quick Add and Shift All row review integrated; core and EN/FR tests pending.
- Selected/All pattern regeneration with new seeds and Clear Selected/All:
  source Editor.cs 7413–7467; native engines exist but exact scope/reseed dispatcher
  and keyboard review remain in preparation. Reseed is a separate source undo action.
- Typed source shortcut callbacks for note operations and Quick Add: in preparation;
  opening a generic panel does not implement a parameterized source command.

## Density graph configuration — confirmed by source/UI comparison

Source UI/UIDensityGraphPreferences.cs 44–99 exposes visibility, stream overlay,
Step versus Row accumulation, position and offsets, width offset, height, color
mode, and high/low/background colors. Native analysis_panel.rs displays RSSP
stream summaries and a density plot, but no DensityGraph/AccumulationType/
ShowStream preference contract was found in the editor core or native edition
modules. Existing RSSP analysis must remain available; it does not by itself
satisfy the original configurable density graph contract. This requires a model,
source defaults/algorithm comparison, persisted keyboard UI and workflow tests.

## File/export follow-up confirmed by source review

- Successful Save/SaveAs must update recent songs; first Untitled SaveAs must
  become Reload's target. Current prepared UI needs this follow-up.
- Advanced Save options have global source preferences already modeled by
  audio_preferences::File. UI still needs to read/persist them rather than use
  default options for every NativeLossless file.
- Source Open Containing Folder, external-change Ignore/Reload watcher/modal,
  and startup last-file/directory preferences are not yet connected natively.
- Source chartless advanced export accepts a null fallback chart, while the
  native policy model currently requires a timing chart identity.
- Source song Save also saves pack metadata; native Pack currently has a
  separate guard/save workflow. This difference needs explicit resolution.

## Delivery validation

Run wider editor regressions after integrations. The full theme suite has 14
recorded failures at the previous checkpoint; no baseline classification is claimed.
Physical keyboard, GPU rendering and audible playback still require runtime review
and user testing. No editor pull request before user validation.
