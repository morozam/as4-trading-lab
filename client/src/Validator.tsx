import { useState } from 'react';
import { FileCheck2 } from 'lucide-react';
import { Busy, Diagnostics } from './Shared';
import type { Command, Validation } from './types';

export function Validator({command}: {command: Command}) {
  const [xml, setXml] = useState(''); const [result, setResult] = useState<Validation | null>(null); const [busy, setBusy] = useState(false); const [error, setError] = useState('');
  async function validate() {setBusy(true); setError(''); setResult(null); try {setResult(await command<Validation>('/api/validate', {xml}));} catch(err) {setError(err instanceof Error ? err.message : String(err));} finally {setBusy(false);}}
  return <><div className="page-title"><div><span className="eyebrow">Standalone inspection</span><h1>XML validator</h1><p>Inspect supported Edig@s XML without creating or sending a business message.</p></div></div><label className="xml-input">XML document<textarea spellCheck={false} value={xml} onChange={e => {setXml(e.target.value); setResult(null);}} placeholder={'<?xml version="1.0" encoding="UTF-8"?>\nPaste an Edig@s document here…'}/></label><div className="editor-actions"><button className="primary" disabled={busy || !xml.trim()} onClick={validate}>{busy ? <Busy>Validating…</Busy> : <><FileCheck2 size={16}/> Validate XML</>}</button><span className="muted small">No business side effects · pinned local schemas</span></div>{error && <p className="error" role="alert">{error}</p>}{result && <Diagnostics report={result}/>}</>;
}
