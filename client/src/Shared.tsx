import { useMemo, useState } from 'react';
import { Check, Copy, Download, FileCode2, LoaderCircle } from 'lucide-react';
import type { ReactNode } from 'react';
import type { Validation } from './types';
import { xmlCategories, xmlParts } from './xmlHighlights';
import type { XmlPart } from './xmlHighlights';

export function Busy({children}: {children: ReactNode}) { return <><LoaderCircle className="spinner" size={15}/>{children}</>; }
export function Empty({title, children}: {title: string; children?: ReactNode}) { return <div className="empty"><FileCode2 size={28} strokeWidth={1.3}/><h3>{title}</h3><p>{children}</p></div>; }
export function Badge({children}: {children: ReactNode}) {
  const value = String(children);
  const tone = /unknown|await|queued|missing|no counter|unchanged|mismatch|expiring/i.test(value) ? 'warn' : /reject|fail|error|expired/i.test(value) ? 'bad' : /accepted|settled|confirmed|pass|receipt received|delivered|exact/i.test(value) ? 'good' : '';
  return <span className={`badge ${tone}`}>{children}</span>;
}
export function Diagnostics({report}: {report: Validation}) {
  return <section className="diagnostics"><div className="section-head"><h3>Layered validation</h3><Badge>{report.valid ? 'Checks passed' : 'Validation failed'}</Badge></div><p className="muted small">Pinned schema and implemented local rules. Standalone validation does not establish process context or production validity.</p><div className="checks">{report.checks.map((check, i) => <div key={i}><Badge>{check.status.replaceAll('_', ' ')}</Badge><strong>{check.layer}</strong><span>{check.detail}</span></div>)}</div>{report.errors.length > 0 && <ul className="validation-errors">{report.errors.map((error, i) => <li key={i}>{error}</li>)}</ul>}</section>;
}
export function XmlViewer({xml, messageId, name = 'edig-as.xml'}: {xml: string; messageId?: string; name?: string}) {
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState('');
  async function copy() { try { await navigator.clipboard.writeText(xml); setCopied(true); setTimeout(() => setCopied(false), 1800); } catch { setCopyError('Clipboard unavailable. Select and copy the XML below.'); } }
  function download() { const url = URL.createObjectURL(new Blob([xml], {type:'application/xml'})); const anchor = document.createElement('a'); anchor.href = url; anchor.download = name; anchor.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); }
  const [highlighting, setHighlighting] = useState(true);
  const parts = useMemo(() => xmlParts(xml), [xml]);
  const categories = xmlCategories.filter(category => parts.some(part => part.category === category.id));
  const lines = useMemo(() => {
    const rows: XmlPart[][] = [[]];
    for (const part of parts) part.text.split('\n').forEach((text, i) => {
      if (i) rows.push([]);
      rows[rows.length - 1].push({ ...part, text });
    });
    return rows;
  }, [parts]);
  return <section className="xml-viewer">
    <div className="section-head"><h3>Exact XML</h3><div className="actions">
      <button className="text-button" onClick={copy}>{copied ? <Check size={14}/> : <Copy size={14}/>} {copied ? 'Copied' : 'Copy'}</button>
      {messageId ? <a className="text-button" href={`/api/messages/${encodeURIComponent(messageId)}/xml?download=1`} download><Download size={14}/> Download</a> : <button className="text-button" onClick={download}><Download size={14}/> Download</button>}
    </div></div>
    <div className="xml-highlight-toolbar">
      <label className="xml-highlight-toggle"><input type="checkbox" checked={highlighting} onChange={event => setHighlighting(event.target.checked)}/> Highlight key fields</label>
      {highlighting && <div className="xml-field-legend" aria-label="XML highlight legend" role="list">{categories.map(category => <span role="listitem" key={category.id}><mark className={`xml-mark-${category.id}`}>{category.label}</mark></span>)}</div>}
    </div>
    {copyError && <p role="alert" className="error">{copyError}</p>}
    <div className="xml-scroll" tabIndex={0} aria-label="XML document with line numbers"><pre>{lines.map((line, i) => <span className="xml-line" key={i}><span aria-hidden="true" className="line-number">{i + 1}</span><code>{line.map((part, j) => highlighting && part.category ? <mark key={j} className={`xml-mark-${part.category}`} data-category={part.category} title={part.label}>{part.text}</mark> : <span key={j}>{part.text}</span>)}</code></span>)}</pre></div>
  </section>;
}
