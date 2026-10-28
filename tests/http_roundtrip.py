#!/usr/bin/env python3
"""Exercise two running independent apps through actual Holodeck AS4 gateways."""
from pathlib import Path
from urllib.request import Request,build_opener,HTTPCookieProcessor
from http.cookiejar import CookieJar
from datetime import date,timedelta
import json,time,uuid

ROOT=Path(__file__).resolve().parents[1]
class Client:
 def __init__(self,role,port):
  self.url=f'http://127.0.0.1:{port}';self.opener=build_opener(HTTPCookieProcessor(CookieJar()));self.csrf=''
  self.csrf=self.call('/api/login',{'username':role,'password':f'local-{role}'})['csrf']
 def call(self,path,data=None,method=None):
  headers={'Origin':self.url,'X-CSRF-Token':self.csrf}
  if data is not None:headers['Content-Type']='application/json'
  req=Request(self.url+path,data=json.dumps(data).encode() if data is not None else None,headers=headers,method=method or ('POST' if data is not None else 'GET'))
  with self.opener.open(req,timeout=15) as r:
   return json.loads(r.read()) if 'json' in r.headers.get('Content-Type','') else r.read().decode()
 def state(self):return self.call('/api/state')

def wait(fn,label,timeout=35):
 deadline=time.monotonic()+timeout
 while time.monotonic()<deadline:
  value=fn()
  if value:return value
  time.sleep(.3)
 raise AssertionError(f'Timed out: {label}')

def key():return str(uuid.uuid4())

