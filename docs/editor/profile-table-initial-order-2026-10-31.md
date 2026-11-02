# Initial profile table ordering

The exact cimgui library at GrooveAuthor48e18f's ImGui.NET gitlink f2812cc9e4955e0496611bde2bcdd34225442b5b was executed headlessly by the root agent. Fresh contexts, source column IDs/flags/labels/zero widths and original ImGuiUtils header sequence produced these specs in both frames0/1:

- Pattern: NoteType, user_id0, ascending.
- Performed: StepSpeedMin, user_id0, ascending.
- First frame dirty=true; second frame dirty=false after source-equivalent acknowledgement.

The observed output and source/native hashes are `tests/fixtures/profile-comparers/first-table-sort.json` in deadsync-editor. The replay script is `scripts/editor_profile_table_initial_reference.py`. It executes the actual pinned vendor library; it does not invoke the profile comparator. Existing `profile-comparer-source-oracle-2026-10-29.md` and1512pair/42sort/2multispec fixtures independently cover the original comparators.

Native initial order previously remained host cultural order until F9. `profile_sort::Runtime::effective_specs` now supplies the observed spec for a family with no explicit user entry. `process_profile_order` first installs real host labels and then applies these typed source specs with the existing complete source comparer/GUID tie-break. Expressed has no sortable table and continues using host cultural ordering. No source comparator or core generation behavior was changed.

An explicit user spec vector—including the existing empty-vector opt-in host-order reset—is retained across reopen/culture changes. That host reset is a keyboard extension, not source SortTristate: the original tables lack SortTristate. It is distinct from the original implicit initial order. F9 starts from the implicit spec so the first key changes column, rather than silently reinstalling the column already in effect. Original source StepSpeedMin ep1-for-both-sides behavior remains intact.

Prepared native regressions compare the effective initial specs to the real vendor receipt; load real custom configs whose ASCII names invert their source NoteType/enable-column order; assert GUID ties and selected identities; retain explicit F9 choice across culture/reopen; keep Expressed cultural and opt-in host reset; assert no document/history mutation without any open song. Existing selector-sort regression was updated so its explicit host reset is compared to the host order, not the now-corrected initial source order.

No new Cargo/test run was performed by the sidecar author. These native regressions remain pending root execution. Headless vendor sorting proof does not establish GPU layout or all profile fields; the bounded source-column/consumer review is separate. No empty/debug source file was treated as missing product behavior.
