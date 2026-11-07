"""Cleanup fault regressions: no processes, database, providers or browser."""
import ast
import io
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import Mock

from queue_recovery import QueueRecovery


class QueueCleanup(unittest.TestCase):
    def test_first_worker_failure_still_attempts_second_and_closes_all_pipes(self):
        recovery = QueueRecovery(None, None, None, {})
        workers = [Mock(stderr=io.BytesIO()), Mock(stderr=io.BytesIO())]
        recovery.workers = workers
        recovery.kill = Mock(side_effect=[RuntimeError('private failure detail'), None])
        with self.assertRaisesRegex(RuntimeError, r'Worker cleanup failed \(1 errors: RuntimeError\)'):
            recovery.cleanup()
        self.assertEqual([call.args[0] for call in recovery.kill.call_args_list], workers)
        self.assertTrue(all(worker.stderr.closed for worker in workers))

    def test_teardown_failure_cannot_leave_pass_and_still_attempts_other_resources(self):
        # Execute the runner's real finally body without its service/bootstrap statements.
        source = ast.parse(Path(__file__).with_name('run-integration.py').read_text())
        teardown = next(node.finalbody for node in source.body if isinstance(node, ast.Try) and node.finalbody)
        code = compile(ast.Module(body=teardown, type_ignores=[]), 'runner-teardown', 'exec')
        for stage in ('worker', 'api', 'database', 'private_root'):
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as temp:
                artifacts = Path(temp)
                folder = artifacts / 'private-root'
                folder.mkdir()
                report = artifacts / 'report.json'
                report.write_text(json.dumps(dict(stats=dict(expected=0, unexpected=1), cleanup_verified=False, error=None)))
                recovery = Mock()
                sql = Mock()
                if stage == 'worker':
                    recovery.cleanup.side_effect = RuntimeError('private worker details')
                elif stage == 'database':
                    sql.side_effect = RuntimeError('private database details')
                api = Mock()
                api.poll.return_value = None
                if stage == 'api':
                    api.wait.side_effect = RuntimeError('private API details')
                files = Mock(wraps=shutil)
                if stage == 'private_root':
                    files.rmtree.side_effect = RuntimeError('private root details')
                namespace = dict(signal=Mock(), os=Mock(), recovery=recovery, browser=None, api=api,
                                 creation_attempted=True, name='isolated', sql=sql, failed=False,
                                 queue_mode=True, queue_report=report, queue_report_name='report',
                                 artifacts=artifacts, folder=folder, json=json, shutil=files, subprocess=subprocess)
                with self.assertRaisesRegex(RuntimeError, 'Isolated teardown failed'):
                    exec(code, namespace)
                api.wait.assert_called_once()
                sql.assert_called_once_with('DROP DATABASE IF EXISTS isolated WITH (FORCE)')
                self.assertEqual(folder.exists(), stage == 'private_root')
                saved = json.loads(report.read_text())
                self.assertFalse(saved['cleanup_verified'])
                self.assertEqual(saved['stats'], dict(expected=0, unexpected=1))
                self.assertEqual(len(saved['cleanup_errors']), 1)
                self.assertNotIn('details', saved['cleanup_errors'][0])


if __name__ == '__main__':
    unittest.main()
