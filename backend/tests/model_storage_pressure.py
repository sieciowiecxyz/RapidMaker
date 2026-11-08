"""Linux/bwrap + local test PostgreSQL; run after building the API. No providers/UI."""
import http.cookiejar
import json
import os
from pathlib import Path
import secrets
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

os.umask(0o077)

def response(opener, url, **kwargs):
    try:
        with opener.open(urllib.request.Request(url, **kwargs), timeout=40) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()

def inside():
    model_root = Path(os.environ['MODEL_STORAGE_DIR'])
    model_root.chmod(0o700)
    api = subprocess.Popen([sys.argv[2]], start_new_session=True)
    try:
        base = 'http://localhost:' + os.environ['API_PORT']
        opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        for _ in range(300):
            if api.poll() is not None:
                raise RuntimeError('Owned API exited during startup')
            try:
                if response(opener, base+'/api/openapi.json')[0] == 200:
                    break
            except (urllib.error.URLError, TimeoutError):
                pass
            time.sleep(.1)
        else:
            raise RuntimeError('Owned API startup timeout')
        if os.environ['PROBE_PRESSURE'] == '1':
            stat = os.statvfs(model_root)
            with (model_root/'owned-filler').open('wb') as f:
                os.posix_fallocate(f.fileno(), 0, stat.f_bavail*stat.f_frsize-4*1024*1024)
        free = lambda: os.statvfs(model_root).f_bavail*os.statvfs(model_root).f_frsize
        before = free()
        minimum = [before]
        stop = threading.Event()
        def monitor():
            while not stop.is_set():
                minimum[0] = min(minimum[0], free())
                time.sleep(.0005)
        thread = threading.Thread(target=monitor)
        thread.start()
        try:
            status, body = response(opener, base+'/api/models', data=Path(sys.argv[3]).read_bytes(), headers={
                'Origin':os.environ['AUTH_ORIGIN'], 'Content-Type':'application/octet-stream', 'x-model-filename':'pressure.stl'})
        finally:
            stop.set()
            thread.join()
        report = {'pressure':os.environ['PROBE_PRESSURE']=='1','upload_status':status,
            'free_before':before,'minimum_free':minimum[0],'free_after':free(),
            'directories_after':sum(p.is_dir() for p in model_root.iterdir()),
            'api_alive':api.poll() is None,'message':json.loads(body).get('message')}
        if os.environ['PROBE_PRESSURE'] == '1':
            try:
                with (model_root/'owned-write-control').open('wb') as control:
                    control.write(Path(sys.argv[3]).read_bytes())
            except OSError as exc:
                report['write_control_errno'] = exc.errno
            finally:
                (model_root/'owned-write-control').unlink(missing_ok=True)
        print('RESULT '+json.dumps(report), flush=True)
    finally:
        if api.poll() is None:
            os.killpg(api.pid, signal.SIGTERM)
        api.wait(timeout=30)

