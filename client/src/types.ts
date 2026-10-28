export type Role = 'brp' | 'tso';
export interface Session { role: Role; user: string; csrf: string }
export interface Info { role: Role; environment: string; peer_url: string }
export interface Hour { start: string; end: string; label: string }
export interface Validation { valid: boolean; checks: {layer: string; status: string; detail: string}[]; errors: string[] }
export interface DraftInput { kind: string; gas_day: string; counterparty: string; direction: string; quantities: number[]; document_id?: string | null; version: number }
export interface Draft { id: string; input: DraftInput; updated_at: string; review_hash?: string | null }
export interface Preview { xml: string; hash: string; validation: Validation; draft: Draft }
export interface Process { confirmation_origin?:string|null; confirmation_document_id?:string|null; confirmation_version?:number|null; confirmation_accepted?:boolean|null; confirmation_reasons?:string[]; nomination_profiles?:NominationProfile[]; programme_profiles?:ProgrammeProfile[]; programme_origin?: string | null; balancing?: BalancingDetail | null; id: string; document_id: string; version: number; kind: string; gas_day: string; counterparty: string; direction: string; quantities: number[]; confirmed: number[]; counterparty_quantities: number[]; matching_statuses: string[]; stage: string; handoff_status: string; transport_status: string; ack_status: string; validation: Validation; events: {id: string; kind: string; title: string; detail: string; at: string; message_id?: string | null}[]; created_at: string; source_message_id: string }
export interface Message { id: string; family: string; code: string; direction: string; document_id: string; version: number; process_id?: string | null; at: string; transport_status: string; hash: string; xml: string; validation: Validation }
export interface AppState extends Info { user: string; connection: {gateway_url: string; queued: number; unknown: number; last_callback: string | null; gateway_reachable: boolean}; drafts: Draft[]; processes: Process[]; messages: Message[]; fixtures: Record<string, number[] | null> }
export type Command = <T>(path: string, body?: unknown, method?: string) => Promise<T>;

export interface BalancingPeriod {start: string; end: string; direction: string; quantity: number; price: string}
export interface BalancingTransaction {identification: string; periods: BalancingPeriod[]}
export interface BalancingInput {gas_day:string;document_code:string;document_id?:string | null;version:number;transactions:BalancingTransaction[]}
export interface BalancingDetail {document_code?:string;transactions:BalancingTransaction[];[key:string]:unknown}
export interface BalancingPreview {id:string;input:BalancingInput;xml:string;hash:string;validation:Validation}
export interface MessageDetails {message:Message; [key:string]:unknown}
export interface Certificate {alias:string;store:string;purpose:string;subject:string;issuer:string;serial:string;not_before:string;not_after:string;fingerprint:string;status:string;key_algorithm:string;key_usage:string;extended_key_usage:string;basic_constraints:string;pmodes:string[];chain_length:number}
export interface PMode {id:string;direction:string;initiator:string;responder:string;service:string;action:string;agreement:string;address:string;signing:string;encryption:string;receipt:string;signing_aliases:string[];mep:string;binding:string}
export interface GatewayInventory {read_at:string;loaded_status:string;certificates:Certificate[];pmodes:PMode[];errors:string[]}

export interface NominationProfile {internal_account:string;point:string;point_scheme:string;nomination_code?:string|null;counterparty:string;direction:string;directions:string[];quantities:number[];statuses:string[];status_codes:string[][];counterparty_quantities:number[];counterparty_directions?:string[]}
export interface ProgrammeProfile {account:string;account_code:string;quantities:number[];reason_codes:string[][];reason_texts:string[][]}
