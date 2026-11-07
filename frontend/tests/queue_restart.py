"""Runner-owned HTTP/process regression; no browser or provider fixtures."""
import hashlib
import http.cookiejar
import json
import subprocess
import time
import urllib.error
import urllib.request
import uuid


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def run_queue_restart(root, folder, container, database, restart, evidence):
    def sql(query):
        return subprocess.check_output([
            'podman', 'exec', container, 'psql', '-U', 'rapidmaker', '-d', database,
            '-At', '-v', 'ON_ERROR_STOP=1', '-c', query,
        ], text=True, timeout=15).strip()

    require(sql('SELECT current_database()') == database, 'Wrong database')
    cookies = http.cookiejar.CookieJar()
    owner = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookies))
    foreign = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def request(path, expected=200, data=None, client=owner, headers=None):
        req = urllib.request.Request('http://localhost:18436' + path, data=data, headers={
            'Host': 'localhost:18437', 'Origin': 'http://localhost:18437', **(headers or {}),
        })
        try:
            response = client.open(req, timeout={'/api/models': 40, '/api/quotes/estimate': 30}.get(path, 5))
        except urllib.error.HTTPError as exc:
            response = exc
        with response:
            body = response.read()
            require(response.status == expected, f'{path}: HTTP {response.status}, expected {expected}')
            return body

    source = (root / 'backend/crates/geometry/tests/fixtures/cube_20mm.stl').read_bytes()
    model = json.loads(request('/api/models', 201, source, headers={
        'Content-Type': 'application/octet-stream', 'x-model-filename': 'cube_20mm.stl',
    }))
    model_id = str(uuid.UUID(model['id']))
    original = folder / 'models' / model_id / 'model.stl'
    digest = hashlib.sha256(source).hexdigest()
    require(hashlib.sha256(original.read_bytes()).hexdigest() == digest, 'Upload hash mismatch')
    catalog = json.loads(request('/api/catalog/materials'))
    material = next(m for m in catalog['materials'] if m['id'] == 'GFA00')
    payload = json.dumps(dict(model_id=model_id, material_id='GFA00', color_code=material['colors'][0]['code'],
                              options=dict(quantity=1, quality='standard', wall_loops=3, infill_percent=20))).encode()
    submitted = json.loads(request('/api/quotes/estimate', 202, payload, headers={'Content-Type': 'application/json'}))
    job_id = str(uuid.UUID(submitted['id']))
    poll = '/api/quotes/jobs/' + job_id
    require(submitted['status'] == 'queued', 'Submission must remain queued without worker')
    require(json.loads(request(poll))['status'] == 'queued', 'GET did not observe queued job')

    def db_state():
        return json.loads(sql(f"SELECT json_build_object('status',status,'attempts',attempts,'leased',lease_token IS NOT NULL OR lease_until IS NOT NULL) FROM slicer_jobs WHERE id='{job_id}'"))

    queued = db_state()
    require(queued == dict(status='queued', attempts=0, leased=False), f'Unexpected queued state: {queued}')
    require(sql('SELECT count(*) FROM quotes') == '0', 'Snapshot exists before processing')
    owner_identity = [(c.name, c.value, c.domain, c.path) for c in cookies]
    require(bool(owner_identity), 'Upload did not create owner cookie')
    evidence.update(model_id=model_id, job_id=job_id, queued=queued, source_sha256=digest)
    evidence.update(restart())
    # Do not bootstrap a new owner after restart: the exact existing cookie must work.
    require([(c.name, c.value, c.domain, c.path) for c in cookies] == owner_identity, 'Cookie changed during restart')
    deadline = time.monotonic() + 160
    while time.monotonic() < deadline:
        job = json.loads(request(poll))
        if job['status'] in ('succeeded', 'failed'):
            break
        time.sleep(0.5)
    else:
        raise AssertionError('Worker completion timeout (160s)')
    require(job['status'] == 'succeeded', f"Worker ended {job['status']}: {job.get('message')}")
    result = job['result']
    measurement = result['measurement']
    require(result['id'] == job_id and result['model_id'] == model_id, 'Result identity mismatch')
    require(measurement['quantity'] == 1 and measurement['time_seconds'] > 0 and measurement['filament_grams'] > 0, 'Missing real slicer measurement')
    require(result['price']['gross_grosze'] > 0, 'Missing measured price')
    finished = db_state()
    require(finished == dict(status='succeeded', attempts=1, leased=False), f'Unexpected completed state: {finished}')
    snapshots = json.loads(sql(f"SELECT json_build_object('count',count(*),'matching',count(*) FILTER (WHERE id='{job_id}' AND result=(SELECT result FROM slicer_jobs WHERE id='{job_id}'))) FROM quotes"))
    require(snapshots == dict(count=1, matching=1), f'Unexpected snapshots: {snapshots}')
    require(json.loads(request(poll)) == job, 'Repeated GET changed completed result')
    preview = request(f'/api/models/{model_id}/preview.glb')
    require(preview[:4] == b'glTF', 'Invalid private GLB')
    # Give a second guest its own identity; it must not inherit the original owner.
    request('/api/models', 201, source, client=foreign, headers={
        'Content-Type': 'application/octet-stream', 'x-model-filename': 'foreign.stl',
    })
    request(f'/api/models/{model_id}/preview.glb', 404, client=foreign)
    request(poll, 404, client=foreign)
    require(hashlib.sha256(original.read_bytes()).hexdigest() == digest, 'Restart/slicer changed source file')
    require(db_state() == finished and sql('SELECT count(*) FROM quotes') == '1', 'Repeated access duplicated processing')
    evidence.update(finished=finished, snapshots=snapshots, measurement=measurement, price=result['price'],
                    same_owner_cookie=True, source_preserved=True, owner_glb_status=200,
                    foreign_glb_status=404, foreign_job_status=404,
                    limits='Queued-job process restart only; no running-lease recovery, browser UI, providers or host reboot.')
