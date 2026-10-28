export type XmlCategory = 'counterparty' | 'point' | 'identity' | 'quantity' | 'time' | 'document';
export interface XmlPart { text: string; category?: XmlCategory; label?: string }
export const xmlCategories: { id: XmlCategory; label: string }[] = [
  { id: 'counterparty', label: 'Counterparty / portfolio' },
  { id: 'point', label: 'Point' },
  { id: 'identity', label: 'EIC / participant' },
  { id: 'quantity', label: 'Quantities / prices' },
  { id: 'time', label: 'Periods / times' },
  { id: 'document', label: 'Document / codes' },
];

function field(stack: string[]): { category: XmlCategory; label: string } | undefined {
  const leaf = stack.at(-1) ?? '';
  const parent = stack.at(-2) ?? '';
  const programme = stack[0]?.startsWith('LoadForecast');
  if (leaf === 'externalAccount' && parent === 'External_Account') return { category: 'counterparty', label: 'Counterparty account' };
  if (leaf === 'identification' && parent === 'Account') return programme
    ? { category: 'counterparty', label: 'Programme portfolio' }
    : { category: 'identity', label: 'Own portfolio account' };
  if (leaf === 'identification' && ['ConnectionPoint', 'MarketArea'].includes(parent)) return { category: 'point', label: parent === 'MarketArea' ? 'Market area' : 'Connection point' };
  if (leaf === 'issuer_MarketParticipant.identification') return { category: 'identity', label: 'Issuer EIC / participant ID' };
  if (leaf === 'recipient_MarketParticipant.identification') return { category: 'identity', label: 'Recipient EIC / participant ID' };
  if (leaf === 'internalAccount') return { category: 'identity', label: 'Own portfolio account' };
  if (leaf === 'contract_Reference.identification') return { category: 'identity', label: 'Contract / portfolio reference' };
  if (leaf.endsWith('roleCode')) return { category: 'identity', label: 'Market participant role' };
  if (leaf === 'price.amount') return { category: 'quantity', label: 'Price · EUR/kWh' };
  if (['quantity.amount', 'accountDirection.account_Quantity.amount'].includes(leaf)) return { category: 'quantity', label: 'Quantity · kWh/h' };
  if (leaf === 'measureUnit.unitOfMeasureCode') return { category: 'quantity', label: 'Quantity unit code' };
  if (leaf === 'currency.currencyCode') return { category: 'quantity', label: 'Price currency' };
  if (['validityPeriod', 'timeInterval', 'creationDateTime'].includes(leaf)) return { category: 'time', label: ({ validityPeriod: 'Document validity · UTC', timeInterval: 'Delivery period · UTC', creationDateTime: 'Created at · UTC' } as Record<string, string>)[leaf] };
  if (leaf === 'identification' && (parent.endsWith('_Document') || stack.length === 2)) return { category: 'document', label: 'Document identifier' };
  if (leaf === 'identification' && parent === 'Transaction') return { category: 'document', label: 'Transaction identifier' };
  if (leaf === 'version') return { category: 'document', label: 'Document version' };
  if (['receiving_Document', 'nomination_Document', 'proDoc_Document'].some(prefix => leaf.startsWith(prefix + '.'))) return { category: 'document', label: 'Referenced document · ' + leaf.split('.').at(-1) };
  if (['documentCode', 'reasonCode', 'nominationCode', 'businessCode', 'accountCode', 'contract_Reference.referenceCode', 'direction.gasDirectionCode', 'accountDirection.accountDirectionCode', 'status.statusCode'].includes(leaf)) return { category: 'document', label: ({ documentCode: 'Document type', reasonCode: 'Acknowledgement / confirmation reason', 'direction.gasDirectionCode': 'Gas direction · Z02 entry / Z03 exit', 'accountDirection.accountDirectionCode': 'Allocation direction · ZPE credit / ZPD debit', 'status.statusCode': 'Status code', businessCode: 'Business series code', nominationCode: 'Nomination type', accountCode: 'Portfolio type' } as Record<string, string>)[leaf] ?? 'Reference code' };
}

// Display tokens only: never parse/re-serialise the authoritative XML or interpret entities.
export function xmlParts(xml: string): XmlPart[] {
  const parts: XmlPart[] = [];
  const stack: string[] = [];
  let pos = 0;
  function append(text: string, description?: { category: XmlCategory; label: string }) {
    if (!text) return;
    if (!description || !text.trim()) { parts.push({ text }); return; }
    const [, before, value, after] = text.match(/^(\s*)([\s\S]*?)(\s*)$/)!;
    if (before) parts.push({ text: before });
    if (value) parts.push({ text: value, ...description });
    if (after) parts.push({ text: after });
  }
  // Quote-aware tag matching keeps attribute values containing '>' intact.
  const tags = /<!--[\s\S]*?-->|<!\[CDATA\[[\s\S]*?\]\]>|<\?[\s\S]*?\?>|<(?:"[^"]*"|'[^']*'|[^'">])*>/g;
  for (const match of xml.matchAll(tags)) {
    append(xml.slice(pos, match.index), field(stack));
    const tag = match[0];
    if (tag.startsWith('<![CDATA[')) {
      parts.push({ text: '<![CDATA[' });
      append(tag.slice(9, -3), field(stack));
      parts.push({ text: ']]>' });
    } else {
      parts.push({ text: tag });
      if (!/^<[!?]/.test(tag)) {
        const name = tag.match(/^<\/?(?:[\w.-]+:)?([\w.-]+)/)?.[1];
        if (tag.startsWith('</')) { if (stack.at(-1) === name) stack.pop(); }
        else if (name && !/\/\s*>$/.test(tag)) stack.push(name);
      }
    }
    pos = match.index! + tag.length;
  }
  append(xml.slice(pos), field(stack));
  return parts;
}
