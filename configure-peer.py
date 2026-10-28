#!/usr/bin/env python3
"""Point a stopped lab node at a colleague through private localhost SSH tunnels."""
from pathlib import Path
from urllib.parse import urlparse
import argparse,json,sqlite3,xml.etree.ElementTree as ET

ROOT=Path(__file__).resolve().parent
NS={'p':'http://holodeck-b2b.org/schemas/2014/10/pmode'}

def loopback_url(raw,as4=False):
 u=urlparse(raw)
 if u.scheme!='http' or u.hostname not in ['127.0.0.1','localhost','::1'] or u.username or u.password or u.query or u.fragment:
  raise SystemExit('Use an HTTP localhost SSH-tunnel URL; public/production endpoints are not accepted by this lab helper.')
 if as4 and u.path!='/holodeckb2b/as4':raise SystemExit('The peer AS4 path must be /holodeckb2b/as4')
 return raw

def main():
 p=argparse.ArgumentParser();p.add_argument('--role',required=True,choices=['brp','tso']);p.add_argument('--peer-as4',required=True);p.add_argument('--peer-ui');args=p.parse_args()
 peer=loopback_url(args.peer_as4,True)
 if args.peer_ui:loopback_url(args.peer_ui)
 node=ROOT/'runtime'/args.role
 if not (node/'config.json').exists():raise SystemExit('Run python3 setup.py first.')
 try:records=json.loads((ROOT/'runtime/pids.json').read_text())
 except FileNotFoundError:records={}
 for label,record in records.items():
  if not label.startswith(args.role+'-'):continue
  try:command=Path(f'/proc/{record["pid"]}/cmdline').read_bytes().replace(b'\0',b' ').decode()
  except FileNotFoundError:continue
  if record['identity'] in command:raise SystemExit(f'Stop this role first: ./run.sh stop --role {args.role}')
 dbpath=node/'app/app.sqlite3'
 if dbpath.exists():
  with sqlite3.connect(dbpath) as db:
   pending=db.execute("SELECT count(*) FROM outbox WHERE status IN ('queued','dispatching','unknown')").fetchone()[0]
  if pending:raise SystemExit('Resolve pending/uncertain handoffs before changing the peer route.')
 changed=0
 for path in (node/'gateway/repository/pmodes').glob('*.xml'):
  tree=ET.parse(path);address=tree.find('.//p:Protocol/p:Address',NS)
  if address is not None:address.text=peer;tree.write(path,encoding='UTF-8',xml_declaration=True);changed+=1
 if changed!=1:raise SystemExit('Expected one outgoing route; inspect the isolated gateway configuration.')
 if args.peer_ui:
  path=node/'config.json';config=json.loads(path.read_text());config['peer_url']=args.peer_ui;path.write_text(json.dumps(config,indent=2)+'\n')
 print(f'{args.role.upper()} outgoing test route set to {peer}. Restart that role to apply it.')

if __name__=='__main__':main()
