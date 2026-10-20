import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('storefront_evidence', Path(__file__).with_name('storefront_evidence.py'))
evidence = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evidence)


class StorefrontEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.output = Path(self.temporary.name)
        self.snapshot = {'sha256': 'source-a', 'commit_sha': 'commit-a'}
        self.source = patch.object(evidence, 'source_snapshot', return_value=self.snapshot)
        self.source.start()
        self.environment = patch.dict(evidence.os.environ, {}, clear=True)
        self.environment.start()
        (self.output / 'golden-path.log').write_text('fixture log\n')

    def tearDown(self):
        self.environment.stop()
        self.source.stop()
        self.temporary.cleanup()

    def stage(self, level='scaffold', native='registry'):
        evidence.begin(self.output, 'test-run')
        report = evidence.load_owned(self.output, 'test-run')
        report.update(level=level, native_build=native)
        evidence.save(self.output, report)

    def test_begin_invalidates_previous_success_even_when_source_inspection_fails(self):
        evidence.save(self.output, {'result': 'passed'})
        with patch.object(evidence, 'source_snapshot', side_effect=RuntimeError('Git unavailable')):
            with self.assertRaises(RuntimeError):
                evidence.begin(self.output, 'new-run')
        self.assertEqual(evidence.load_owned(self.output, 'new-run')['result'], 'running')
        evidence.fail(self.output, 'new-run', 1)
        self.assertEqual(evidence.load_owned(self.output, 'new-run')['result'], 'failed')

    def test_source_drift_cannot_be_published_as_success(self):
        self.stage()
        with patch.object(evidence, 'source_snapshot', return_value={'sha256': 'changed', 'commit_sha': 'commit-a'}):
            with self.assertRaisesRegex(RuntimeError, 'source changed'):
                evidence.finish(self.output, 'test-run')
        report = evidence.load_owned(self.output, 'test-run')
        self.assertFalse(report['source_unchanged'])
        self.assertNotEqual(report['result'], 'passed')

    def test_scaffold_success_is_source_bound_and_log_hashed(self):
        self.stage()
        evidence.finish(self.output, 'test-run')
        report = evidence.load_owned(self.output, 'test-run')
        self.assertEqual(report['result'], 'passed')
        self.assertTrue(report['source_unchanged'])
        self.assertEqual(len(report['log_sha256']), 64)

    def test_late_controller_cannot_overwrite_another_invocation(self):
        self.stage()
        with self.assertRaisesRegex(RuntimeError, 'another invocation'):
            evidence.fail(self.output, 'old-run', 143)
        self.assertEqual(evidence.load_owned(self.output, 'test-run')['result'], 'running')

    def test_full_gate_requires_configuration_and_native_journey_proofs(self):
        self.stage('install-seed-doctor-journey-typecheck-build')
        with self.assertRaisesRegex(RuntimeError, 'configuration proof'):
            evidence.finish(self.output, 'test-run')
        log = self.output / 'golden-path.log'
        log.write_text('storefront-configuration-proof mode=launch result=passed\n')
        with self.assertRaisesRegex(RuntimeError, 'journey proof'):
            evidence.finish(self.output, 'test-run')
        journey = {'result': 'passed', 'settlement': 'simulated-no-funds-transferred', 'externalProvidersVerified': False}
        log.write_text(log.read_text() + json.dumps(journey) + '\n')
        evidence.finish(self.output, 'test-run')
        self.assertEqual(evidence.load_owned(self.output, 'test-run')['merchant_journey'], journey)

    def test_ci_rejects_reused_native_binary(self):
        self.stage('install-seed-doctor-journey-typecheck-build', native='reused')
        with patch.dict(evidence.os.environ, {'GITHUB_ACTIONS': 'true'}):
            with self.assertRaisesRegex(RuntimeError, 'reused binary'):
                evidence.finish(self.output, 'test-run')

    def test_failures_and_requested_commit_mismatches_are_not_success(self):
        with patch.dict(evidence.os.environ, {'GITHUB_SHA': 'wrong-commit'}):
            with self.assertRaisesRegex(RuntimeError, 'GITHUB_SHA'):
                evidence.begin(self.output, 'test-run')
        evidence.fail(self.output, 'test-run', 143)
        report = evidence.load_owned(self.output, 'test-run')
        self.assertEqual(report['result'], 'failed')
        self.assertEqual(report['exit_code'], 143)
        self.assertEqual(len(report['log_sha256']), 64)


if __name__ == '__main__':
    unittest.main()
