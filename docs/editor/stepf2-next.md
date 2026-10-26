# StepF2: inspector and normalization integration

The typed inspector, conversion consistency helpers and explicit structural
normalization are implemented. Initial normalization and native tests passed;
additional boundary tests are pending. Executed evidence is in `stepf2.md`.

## Native inspector

Keep the existing five-row inspector for ordinary charts. In a decoded StepF2
chart, show eight rows: lane, endpoint, visibility, fake flag, reserved character,
raw suffix, apply, cancel. F2 switches between this typed view and the existing
raw view without recreating the `note_attributes::Draft` or applying it.

- Left/right on lane retains the existing pending-edit protection.
- Left/right on endpoint selects head/tail; tail is unavailable for taps.
- Left/right on visibility cycles Normal/Sudden/Vanish/Hidden.
- Left/right on fake toggles the selected endpoint flag. A fake hold remains
  `Kind::Hold`; the head flag changes Tap/Fake classification only for taps.
- Reserved accepts exactly one character through the existing text buffer;
  `Attributes::suffix` remains authoritative for allowed characters.
- Raw suffix editing uses the same endpoint as the typed controls, including
  trailing keysound/extension data. No intermediate malformed text is committed.
- Navigation, endpoint change, and F2 first synchronize pending text. Invalid
  typed parsing reports an error and preserves the draft and raw edit buffer.
- Escape/cancel discards the entire gesture. Apply produces one existing
  Session history entry and preserves its stale-token protection.

Use a small explicit inspector view state rather than duplicating the draft.
Text-field target mapping, panel row labels, focus handling, and serialization
must all derive their indices from that view state. Localize labels/help/errors
in English and French. Do not silently substitute defaults for malformed raw
compound fields when returning to typed controls.

GrooveAuthor's `docs/MultiplayerCharts.md` says it does not retain Sudden, Hidden,
Vanish, or Heart attributes. Rust's retained visibility/reserved flags therefore
remain a preservation extension, not proof of GrooveAuthor UI parity. The
reserved character must not be labelled Heart without a verified semantics
contract. Tests of flags prove serialization/editing, not gameplay visibility.

## Implemented adjacent consistency work

`transform.rs` now uses `stepf2::change_kind` to synchronize explicit Tap/Fake
conversions and retain visibility, reserved and trailing suffix data. An explicit
conversion to Tap clears the fake bit. Conversion from Fake to Hold retains it.
Unsupported StepF2 target kinds are rejected before the transaction is executed.

`Grid::force_insert` uses the same helper through `shorten_to_tap`: a shortened
fake hold becomes Fake rather than an inconsistent Tap, while ordinary holds
become Tap. Sustain-only tail attributes are removed when the sustain disappears.

`Session::normalize_stepf2_chart` and `Command::NormalizeStepF2` implement an
explicit reversible Pump Double co-op to Pump Routine structural gesture. The
conditional chart-properties action protects unapplied property drafts. It
updates the live descriptor and participant count, retains chart identities and
other unsaved edits, and supports structural undo/redo. Opening a chart remains
byte-preserving. Fumen instead promotes the layout automatically during parsing;
this explicit persistence action preserves DeadSync's existing read-only contract.

Resident Pump Routine defaults to two players, including ordinary notation.
Explicit GrooveAuthor participant metadata can select one when note ownership
allows it. Creating unrelated metadata retains the current layout default.
The low-level `Grid::parse`/`parse_layout` standard-stream contract still describes
serialized participant segments; Session supplies the Routine resident default.

## Focused validation status

1. Core typed/raw round-trips for each visibility and fake state on head/tail,
   with reserved and trailing suffix preservation.
2. Tap/Fake conversions and sustain shortening retain coherent fake semantics;
   save/reopen and undo/redo restore exact prior states.
3. Native EN/FR workflows: all rows accessible without mouse, typed/raw F2,
   endpoint changes, malformed suffix recovery, pending lane protection,
   cancellation, no-op apply, stale draft refusal, and one-gesture undo/redo.
4. Ordinary chart inspector stays five rows with its existing raw behavior.
5. Pump Double normalization proves descriptor/layout/player consistency and
   reversible save/reopen separately; it is not inferred from a typed UI test.

Run Cargo centrally, with `--locked --jobs 2`, after coordinating source ownership
with the integrating agent. No native hardware/render/audio claim follows from
headless input workflows.

The integrating agent confirmed initial normalization (5), differential (2),
participant count (4), chart properties (7), metadata (11), conversion (3) and
StepF2 (16) targets passed. The native EN/FR targets passed seven tests each
(14 total), and existing `edition_workflow` passed 37 tests. Four new boundary
workflows await centralized execution: nonleading normalization and undo after
save, duplicate SSC layout fields, pending P4 notes plus unrelated metadata,
and metadata-only preservation of unsupported notes. The normalization target
now has nine tests; its complete updated version has not yet been rerun.