def main():
 brp=Client('brp',8797);tso=Client('tso',8798);evidence=[]
 def submit(day,kind='NOMINT'):
  hours=brp.call('/api/calendar?day='+day)['hours']
  draft=brp.call('/api/drafts',{'kind':kind,'gas_day':day,'counterparty':'GSLABCOUNTER','direction':'sell','quantities':[0 if kind=='PRODOC' else 10000]*len(hours),'document_id':None,'version':1})
  preview=brp.call(f'/api/drafts/{draft["id"]}/preview',{})
  assert preview['validation']['valid'],preview['validation']
  command={'hash':preview['hash'],'idempotency_key':key()}
  sent=brp.call(f'/api/drafts/{draft["id"]}/send',command)
  assert brp.call(f'/api/drafts/{draft["id"]}/send',command)==sent
  incoming=wait(lambda:next((p for p in tso.state()['processes'] if p['document_id']==draft['input']['document_id'] and p['version']==draft['input']['version']),None),'TSO payload callback')
  wait(lambda:next((p for p in brp.state()['processes'] if p['id']==sent['process_id'] and p['transport_status']=='Receipt received'),None),'signed AS4 receipt')
  return sent['process_id'],incoming
 def accept(inbound):
  tso.call(f'/api/processes/{inbound["id"]}/ack',{'accepted':True,'reason':'','idempotency_key':key()})
  wait(lambda:next((p for p in brp.state()['processes'] if p['document_id']==inbound['document_id'] and p['version']==inbound['version'] and p['ack_status']=='Accepted'),None),'reverse ACKNOW callback')
 def match(inbound,quantities,expected):
  if inbound['kind']=='NOMINT':tso.call(f'/api/fixtures/{inbound["id"]}',{'quantities':quantities},'PUT')
  tso.call(f'/api/processes/{inbound["id"]}/result',{'idempotency_key':key()})
  p=wait(lambda:next((p for p in brp.state()['processes'] if p['document_id']==inbound['document_id'] and p['version']==inbound['version'] and p['stage']==expected),None),expected)
  evidence.append({'kind':p['kind'],'document_id':p['document_id'],'version':p['version'],'outcome':p['stage'],'confirmed':p['confirmed'],'events':p['events']})
  return p
 base=date.today()+timedelta(days=7)
 pid,request=submit(base.isoformat());accept(request)
 settled=match(request,[10000]*len(request['quantities']),'Settled');assert settled['confirmed']==[10000]*len(request['quantities'])
 # Both requested and confirmed values remain visible during a unilateral revision.
 revision=brp.call(f'/api/processes/{pid}/revise',{})
 revision['input']['quantities']=[9000]*len(request['quantities'])
 brp.call(f'/api/drafts/{revision["id"]}',revision['input'],'PUT')
 preview=brp.call(f'/api/drafts/{revision["id"]}/preview',{})
 brp.call(f'/api/drafts/{revision["id"]}/send',{'hash':preview['hash'],'idempotency_key':key()})
 revised=wait(lambda:next((p for p in tso.state()['processes'] if p['document_id']==request['document_id'] and p['version']==2),None),'revision callback')
 accept(revised);unchanged=match(revised,[10000]*len(request['quantities']),'Unchanged settlement');assert unchanged['confirmed']==settled['confirmed']
 _,request=submit((base+timedelta(days=1)).isoformat());accept(request)
 mismatch=match(request,[8000]*len(request['quantities']),'Mismatch');assert all(v==8000 for v in mismatch['confirmed'])
 _,request=submit((base+timedelta(days=2)).isoformat());accept(request);match(request,None,'No counter-nomination')
 _,request=submit((base+timedelta(days=3)).isoformat(),'PRODOC');accept(request)
 programme=match(request,None,'Programme confirmed');assert all(v==0 for v in programme['confirmed'])
 _,request=submit((base+timedelta(days=4)).isoformat())
 tso.call(f'/api/processes/{request["id"]}/ack',{'accepted':False,'reason':'Simulated business rejection for this test case','idempotency_key':key()})
 rejected=wait(lambda:next((p for p in brp.state()['processes'] if p['document_id']==request['document_id'] and p['stage']=='Rejected'),None),'business rejection')
 evidence.append({'kind':rejected['kind'],'document_id':rejected['document_id'],'outcome':rejected['stage']})
 # Independent GTS-generated programme travels through the same AS4 gateway.
 default_date=base+timedelta(days=5)
 used_days={p['gas_day'] for p in tso.state()['processes']}
 while default_date.isoformat() in used_days:default_date+=timedelta(days=1)
 default_day=default_date.isoformat()
 command={'gas_day':default_day,'idempotency_key':key()}
 default_sent=tso.call('/api/programmes/default',command)
 assert tso.call('/api/programmes/default',command)==default_sent
 default_tso=next(p for p in tso.state()['processes'] if p['id']==default_sent['process_id'])
 default_brp=wait(lambda:next((p for p in brp.state()['processes'] if p['document_id']==default_tso['document_id'] and p.get('programme_origin')=='default'),None),'independent DEFAULT PROCON')
 assert default_brp['kind']=='PROCON' and default_brp['ack_status']=='Not applicable'
 evidence.append({'kind':'PROCON','origin':'default','document_id':default_brp['document_id']})
 # Both 6.1 allocation types: reviewed exact XML, AS4 callbacks, revisions, transaction detail.
 allocation_day=(base+timedelta(days=6)).isoformat()
 hours=tso.call('/api/calendar?day='+allocation_day)['hours']
 for code in ['ANZ','AVI']:
  doc_id=None
  for version in [1,2]:
   input={'gas_day':allocation_day,'document_code':code,'document_id':doc_id,'version':version,'transactions':[
    {'identification':'TR000001','periods':[{'start':hours[0]['start'],'end':hours[-1]['end'],'direction':'debit','quantity':2500+version,'price':'0.035000'}]},
    {'identification':'TR000002','periods':[{'start':hours[1]['start'],'end':hours[1]['end'],'direction':'credit','quantity':500,'price':'-0.010000'}]}]}
   preview=tso.call('/api/balancing/preview',input)
   assert preview['validation']['valid'],preview['validation']
   doc_id=preview['input']['document_id']
   command={'preview_id':preview['id'],'hash':preview['hash'],'idempotency_key':key()}
   sent=tso.call('/api/balancing/send',command)
   assert tso.call('/api/balancing/send',command)==sent
   allocation=wait(lambda:next((p for p in brp.state()['processes'] if p['document_id']==doc_id and p['version']==version),None),'MARSIT '+code+' v'+str(version))
   assert allocation['kind']=='MARSIT' and allocation['balancing']['transactions'][1]['periods'][0]['price']=='-0.010000'
   message=next(m for m in brp.state()['messages'] if m['id']==allocation['source_message_id'])
   assert message['xml']==preview['xml'] and message['hash']==preview['hash']
   detail=brp.call('/api/messages/'+message['id']+'/details')
   assert detail['message']['xml']==preview['xml']
   if version==2:
    old=next(p for p in brp.state()['processes'] if p['document_id']==doc_id and p['version']==1)
    assert old['stage']=='Superseded allocation'
   wait(lambda:next((p for p in tso.state()['processes'] if p['id']==sent['process_id'] and p['transport_status']=='Receipt received'),None),'MARSIT signed receipt')
   evidence.append({'kind':'MARSIT','code':code,'document_id':doc_id,'version':version,'hash':message['hash'],'transactions':allocation['balancing']['transactions']})
 for role,client in [('brp',brp),('tso',tso)]:
  state=client.state();assert state['connection']['unknown']==0
  evidence.append({'node':role,'connection':state['connection'],'messages':[{'family':m['family'],'direction':m['direction'],'as4_id':m['id'],'hash':m['hash']} for m in state['messages']]})
 out=ROOT/'test-output';out.mkdir(exist_ok=True)
 (out/'as4-roundtrip.json').write_text(json.dumps(evidence,indent=2))
 print('PASS: real AS4 equal, unilateral revision, mismatch, missing counterparty, programme, business rejection, DEFAULT PROCON and ANZ/AVI allocation revisions; signed receipt callbacks and idempotent Send observed.')
 print(out/'as4-roundtrip.json')

if __name__=='__main__':main()
