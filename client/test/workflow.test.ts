import { test } from 'node:test';
import assert from 'node:assert/strict';
import { pipeline, counterpartyParts } from '../src/workflow.ts';
import type { Process, Message } from '../src/types.ts';

const process = {id:'p',kind:'NOMINT',ack_status:'Rejected',source_message_id:'n',confirmed:[],events:[]} as unknown as Process;
test('rejected submission stops the expected result and retains clickable request and acknowledgement', () => {
  const messages = [{id:'n',family:'NOMINT',process_id:'p'},{id:'a',family:'ACKNOW',process_id:'p'}] as Message[];
  const steps = pipeline(process,messages);
  assert.deepEqual(steps.map(s=>s.family),['NOMINT','ACKNOW','NOMRES']);
  assert.equal(steps[2].status,'Not expected');
  assert.deepEqual(steps[1].messages.map(m=>m.id),['a']);
});
test('a default programme never invents a submitted PRODOC or ACKNOW', () => {
  const steps = pipeline({...process,kind:'PRODOC',programme_origin:'default',ack_status:'Not applicable'},[]);
  assert.deepEqual(steps.map(s=>s.family),['PROCON']);
});
test('counterparty highlighting preserves every byte and excludes recipient, comments and attributes', () => {
  const xml='<Doc><recipient>GSLABCOUNTER</recipient><!-- <externalAccount>X</externalAccount> --><External_Account><externalAccount codingScheme="ZSO">GSLABCOUNTER</externalAccount></External_Account></Doc>';
  const parts=counterpartyParts(xml);
  assert.equal(parts.map(p=>p.text).join(''),xml);
  assert.deepEqual(parts.filter(p=>p.highlight).map(p=>p.text),['GSLABCOUNTER']);
});
test('programme highlights reserved portfolio values without highlighting its own contract', () => {
  const xml='<LoadForecast_Document><contract_Reference.identification>GSBRP</contract_Reference.identification><ConnectionPoint><Account><identification codingScheme="ZSO">GSTPTRADE</identification></Account></ConnectionPoint></LoadForecast_Document>';
  assert.deepEqual(counterpartyParts(xml).filter(p=>p.highlight).map(p=>p.text),['GSTPTRADE']);
});
test('outgoing allocation never claims to have been received',()=>{
 const steps=pipeline({...process,kind:'MARSIT',ack_status:'Not applicable'},[{id:'m',family:'MARSIT',process_id:'p',direction:'out',transport_status:'Queued'}] as Message[]);
 assert.equal(steps[0].status,'Queued');
});
test('independent NOMRES has no invented request or acknowledgement and links its actual message',()=>{
 const p={...process,kind:'NOMRES',confirmation_origin:'unsolicited',ack_status:'Not applicable',source_message_id:'n',events:[]} as Process;
 assert.deepEqual(pipeline(p,[{id:'n',family:'NOMRES',direction:'in'}] as Message[]).map(s=>s.family),['NOMRES']);
});
test('a linked consolidated confirmation is found through the process evidence even when stored on its own process',()=>{
 const p={...process,events:[{id:'e',kind:'result',title:'Received',detail:'',at:'2026-10-27',message_id:'consolidated'}]};
 const steps=pipeline(p,[{id:'consolidated',family:'NOMRES',process_id:'independent',direction:'in'}] as Message[]);
 assert.equal(steps.at(-1)?.messages[0]?.id,'consolidated');
});