def main():
    if sys.platform != 'linux' or not shutil.which('bwrap'):
        raise SystemExit('Requires Linux and bubblewrap; no database created')
    root = Path(__file__).resolve().parents[2]
    run = secrets.token_hex(6)
    out = root/'frontend/tests/artifacts'/('model-disk-'+run)
    out.mkdir(mode=0o700, parents=True)
    folder = Path(tempfile.mkdtemp(prefix='rapidmaker-disk-'))
    db = 'rapidmaker_test_browser_disk_'+run
    container = os.environ.get('TEST_PG_CONTAINER','rapidmaker-catalog-postgres')
    def sql(q, database='postgres'):
        return subprocess.run(['podman','exec',container,'psql','-U','rapidmaker','-d',database,'-v','ON_ERROR_STOP=1','-Atc',q],
            check=True,capture_output=True,timeout=20).stdout.decode().strip()
    values = {}
    for line in (root/'.env').read_text().splitlines():
        if line.startswith('DATABASE_URL='):
            values['DATABASE_URL'] = line.split('=',1)[1].strip().strip('"').strip("'")
    parsed = urllib.parse.urlsplit(values['DATABASE_URL'])
    assert parsed.hostname in ('localhost','127.0.0.1') and parsed.port == 18434
    values['DATABASE_URL'] = parsed._replace(path='/'+db).geturl()
    fixture = (root/'backend/crates/geometry/tests/fixtures/cube_20mm.stl').read_bytes()
    facets = []
    for i in range(struct.unpack_from('<I',fixture,80)[0]):
        t = struct.unpack_from('<12fH',fixture,84+50*i)
        facets.append('facet normal '+' '.join(map(str,t[:3]))+'\nouter loop\n'+''.join('vertex '+' '.join(map(str,t[j:j+3]))+'\n' for j in (3,6,9))+'endloop\nendfacet\n')
    data = ('solid pressure\n'+''.join(facets)+'endsolid pressure\n').encode()
    model = folder/'pressure.stl'
    model.write_bytes(data+b' '*(8*1024*1024-len(data)))
    binary = folder/'rapidmaker-api'
    shutil.copyfile(Path(sys.argv[1]) if len(sys.argv)>1 else root/'backend/target/debug/rapidmaker-api', binary)
    binary.chmod(0o500)
    reports = []
    created = False
    try:
        created = True
        sql('CREATE DATABASE '+db)
        for pressure, size in [(False,1024*1024*1024),(True,64*1024*1024)]:
            model_root = folder/('pressure-models' if pressure else 'healthy-models')
            model_root.mkdir(mode=0o700)
            with socket.socket() as sock:
                sock.bind(('127.0.0.1',0))
                port = sock.getsockname()[1]
            env = {k:v for k,v in os.environ.items() if k in ('PATH','HOME','LD_LIBRARY_PATH')}
            env.update(values, MODEL_STORAGE_DIR=str(model_root), API_PORT=str(port),
                AUTH_ORIGIN='http://localhost:'+str(port),AUTH_MODE='localhost',AUTH_CODE_KEY=secrets.token_hex(32),
                MAIL_MODE='disabled',MAIL_INBOUND_MODE='disabled',LOG_LEVEL='warn',PROBE_PRESSURE=str(int(pressure)))
            args=['bwrap','--unshare-user','--unshare-pid','--die-with-parent','--ro-bind','/','/',
                '--proc','/proc','--dev-bind','/dev','/dev','--size',str(size),'--tmpfs',str(model_root),
                sys.executable,str(Path(__file__).resolve()),'--inside',str(binary),str(model)]
            cp = subprocess.run(args,env=env,capture_output=True,timeout=95)
            (out/(run+('-pressure' if pressure else '-healthy')+'.log')).write_bytes(cp.stdout+cp.stderr)
            if cp.returncode:
                raise RuntimeError('Owned namespace probe failed, preserved log; exit '+str(cp.returncode))
            report = json.loads(next(line[7:] for line in cp.stdout.decode().splitlines() if line.startswith('RESULT ')))
            report['stored_rows'] = int(sql('SELECT count(*) FROM private_models', db))
            reports.append(report)
        result={'reports':reports,'input_bytes':model.stat().st_size,'private_database':True,'normal_api_untouched':True}
        (out/(run+'-result.json')).write_text(json.dumps(result,indent=2)+'\n')
        healthy, pressure = reports
        assert healthy['upload_status'] == 201 and healthy['directories_after'] == 1
        assert pressure['upload_status'] == 503 and pressure['directories_after'] == 0
        assert pressure['minimum_free'] == pressure['free_before'] == pressure['free_after']
        assert pressure['write_control_errno'] == 28
        assert pressure['message'].startswith('Brak miejsca')
        assert all(r['api_alive'] and r['stored_rows'] == 1 for r in reports)
    finally:
        try:
            if created:
                sql('DROP DATABASE IF EXISTS '+db+' WITH (FORCE)')
                assert sql("SELECT count(*) FROM pg_database WHERE datname='"+db+"'") == '0'
        finally:
            shutil.rmtree(folder)
        if 'result' in locals():
            result['cleanup_verified'] = True
            (out/(run+'-result.json')).write_text(json.dumps(result,indent=2)+'\n')
    assert not folder.exists()
    result['cleanup_verified'] = True
    (out/(run+'-result.json')).write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'status':'PASS','artifact':str(out/(run+'-result.json')),**result}))

if __name__ == '__main__':
    inside() if len(sys.argv)>1 and sys.argv[1]=='--inside' else main()
