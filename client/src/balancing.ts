import type { BalancingTransaction, Hour } from './types';
export interface AllocationRow {id:string;first:number;last:number;direction:string;quantity:string;price:string}
export function allocationTransactions(rows:AllocationRow[],hours:Hour[]):BalancingTransaction[] {
 const covered=hours.map(()=>false);
 const transactions:BalancingTransaction[]=rows.map(r=>{
  if(!Number.isSafeInteger(Number(r.quantity))||!r.quantity.trim()||Number(r.quantity)<0)throw new Error('Allocation quantities must be non-negative whole kWh/h.');
  if(!hours[r.first]||!hours[r.last]||r.last<r.first)throw new Error('Choose a valid allocation interval.');
  for(let i=r.first;i<=r.last;i++)covered[i]=true;
  return {identification:r.id,periods:[{start:hours[r.first].start,end:hours[r.last].end,direction:r.direction,quantity:Number(r.quantity),price:r.price}]};
 });
 for(let i=0;i<hours.length;i++){if(covered[i])continue;const start=i;while(i+1<hours.length&&!covered[i+1])i++;transactions.push({identification:'NO_TRANS',periods:[{start:hours[start].start,end:hours[i].end,direction:'debit',quantity:0,price:'0.000000'}]});}
 return transactions;
}
