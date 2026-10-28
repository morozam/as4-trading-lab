import { test } from 'node:test';
import assert from 'node:assert/strict';
import { xmlParts } from '../src/xmlHighlights.ts';

test('distinguishes document, party, point, counterparty, quantity, time and code values by their XML path',()=>{
 const xml='<Nomination_Document><identification>NOM-1</identification><version>2</version><documentCode>02G</documentCode><issuer_MarketParticipant.identification codingScheme="305">99X-LAB-BRP----0</issuer_MarketParticipant.identification><recipient_MarketParticipant.identification codingScheme="305">99X-LAB-TSO----0</recipient_MarketParticipant.identification><Internal_Account><internalAccount codingScheme="ZSO">GSLABBRP</internalAccount><ConnectionPoint><identification codingScheme="305">21YNL----TTF---1</identification><NominationType><External_Account><externalAccount codingScheme="ZSO">GSLABCOUNTER</externalAccount><Period><timeInterval>2026-10-27T05:00Z/2026-10-27T06:00Z</timeInterval><direction.gasDirectionCode>Z03</direction.gasDirectionCode><quantity.amount>10000</quantity.amount></Period></External_Account></NominationType></ConnectionPoint></Internal_Account></Nomination_Document>';
 const parts=xmlParts(xml);assert.equal(parts.map(p=>p.text).join(''),xml);
 const category=(value:string)=>parts.find(p=>p.text===value)?.category;
 assert.equal(category('NOM-1'),'document');assert.equal(category('2'),'document');assert.equal(category('99X-LAB-BRP----0'),'identity');assert.equal(category('99X-LAB-TSO----0'),'identity');assert.equal(category('GSLABBRP'),'identity');assert.equal(category('21YNL----TTF---1'),'point');assert.equal(category('GSLABCOUNTER'),'counterparty');assert.equal(category('10000'),'quantity');assert.equal(category('2026-10-27T05:00Z/2026-10-27T06:00Z'),'time');assert.equal(category('Z03'),'document');
});
test('handles namespace prefixes, quoted angle brackets, multiline text and comments without changing exact XML',()=>{
 const xml='<?xml version="1.0"?>\r\n<x:LoadForecast_Document xmlns:x="urn:test"><!-- <quantity.amount>999</quantity.amount> --><x:ConnectionPoint><x:identification label="a > b"> VPPV </x:identification><x:Account><x:identification>GSTPTRADE</x:identification><x:quantity.amount>\n 0\n</x:quantity.amount></x:Account></x:ConnectionPoint></x:LoadForecast_Document>';
 const parts=xmlParts(xml);assert.equal(parts.map(p=>p.text).join(''),xml);assert.equal(parts.find(p=>p.text==='GSTPTRADE')?.category,'counterparty');assert.equal(parts.find(p=>p.text==='VPPV')?.category,'point');assert.equal(parts.find(p=>p.text==='0')?.category,'quantity');assert.ok(!parts.some(p=>p.text==='999'&&p.category));
});
test('keeps balancing own portfolio separate from a counterparty and labels prices and reference versions',()=>{
 const xml='<MarketSituation_Document><identification>MAR1</identification><MarketArea><identification>BVP</identification><Account><identification>GSLABBRP</identification><price.amount>-0.035000</price.amount><accountDirection.account_Quantity.amount>2500</accountDirection.account_Quantity.amount></Account></MarketArea><proDoc_Document.version>3</proDoc_Document.version></MarketSituation_Document>';
 const parts=xmlParts(xml);assert.equal(parts.find(p=>p.text==='GSLABBRP')?.category,'identity');assert.equal(parts.find(p=>p.text==='-0.035000')?.category,'quantity');assert.match(parts.find(p=>p.text==='-0.035000')?.label??'',/price/i);assert.equal(parts.find(p=>p.text==='3')?.category,'document');
});
