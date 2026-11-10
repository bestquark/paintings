import contextlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from paintings import adapters


class ItermProfileTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.home = Path(self.scratch.name)
        self.directory = self.home / 'Library/Application Support/iTerm2/DynamicProfiles'
        self.directory.mkdir(parents=True)
        self.profile = self.directory / 'Paintings.json'
        self.other = self.directory / 'Other.json'
        self.other.write_text('{"Profiles": []}')
        self.state = self.home / 'state'
        self.state.mkdir()
        (self.state / 'iterm-default-before.json').write_text('{}')
        for mocked in (
            patch.object(Path, 'home', return_value=self.home),
            patch.object(adapters.platform, 'system', return_value='Darwin'),
            patch.object(adapters, 'STATE', self.state),
            patch.object(adapters, 'backup'),
            patch.object(adapters, 'run', return_value=''),
        ):
            mocked.start()
            self.addCleanup(mocked.stop)

    def check_update(self, existing):
        if existing:
            self.profile.write_text('{"Profiles": [{"Name": "Previous"}]}')
        old = self.profile.read_bytes() if existing else None
        original_open = Path.open
        original_replace = Path.replace
        observations = []

        def inspect():
            files = set(self.directory.iterdir())
            self.assertEqual(files, {self.other, self.profile} if existing else {self.other})
            for item in files:
                json.loads(item.read_bytes())
            if existing:
                self.assertEqual(self.profile.read_bytes(), old)
            observations.append(files)

        @contextlib.contextmanager
        def observed_open(path, mode='r', *args, **kwargs):
            with original_open(path, mode, *args, **kwargs) as handle:
                if any(flag in mode for flag in ('w', 'a', 'x')):
                    # Observe the folder after creation but before any bytes are written.
                    inspect()
                yield handle

        def observed_replace(path, target):
            if Path(target) == self.profile:
                inspect()
                payload = json.loads(path.read_bytes())
                self.assertEqual(payload['Profiles'][0]['Background Image Location'], '/cached/painting.jpg')
            return original_replace(path, target)

        with patch.object(Path, 'open', observed_open), patch.object(Path, 'replace', observed_replace):
            adapters.iterm(Path('/cached/painting.jpg'), {'fit': 'cover', 'terminal_opacity': .16})
        self.assertTrue(observations)
        profile = json.loads(self.profile.read_text())['Profiles'][0]
        self.assertEqual(profile['Name'], 'Paintings')
        self.assertEqual(profile['Background Image Mode'], 2)
        self.assertAlmostEqual(profile['Blend'], .84)
        self.assertEqual(self.other.read_text(), '{"Profiles": []}')
        self.assertEqual(set(self.directory.iterdir()), {self.other, self.profile})
        self.assertEqual(set(self.directory.parent.iterdir()), {self.directory})

    def test_updates_never_expose_partial_files(self):
        self.check_update(existing=True)

    def test_initial_profile_never_exposes_partial_files(self):
        self.check_update(existing=False)

    def test_failed_replace_preserves_profile_and_cleans_staging(self):
        old = '{"Profiles": [{"Name": "Previous"}]}'
        self.profile.write_text(old)
        original_replace = Path.replace

        def fail_replace(path, target):
            if Path(target) == self.profile:
                raise OSError('simulated replacement failure')
            return original_replace(path, target)

        with patch.object(Path, 'replace', fail_replace):
            with self.assertRaisesRegex(OSError, 'simulated replacement failure'):
                adapters.iterm(Path('/cached/painting.jpg'), {'fit': 'cover'})
        self.assertEqual(self.profile.read_text(), old)
        self.assertEqual(set(self.directory.iterdir()), {self.other, self.profile})
        self.assertEqual(set(self.directory.parent.iterdir()), {self.directory})


if __name__ == '__main__':
    unittest.main()
