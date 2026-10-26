# Original Pack.ini oracle

Run `python3 scripts/editor-pack-reference.py --editor-root /path/to/grooveauthor --reference-root /path/to/StepManiaLibrary --dotnet /path/to/dotnet --output crates/deadsync-editor/tests/fixtures/grooveauthor/pack` from the repository root.

The generator checks the pinned clean StepManiaLibrary/Fumen trees, reads the pinned GrooveAuthor source blobs, and records Git blob IDs and SHA256 provenance. It invokes real Fumen `IniFile` and `ItgManiaPack` readers/writers and unchanged original editor creation, save and dirty methods. The adapter supplies an inert watcher, owner and preference context and an injected UTC year. Temporary files belong exclusively to the oracle and are removed on exit. This does not exercise filesystem watchers, native widgets or user pack storage.

The checked fixture contains 37 reader cases and six creation/default cases. It includes first-key/first-group behavior, exact case and value whitespace, malformed and future versions, arbitrary numeric enum values, BOM encodings, invalid UTF-8, null versus empty strings, clone independence and dirty/save state. The canonical writer is the original destructive writer. Rust's preservation of unknown bytes and original encoding is a deliberate extension, checked independently rather than attributed to this oracle.

SDK used for the checked fixture: official .NET 10.0.403. Source commit IDs, source blob hashes, extracted-member hashes, request/output hashes and assembly hashes are recorded in `pack.reference.json`.
