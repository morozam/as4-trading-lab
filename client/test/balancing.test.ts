import { test } from 'node:test';
import assert from 'node:assert/strict';
import { allocationTransactions } from '../src/balancing.ts';
const hours=Array.from({length:4},(_,i)=>({start:`2026-10-27T0${i+5}:00:00Z`,end:`2026-10-27T0${i+6}:00:00Z`,label:`${i}`}));
test('fills only unallocated hours with NO_TRANS, retaining overlapping independent transactions',()=>{
 const transactions=allocationTransactions([{id:'TR000001',first:1,last:2,direction:'debit',quantity:'10',price:'0.030000'},{id:'TR000002',first:2,last:2,direction:'credit',quantity:'5',price:'0.040000'}],hours);
 assert.equal(transactions.length,4);
 assert.deepEqual(transactions.filter(t=>t.identification==='NO_TRANS').map(t=>[t.periods[0].start,t.periods[0].end]),[[hours[0].start,hours[0].end],[hours[3].start,hours[3].end]]);
});
