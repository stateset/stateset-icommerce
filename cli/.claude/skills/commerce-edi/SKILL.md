---
name: commerce-edi
description: Use when tracking EDI documents (850/855/856/810), managing their status lifecycle, or coordinating with the StateSet EDI gateway.
---

# Commerce EDI Skill

Domain knowledge for EDI document tracking: what the engine stores, the
status lifecycle, and how it relates to the transport gateway.

## Division of Responsibility

**This engine stores and tracks EDI documents. The separate `stateset-edi`
gateway handles transport** (AS2/SFTP/VAN, X12 parsing/generation, 997
functional acks, trading-partner onboarding). The engine's `ediDocuments`
surface is the system-of-record ledger of what was exchanged, its business
reference, and where it is in its lifecycle.

Typical integration: the gateway receives an 850, then creates an inbound
document record here (with the raw payload and PO reference); when the
engine ships, an outbound 856 record is created and handed to the gateway
to transmit, with status updated as transmission progresses.

## Document Types

| Type | Name | Direction (typical) | Business object |
|------|------|--------------------|-----------------|
| `850` | Purchase Order | inbound | Creates a sales order |
| `855` | PO Acknowledgment | outbound | Accept/reject the 850 |
| `856` | Advance Ship Notice (ASN) | outbound | Shipment + carton contents |
| `810` | Invoice | outbound | Bills the trading partner |

Other X12 types (860 changes, 940/945 warehouse, 204, etc.) can be stored —
`documentType` is a free string; these four are the core retail flow:

```
partner 850 ──► [engine order] ──► 855 ack ──► 856 ASN ──► 810 invoice
```

## Status Lifecycle

```
[pending] ──► [sent] ──► [acknowledged] ──► [processed]
     │           │              │
     └───────────┴──────────────┴──────► [error] (with errorMessage)
```

| Status | Meaning |
|--------|---------|
| `pending` | Recorded, not yet transmitted/handled |
| `sent` | Handed to the gateway / transmitted |
| `acknowledged` | Partner functional ack (997) received |
| `processed` | Business processing complete (order created, invoice booked) |
| `error` | Failed — `errorMessage` carries the failure detail |

Direction is `inbound` or `outbound` (create defaults to inbound).
For inbound documents, `pending → processed` is the common path; the
`sent`/`acknowledged` steps mostly apply to outbound documents.

## Tool Surface (`edi_documents` domain)

- `list_edi_documents` — filter by `documentType` ("850"), `direction`,
  `status`, `partner`, with limit/offset pagination
- `get_edi_document` — full record by ID, including payload
- `create_edi_document` — record a document: `documentType` (required),
  `direction`, `partner`, `reference` (PO/order number), `payload`
  (raw EDI) — requires `--apply`
- `set_edi_document_status` — move through the lifecycle; pass
  `errorMessage` (max 2000 chars) when setting `error` — requires `--apply`
- `get_edi_summary` — counts by status and type (per partner) for
  operational dashboards

The same surface is exposed as `commerce.ediDocuments` on the
`@stateset/embedded` Node binding, and the admin app has a read-only
**`/operations/edi`** page showing 850/855/856/810 summaries and status
tracking.

## Common Workflows

### Ingest an Inbound 850
```
1. create_edi_document({ documentType: "850", direction: "inbound",
     partner: "RETAILER-A", reference: "PO-88231", payload: "<raw X12>" })
2. [create the sales order from the PO data]
3. set_edi_document_status(id, "processed")
   — or "error" with an errorMessage if parsing/order creation failed
```

### Send an Outbound 856 (ASN)
```
1. Ship the order (shipment created, cartons packed)
2. create_edi_document({ documentType: "856", direction: "outbound",
     partner, reference: shipmentNumber })   # status: pending
3. Gateway transmits            ──► set_edi_document_status(id, "sent")
4. 997 received from partner    ──► set_edi_document_status(id, "acknowledged")
```

### Triage Errors
```
1. list_edi_documents({ status: "error" })
2. get_edi_document(id) — read errorMessage and payload
3. Fix the root cause (bad reference, partner mapping, transport)
4. Re-create or re-transmit via the gateway; update status accordingly
```

### Daily Operations Review
```
1. get_edi_summary — counts by status/type per partner
2. list_edi_documents({ status: "pending", direction: "outbound" })
   — anything stuck untransmitted?
3. Check /operations/edi in the admin for the same view
```

## Best Practices

1. **Always set `reference`** — linking to the PO/order/shipment number is
   what makes documents findable during disputes
2. **Store the raw payload** — the original X12 is the audit artifact
3. **Record errors with detail** — a bare `error` status without
   `errorMessage` is undiagnosable later
4. **Watch pending outbound** — a growing pending queue means the gateway
   handoff is broken
5. **Track the full chain** — every 850 should end with a processed 810;
   `get_edi_summary` gaps reveal missed invoices
6. **Transport questions go to the gateway** — AS2 certs, 997 generation,
   partner onboarding, and X12 syntax are `stateset-edi` gateway concerns,
   not engine document records

## Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `EDI document not found` | Bad ID | `list_edi_documents` to locate it |
| `Apply required` | Write without `--apply` | Re-run with `--apply` |
| Document stuck `pending` | Gateway handoff failed | Check gateway transport, retransmit |
| Partner rejects 856 | Content/mapping issue | Inspect payload; fix mapping in the gateway |
