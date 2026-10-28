# Test the three processes

[Back to the main guide](../README.md)

Open the BRP and TSO apps in two browser tabs, or give the TSO tab to a colleague.
Each side has its own login and message history.
All examples below are tests. No message goes to GTS.

## Before you send

- Pick a gas day. It runs from 06:00 to 06:00 in Amsterdam time.
- The app gives you 23, 24 or 25 hours when the clocks change.
- Enter quantities in **kWh/h**. For example, 10 MWh/h means **10000 kWh/h**.
- Use the same hourly shape when you compare a buy and a sell.
- Preview and validate the XML. Then send it.

Changing a draft after the preview means you must preview it again.
The app sends the exact XML that you reviewed.

## 1. Nomination

A nomination tells the TSO what you want to transfer at TTF.

```text
BRP: NOMINT → TSO: ACKNOW → TSO: NOMRES → BRP reads the result
```

1. In the BRP workspace, click **New submission**.
2. Choose **NOMINT**, a gas day, buy or sell, and the test counterparty.
3. Fill the hourly amounts. Use **Apply to all**, a range, or paste a column from a spreadsheet.
4. Click **Preview & validate XML**, then **Send NOMINT**.
5. In the TSO workspace, open the new request.
6. Click **Accept · Send ACKNOW**.
7. Under **Counterparty test fixture**, set the other side's amounts and apply them.
8. Click **Send NOMRES**.
9. On the BRP side, open the process and read the confirmed hourly amounts.

The test fixture stands in for the other trading company's nomination.
There is no third trading-company user in this app.

| Test | What to try |
|---|---|
| Match | Give both sides the same amounts |
| Mismatch | Give the other side a smaller amount |
| Missing counter-nomination | Choose the missing-counterparty case |
| Rejection | TSO enters a reason and clicks **Reject request** |
| Correction | BRP uses **Correct & resubmit · v2**, edits, previews and sends again |

A rejected request does not need a later NOMRES in this lab.
The corrected request keeps its document ID and uses a higher version.
The old version stays available.
A change to an already settled nomination can leave the old settlement in place.
Always read the returned result and amounts.

## 2. Trade programme

The lab's trade programme sends a zero profile for `GSTPTRADE`.

```text
BRP: PRODOC → TSO: ACKNOW → TSO: PROCON → BRP reads the result
```

1. BRP clicks **New submission** and chooses **PRODOC**.
2. Pick the gas day. The profile is zero in every hour.
3. Preview, validate and send it.
4. TSO opens it and sends an accepted ACKNOW.
5. TSO clicks **Send PROCON**.
6. BRP checks the returned programme.

The test confirmation includes `GSTPTRADE`, `GSTPD`, `GSTPVPPVEN` and
`GSTPVPPVEX`, all with zero amounts.
TSO can also use **Default programme** to send a PROCON without a prior PRODOC.
That process has no made-up request or ACKNOW step.

A zero trade programme is not proof that your actual trades are balanced.
This app does not import screen trades or calculate a live position.

## 3. Balancing allocation

This is a notice from the TSO about allocated balancing gas and its price.
It is not an order that the BRP submits.

```text
TSO: MARSIT → BRP reads the allocation
```

1. TSO clicks **New balancing allocation**.
2. Pick the gas day and normal or emergency allocation type.
3. Fill the transaction, period, direction, quantity and price.
4. Preview, validate and send MARSIT.
5. BRP opens the new balancing process and reads the allocation.

Prices use **EUR/kWh**. `0.035000` EUR/kWh means 35 EUR/MWh.
Debit means gas out of the BRP portfolio. Credit means gas into it.
`NO_TRANS` covers hours without an allocated transaction.
A higher version replaces the earlier allocation; do not add both versions together.
This lab does not settle invoices or calculate GTS imbalance charges.

## Read a process or message

The vertical process line shows each expected step.
Click a step to see its message. Click **Full message details** for more detail.
Open the **XML** tab to see, copy or download the exact document.

| Highlight | Meaning |
|---|---|
| Yellow | Counterparty or portfolio |
| Blue | Point |
| Purple | Party or EIC identity |
| Green | Quantity, price or unit |
| Peach | Dates and times |
| Grey | Document IDs, references or status |

Highlights do not change the XML. You can turn them off.
The **AS4 transport** tab shows gateway IDs and delivery events.

Three different things can happen:

1. **AS4 receipt:** the other gateway received the message.
2. **ACKNOW:** the other business app accepted or rejected the request.
3. **NOMRES / PROCON:** the business result.

A receipt alone does not mean your nomination was matched.
If a response has warnings, unknown codes or no matching local request,
read its details before treating the process as finished.

## Validate XML without sending it

Open **XML validator**, paste a document, then click **Validate XML**.
This does not send a message.
It checks the supported message families against local Edig@s 6.1 schemas
and the rules coded for this lab. It is not a GTS approval test.

## Inspect certificates and routes

Open **Trust & routing**. Choose a certificate or the **P-Modes** tab.
These pages show local settings and certificate details. They do not edit them.
A P-Mode file on disk alone does not prove that a live exchange succeeded.
