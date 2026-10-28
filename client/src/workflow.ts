import { xmlParts } from './xmlHighlights.ts';
import type { Message, Process } from './types';

export const familyName = (kind:string) => ({NOMINT:'TTF nomination',PRODOC:'Trading programme',PROCON:'Programme confirmation',NOMRES:'TTF confirmation',MARSIT:'Balancing allocation'}[kind] ?? kind);
export function nextAction(p:Process, role:string):string {
  if(p.kind==='MARSIT') return role==='tso'?'View allocation':'Review allocation';
  if(p.kind==='NOMRES') return p.confirmation_origin==='aggregate'?'Review aggregate confirmation':'Review confirmation';
  if(/Programme rejected|Programme review required/.test(p.stage)) return 'Review programme reasons';
  if(p.programme_origin==='default') return 'Review GTS programme';
  if(p.ack_status==='Rejected') return role==='brp' ? 'Correct & resubmit' : 'Request rejected';
  if(p.confirmed.length) return /Mismatch|No counter|Unchanged/i.test(p.stage) ? 'Review matching' : 'View confirmation';
  if(role==='tso') return p.ack_status==='Awaiting' ? 'Review request' : 'Send confirmation';
  return p.ack_status==='Accepted' ? 'Await confirmation' : 'Await acknowledgement';
}
export interface PipelineStep {family:string;title:string;status:string;messages:Message[]}
export function pipeline(p:Process, all:Message[]):PipelineStep[] {
  const messages=all.filter(m=>m.process_id===p.id || m.id===p.source_message_id || p.events.some(event=>event.message_id===m.id));
  const families=['MARSIT','NOMRES','PROCON'].includes(p.kind) ? [p.kind] : p.programme_origin==='default' && !messages.some(m=>m.family==='PRODOC') ? ['PROCON'] : [p.kind,'ACKNOW',p.kind==='PRODOC'?'PROCON':'NOMRES'];
  return families.map((family,i)=>{
    const found=messages.filter(m=>m.family===family).sort((a,b)=>b.version-a.version || b.at?.localeCompare(a.at));
    let status=found.length?'Recorded':'Expected';
    if(family==='ACKNOW') status=found.length?p.ack_status:'Awaiting';
    else if(i>1 && p.ack_status==='Rejected' && !found.length) status='Not expected';
    else if(i>1 && found.length) status=p.stage;
    else if(i===0 && found.length) status=found[0].direction==='out'?(found[0].transport_status||'Outbound'):p.kind==='MARSIT'?'Allocation received':p.programme_origin==='default'?'GTS-generated':'Received';
    return {family,title:({NOMINT:'Nomination',PRODOC:'Trading programme',ACKNOW:'Acknowledgement',NOMRES:'Matching result',PROCON:'Programme confirmation',MARSIT:'Balancing allocation'} as Record<string,string>)[family]??family,status,messages:found};
  });
}

// Compatibility helper for existing counterparty-only consumers.
export function counterpartyParts(xml:string):{text:string;highlight:boolean}[] {
  return xmlParts(xml).map(part=>({text:part.text,highlight:part.category==='counterparty'}));
}
