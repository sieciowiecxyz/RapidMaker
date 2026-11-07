"""Actual 180-second lease recovery; all processes and files belong to this run."""
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess
import time

from queue_restart import require


def process_identity(pid):
    try:
        fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
        return fields[0], int(fields[1]), fields[19]  # state, parent, start time
    except (FileNotFoundError, ProcessLookupError):
        return None


class QueueRecovery:
    def __init__(self, root, folder, binary, env):
        self.root, self.folder, self.binary = root, folder, binary
        self.env = dict(env, LOG_LEVEL='info')
        self.workers = []
        self.descendants = {}

    def start(self, capture=False):
        log = self.folder / f'worker-{len(self.workers) + 1}.jsonl'
        with log.open('ab') as output:
            log.chmod(0o600)
            worker = subprocess.Popen([str(self.binary), '--slicer-worker'], cwd=self.root,
                                      env=self.env, stdin=subprocess.DEVNULL, stdout=output,
                                      stderr=subprocess.PIPE if capture else output, start_new_session=True)
        # Register before any observation or assertion can fail.
        self.workers.append(worker)
        return worker, log

    def capture_descendants(self, worker):
        owned = {worker.pid}
        processes = {}
        for path in Path('/proc').iterdir():
            if path.name.isdigit():
                identity = process_identity(int(path.name))
                if identity:
                    processes[int(path.name)] = identity
        while True:
            children = {pid for pid, (_, parent, _) in processes.items() if parent in owned} - owned
            if not children:
                break
            owned.update(children)
        self.descendants.update({pid: processes[pid][2] for pid in owned if pid != worker.pid})

    def kill(self, worker):
        if worker.poll() is None:
            self.capture_descendants(worker)
            try:
                os.killpg(worker.pid, signal.SIGKILL)  # also works when SIGSTOPped
            except ProcessLookupError:
                pass
            worker.wait(timeout=10)
        # bwrap --new-session may escape killpg; only recorded descendants are eligible.
        for pid, started in self.descendants.items():
            identity = process_identity(pid)
            if identity and identity[2] == started and identity[0] != 'Z':
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            alive = [pid for pid, started in self.descendants.items()
                     if (state := process_identity(pid)) and state[2] == started and state[0] != 'Z']
            if not alive:
                return
            time.sleep(0.05)
        raise AssertionError('Owned worker descendants survived cleanup')

    def cleanup(self):
        errors = []
        try:
            for worker in self.workers:
                try:
                    self.kill(worker)
                except Exception as exc:
                    errors.append(type(exc).__name__)
        finally:
            for worker in self.workers:
                if worker.stderr:
                    try:
                        worker.stderr.close()
                    except Exception as exc:
                        errors.append(type(exc).__name__)
        if errors:
            # Exception messages may contain private paths/data; keep the summary bounded.
            raise RuntimeError(f"Worker cleanup failed ({len(errors)} errors: {', '.join(errors[:4])})")

    def copy_failure_logs(self, artifacts, name):
        for log in self.folder.glob('worker-*.jsonl'):
            destination = artifacts / f'{name}-{log.name}'
            shutil.copyfile(log, destination)
            destination.chmod(0o600)

    def run(self, job_id, sql, request):
        worker, log = self.start(capture=True)
        os.set_blocking(worker.stderr.fileno(), False)
        pending = b''
        claimed = False
        deadline = time.monotonic() + 30
        with selectors.DefaultSelector() as selector, log.open('ab') as output:
            selector.register(worker.stderr, selectors.EVENT_READ)
            while time.monotonic() < deadline and not claimed:
                require(worker.poll() is None, 'Worker exited before claimed-job event')
                for _, _ in selector.select(timeout=0.1):
                    chunk = os.read(worker.stderr.fileno(), 65536)
                    require(bool(chunk), 'Worker stderr closed before claimed-job event')
                    output.write(chunk)
                    output.flush()
                    pending += chunk
                    while b'\n' in pending:
                        line, pending = pending.split(b'\n', 1)
                        event = json.loads(line)
                        if event.get('fields', {}).get('event') == 'slicer_started' and event.get('span', {}).get('job_id') == job_id:
                            os.killpg(worker.pid, signal.SIGSTOP)
                            claimed = True
                            break
        require(claimed, 'No matching claimed-job event within 30s')

        def state():
            return json.loads(sql(f"SELECT json_build_object('status',status,'attempts',attempts,'leased',lease_token IS NOT NULL,'lease_until',lease_until,'remaining',lease_until-extract(epoch FROM clock_timestamp()),'snapshots',(SELECT count(*) FROM quotes)) FROM slicer_jobs WHERE id='{job_id}'"))

        stopped = state()
        require(stopped['status'] == 'running' and stopped['attempts'] == 1 and stopped['leased']
                and stopped['lease_until'] is not None and stopped['remaining'] > 0 and stopped['snapshots'] == 0,
                'Job was not running under first active lease when stopped (already finished is a failure)')
        completion_deadline = time.monotonic() + stopped['remaining'] + 50
        self.kill(worker)
        replacement, _ = self.start()
        require(replacement.pid != worker.pid, 'Worker was not replaced')
        require(state()['remaining'] > 150, 'Replacement launched too late in active lease')
        # Deliberately let the replacement poll a still-valid lease for seven seconds.
        time.sleep(7)
        held = state()
        require(replacement.poll() is None, 'Replacement worker exited')
        require(held['status'] == 'running' and held['attempts'] == 1 and held['leased']
                and held['lease_until'] == stopped['lease_until'] and held['remaining'] > 0 and held['snapshots'] == 0,
                'Replacement changed a valid lease or committed a premature snapshot')
        require(json.loads(request('/api/quotes/jobs/' + job_id))['status'] == 'running', 'API not responsive during recovery')
        print(f'Queue recovery: real lease wait live; owned replacement PID {replacement.pid}; no expiry mutation.', flush=True)
        return dict(old_worker_pid=worker.pid, old_worker_exit=worker.returncode, replacement_worker_pid=replacement.pid,
                    claimed_job_event=True, stopped=stopped, held_after_seconds=7, held=held,
                    captured_descendants=len(self.descendants), owned_descendants_gone=True,
                    completion_deadline=completion_deadline)
