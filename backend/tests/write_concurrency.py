"""Bound write bodies using the real isolated API/TCP. Linux loopback aliases; no DB, providers or browser."""
import json, os, secrets, shutil, signal, socket, subprocess, sys, tempfile, time, urllib.request, urllib.error
from pathlib import Path
os.umask(0o077)
root=Path(__file__).resolve().parents[2]
name=secrets.token_hex(6)
out=root/'frontend/tests/artifacts'/('json-inflight-'+name)
out.mkdir(mode=0o700,parents=True)
folder=Path(tempfile.mkdtemp(prefix='rapidmaker-json-budget-'))
binary=folder/'rapidmaker-api'
shutil.copyfile(Path(sys.argv[1]) if len(sys.argv)>1 else root/'backend/target/debug/rapidmaker-api',binary)
binary.chmod(0o500)
with socket.socket() as listener:
    listener.bind(('127.0.0.1',0));port=listener.getsockname()[1]
base=f'http://localhost:{port}'
env={k:v for k,v in os.environ.items() if k in ('PATH','HOME','LD_LIBRARY_PATH')}
env.update(API_PORT=str(port),AUTH_MODE='disabled',AUTH_ORIGIN=base,AUTH_CODE_KEY=secrets.token_hex(32),MAIL_MODE='disabled',MAIL_INBOUND_MODE='disabled',SLICER_WORKER_MODE='external',LOG_LEVEL='warn')
def request(path,data=None):
    req=urllib.request.Request(base+path,data=data,headers={'Origin':base,'Content-Type':'application/json'})
    try:
        with urllib.request.urlopen(req,timeout=3) as r:return r.status
    except urllib.error.HTTPError as r:return r.code
def peer_write(ip='127.0.0.2'):
    with socket.create_connection(('127.0.0.1',port),timeout=3,source_address=(ip,0)) as sock:
        data=b'{"parts":[],"delivery":"pickup"}'
        sock.sendall((f'POST /api/shipping/estimate HTTP/1.1\r\nHost: localhost:{port}\r\nOrigin: {base}\r\nContent-Type: application/json\r\nContent-Length: {len(data)}\r\nConnection: close\r\n\r\n').encode()+data)
        return int(sock.recv(2048).split(b' ',2)[1])
def rss():
    return int(next(line.split()[1] for line in Path(f'/proc/{api.pid}/status').read_text().splitlines() if line.startswith('VmRSS:')))*1024
connections=[]
result=None
with (out/(name+'-api.log')).open('wb') as log:
    api=subprocess.Popen([str(binary)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
    try:
        deadline=time.monotonic()+15
        while time.monotonic()<deadline:
            assert api.poll() is None
            try:
                if request('/api/openapi.json')==200:break
            except urllib.error.URLError:pass
            time.sleep(.05)
        else:raise RuntimeError('Private API not ready')
        before=rss();body=b'{"parts":[],"delivery":"pickup","padding":"'+b'x'*(768*1024)
        for i in range(48):
            sock=socket.create_connection(('127.0.0.1',port),timeout=3)
            sock.sendall((f'POST /api/webhooks/stripe HTTP/1.1\r\nHost: localhost:{port}\r\nOrigin: {base}\r\nX-Forwarded-For: 203.0.113.{i+1}\r\nContent-Type: application/json\r\nContent-Length: {len(body)+2}\r\n\r\n').encode())
            connections.append(sock)
        time.sleep(.3)
        statuses=[];active=[]
        for sock in connections:
            sock.settimeout(.02)
            try:
                response=sock.recv(2048)
                statuses.append(int(response.split(b' ',2)[1]));assert b'Retry-After: 1' in response or b'retry-after: 1' in response
            except TimeoutError:active.append(sock)
        for sock in active:
            sock.settimeout(3);sock.sendall(body)
        time.sleep(.3)
        held_after_body=0
        for sock in active:
            sock.settimeout(.02)
            try:
                extra=sock.recv(2048)
                raise RuntimeError('Body unexpectedly completed with HTTP '+extra[:30].decode(errors='replace'))
            except TimeoutError:held_after_body+=1
        result={'held_after_body':held_after_body,'attempted':len(connections),'held':len(active),'rejected_statuses':statuses,'rss_before':before,'rss_after':rss(),'read_status':request('/api/openapi.json'),'other_ip_write_while_busy':peer_write()}
        peers=[]
        for _ in range(16):
            sock=socket.create_connection(('127.0.0.1',port),timeout=3,source_address=('127.0.0.2',0))
            sock.sendall((f'POST /api/webhooks/stripe HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Length: {len(body)+2}\r\n\r\n').encode()+body)
            peers.append(sock);connections.append(sock)
        time.sleep(.3)
        result['global_full_status']=peer_write('127.0.0.3')
        result['read_while_global_full']=request('/api/openapi.json')
        for sock in peers:sock.close();connections.remove(sock)
        deadline=time.monotonic()+5
        while time.monotonic()<deadline:
            status=peer_write('127.0.0.3')
            if status==200:break
            time.sleep(.05)
        result['other_ip_after_release']=status
        for sock in connections:sock.close()
        connections=[]
        deadline=time.monotonic()+5
        while time.monotonic()<deadline:
            status=request('/api/shipping/estimate',b'{"parts":[],"delivery":"pickup"}')
            if status==200:break
            time.sleep(.05)
        result['write_after_disconnect']=status
    finally:
        for sock in connections:sock.close()
        if api.poll() is None:os.killpg(api.pid,signal.SIGTERM)
        api.wait(timeout=30)
        shutil.rmtree(folder)
        if result is not None:
            result['cleanup_verified']=not folder.exists() and api.poll() is not None
            (out/(name+'-result.json')).write_text(json.dumps(result,indent=2)+'\n')
assert result is not None
assert result['attempted']==48 and result['held_after_body']==result['held']==16
assert result['rejected_statuses']==[429]*32
assert result['other_ip_write_while_busy']==200 and result['global_full_status']==429 and result['read_while_global_full']==200 and result['other_ip_after_release']==200
assert result['read_status']==200 and result['write_after_disconnect']==200 and result['cleanup_verified']
print(json.dumps({'status':'PASS','artifact':str(out/(name+'-result.json')),**result}))
