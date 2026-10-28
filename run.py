#!/usr/bin/env python3
"""Start/stop only this lab's recorded processes, without touching existing gateways."""
from pathlib import Path
import argparse, json, os, signal, socket, subprocess, sys, time
from urllib.request import urlopen

ROOT=Path(__file__).resolve().parent
PIDFILE=ROOT/'runtime/pids.json'

def alive(record):
 try:
  raw=Path(f'/proc/{record["pid"]}/cmdline').read_bytes().replace(b'\0',b' ').decode()
  return record['identity'] in raw
 except (FileNotFoundError,PermissionError):return False

def load():
 try:return json.loads(PIDFILE.read_text())
 except FileNotFoundError:return {}

def available(port):
 with socket.socket() as sock:
  sock.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
  sock.bind(('127.0.0.1',port))

def ready(url):
 try:
  with urlopen(url,timeout=.4) as r:return r.status==200
 except Exception:return False

def spawn(label,command,identity,log,env,records):
 if label in records and alive(records[label]):return
 log.parent.mkdir(parents=True,exist_ok=True)
 with open(log,'ab',buffering=0) as out:
  proc=subprocess.Popen(command,cwd=ROOT,env=env,stdout=out,stderr=subprocess.STDOUT,start_new_session=True)
 records[label]={'pid':proc.pid,'identity':identity,'log':str(log)}
 PIDFILE.write_text(json.dumps(records,indent=2))

def main():
 parser=argparse.ArgumentParser();parser.add_argument('command',nargs='?',default='start',choices=['start','stop','status','build']);parser.add_argument('--role',choices=['brp','tso','both'],default='both');args=parser.parse_args()
 roles=['brp','tso'] if args.role=='both' else [args.role]
 if args.command=='build':
  env=os.environ.copy();env['CARGO_HOME']=str(ROOT/'.cargo')
  subprocess.run(['cargo','build','--locked'],cwd=ROOT,env=env,check=True)
  subprocess.run(['npm','ci','--cache',str(ROOT/'.cache/npm')],cwd=ROOT/'client',check=True)
  subprocess.run(['npm','run','build'],cwd=ROOT/'client',check=True)
  return
 records=load()
 if args.command=='status':
  for label,rec in records.items():print(f'{label}: {"running" if alive(rec) else "stopped"} / {rec["log"]}')
  if not records:print('Lab has not been started.')
  return
 if args.command=='stop':
  for role in roles:
   for component in ['app','gateway']:
    rec=records.get(f'{role}-{component}')
    if rec and alive(rec):os.kill(rec['pid'],signal.SIGTERM)
  deadline=time.monotonic()+15
  while time.monotonic()<deadline and any(alive(r) for label,r in records.items() if label.split('-')[0] in roles):time.sleep(.15)
  print('Requested lab nodes stopped. Saved application and gateway state retained.')
  return
 subprocess.run([sys.executable,str(ROOT/'setup.py')],check=True)
 binary=ROOT/'target/debug/nomination-lab'
 if not binary.exists() or not (ROOT/'client/dist/index.html').exists():raise SystemExit('Build first: ./run.sh build')
 for role in roles:
  for component,port in [('app',8797 if role=='brp' else 8798),('gateway',19080 if role=='brp' else 19081)]:
   label=f'{role}-{component}'
   if label not in records or not alive(records[label]):available(port)
 for role in roles:
  node=ROOT/'runtime'/role;gateway=node/'gateway'
  env=os.environ.copy();env['JAVA_HOME']=os.environ.get('LAB_JAVA_HOME',str(ROOT.parent/'java17'));env['HB2B_HOME']=str(gateway);env['HB2B_DB_DIR']=str(gateway/'db');env['JAVA_TOOL_OPTIONS']='-Xms64m -Xmx256m'
  spawn(f'{role}-app',[str(binary),'--config',str(node/'config.json')],str(node/'config.json'),node/'logs/app.log',env,records)
  spawn(f'{role}-gateway',['bash',str(gateway/'bin/startServer.sh')],str(gateway),node/'logs/gateway.log',env,records)
 deadline=time.monotonic()+45
 while time.monotonic()<deadline:
  apps=all(ready(f'http://127.0.0.1:{8797 if r=="brp" else 8798}/api/info') for r in roles)
  gateways=True
  for r in roles:
   try:
    with socket.create_connection(('127.0.0.1',19080 if r=='brp' else 19081),timeout=.3):pass
   except OSError:gateways=False
  if apps and gateways:
   print('Local nomination lab is running:')
   for r in roles:print(f'  {r.upper()}: http://127.0.0.1:{8797 if r=="brp" else 8798}  /  {r} / local-{r}')
   print('Only synthetic test identities and example keys are configured.');return
  for label,rec in records.items():
   if label.split('-')[0] in roles and not alive(rec):raise SystemExit(f'{label} exited. Inspect {rec["log"]}')
  time.sleep(.3)
 raise SystemExit('Readiness timed out. Inspect runtime/<role>/logs and gateway/logs.')

if __name__=='__main__':main()
