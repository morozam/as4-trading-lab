import { useEffect, useState } from 'react';
import { ArrowLeft, ArrowUpRight } from 'lucide-react';
import { dateTime, request } from './api';
import { Badge, Diagnostics, XmlViewer } from './Shared';
import type { AppState, Message } from './types';

function fields(xml:string) {
  const doc=new DOMParser().parseFromString(xml,'application/xml');
  const value=(name:string)=>Array.from(doc.getElementsByTagNameNS('*',name)).map(n=>n.textContent??'').filter(Boolean).join(', ');
  return [['Issuer',value('issuer_MarketParticipant.identification')],['Recipient',value('recipient_MarketParticipant.identification')],['Document code',value('documentCode')],['Validity period · UTC',value('validityPeriod')],['Counterparty account',value('externalAccount')],['Programme portfolios',Array.from(doc.getElementsByTagNameNS('*','Account')).map(a=>Array.from(a.children).find(n=>n.localName==='identification')?.textContent).filter(Boolean).join(', ')],['Referenced document',value('receiving_Document.identification')||value('nomination_Document.identification')||value('proDoc_Document.identification')],['Referenced version',value('receiving_Document.version')||value('nomination_Document.version')||value('proDoc_Document.version')]];
}
function metadata(value:unknown):[string,string][] {
  if(!value || typeof value!=='object') return [];
  return Object.entries(value).flatMap(([k,v]):[string,string][]=>v===null||v===undefined?[]:typeof v==='object'?metadata(v).map(([n,t])=>[`${k} / ${n}`,t]):[[k,String(v)]]);
}
export function MessageDetail({message:m,state,onBack,onProcess}:{message:Message;state:AppState;onBack:()=>void;onProcess:(id:string)=>void}) {
  const [tab,setTab]=useState('overview'); const [detail,setDetail]=useState<unknown>(null); const [error,setError]=useState('');
  useEffect(()=>{let active=true;request<unknown>(`/api/messages/${encodeURIComponent(m.id)}/details`).then(d=>{if(active)setDetail(d);}).catch(e=>{if(active)setError(e.message);});return()=>{active=false;};},[m.id,m.transport_status]);
  const process=state.processes.find(p=>p.id===m.process_id);
  return <section className="message-page"><button className="text-button" onClick={onBack}><ArrowLeft size={16}/> Back</button><div className="page-title"><div><span className="eyebrow">{m.direction==='in'?'Inbound':'Outbound'} message · {m.code}</span><h1>{m.family} <span className="heading-soft">/ version {m.version}</span></h1><p className="document-id">{m.document_id}</p></div><Badge>{m.transport_status}</Badge></div><div className="message-meta-strip"><span>{dateTime(m.at)}</span><span>AS4 ID <code>{m.id}</code></span>{process&&<button className="text-button" onClick={()=>onProcess(process.id)}>Open process <ArrowUpRight size={14}/></button>}</div><div className="view-tabs" role="tablist" aria-label="Message details">{['overview','xml','validation','transport','history'].map(t=><button role="tab" aria-selected={tab===t} className={tab===t?'active':''} key={t} onClick={()=>setTab(t)}>{({overview:'Overview',xml:'XML',validation:'Validation',transport:'AS4 transport',history:'Audit history'} as Record<string,string>)[t]}</button>)}</div>
    {tab==='overview'&&<><dl className="metadata-list">{fields(m.xml).filter(([,v])=>v).map(([k,v])=><div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}<div><dt>SHA-256</dt><dd><code>{m.hash}</code></dd></div></dl><button className="secondary" onClick={()=>setTab('xml')}>View complete XML</button></>}
    {tab==='xml'&&<XmlViewer xml={m.xml} messageId={m.id} name={`${m.family}-v${m.version}.xml`}/>}
    {tab==='validation'&&<Diagnostics report={m.validation}/>}
    {tab==='transport'&&<><p className="notice">An AS4 receipt confirms transport. Business acceptance and matching results are separate messages.</p>{error&&<p className="error" role="alert">{error}</p>}<dl className="metadata-list"><div><dt>Transport status</dt><dd>{m.transport_status}</dd></div><div><dt>AS4 message ID</dt><dd><code>{m.id}</code></dd></div>{metadata(detail&&typeof detail==='object'?Object.fromEntries(Object.entries(detail).filter(([k])=>['transport','outbox','callbacks'].includes(k))):null).filter(([k])=>!k.startsWith('message')).map(([k,v],i)=><div key={i}><dt>{k.replaceAll('_',' ')}</dt><dd><code>{v}</code></dd></div>)}</dl></>}
    {tab==='history'&&<section className="timeline"><ol>{process?.events.filter(e=>e.message_id===m.id).map(e=><li key={e.id}><span className="timeline-dot"/><div><strong>{e.title}</strong><p>{e.detail}</p><time>{dateTime(e.at)}</time></div></li>)}</ol>{!process?.events.some(e=>e.message_id===m.id)&&<p className="muted">Recorded {dateTime(m.at)}. No additional process events for this message.</p>}</section>}
  </section>;
}
