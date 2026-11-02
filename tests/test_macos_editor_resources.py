"""Editor resource packaging checks without Cargo or native macOS tools."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/build-macos-app.py"
SPEC = importlib.util.spec_from_file_location("build_macos_app", SCRIPT)
BUNDLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUNDLE)


class EditorResourcesTest(unittest.TestCase):
    def fixture(self, root):
        paths = list(BUNDLE.EDITOR_ASSETS)
        paths.extend("docs/editor/licenses/" + name for name in BUNDLE.EDITOR_NOTICES)
        for relative in paths:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(relative.encode())

    def test_untracked_source_assets_and_notices_are_copied_exactly(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            resources = Path(temporary) / "Resources"
            self.fixture(root)
            with patch.object(BUNDLE, "ROOT", root):
                BUNDLE.copy_editor_resources(resources)
            for relative in BUNDLE.EDITOR_ASSETS:
                self.assertEqual((resources / relative).read_bytes(), relative.encode())
            for name in BUNDLE.EDITOR_NOTICES:
                self.assertEqual(
                    (resources / "licenses/editor" / name).read_bytes(),
                    ("docs/editor/licenses/" + name).encode(),
                )

    def test_missing_required_resource_fails_before_copying(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            resources = Path(temporary) / "Resources"
            self.fixture(root)
            (root / BUNDLE.EDITOR_ASSETS[-1]).unlink()
            with patch.object(BUNDLE, "ROOT", root):
                with self.assertRaisesRegex(RuntimeError, "Missing editor resource"):
                    BUNDLE.copy_editor_resources(resources)
            self.assertFalse(resources.exists())

    def test_unlisted_local_songs_and_user_preferences_are_not_copied(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            resources = Path(temporary) / "Resources"
            self.fixture(root)
            for relative in ("songs/private/song.ssc", "editor-preferences.json"):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("local user data")
            with patch.object(BUNDLE, "ROOT", root):
                BUNDLE.copy_editor_resources(resources)
            self.assertFalse((resources / "songs").exists())
            self.assertFalse((resources / "editor-preferences.json").exists())


if __name__ == "__main__":
    unittest.main()
