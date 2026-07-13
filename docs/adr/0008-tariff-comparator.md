# ADR-0008: Versioned power-adjusted cost comparator

- Status: Accepted as a reporting method; not a payment oracle
- Date: 2026-07-12
- Owners: Economics and operations owners

## Decision

Provider economics separates measured execution, estimated energy, tariff assumption, projected cost and projected margin. The marketplace does not pay solely for claimed GPU time, and the comparator does not modify accepted quote or settlement values.

Each run records wall time, active/idle device time where available, sampled or vendor-reported power, host overhead assumption, storage and transfer bytes, region/currency, tariff source, capture timestamp, tax treatment and pricing-policy version. Measured values and estimates are labelled separately. Missing hardware power telemetry uses a conservative declared estimate and receives lower evidence quality.

Electricity cost uses integer arithmetic over milliwatt-milliseconds and tariff micros per kWh, rounding the final cost upward. Time-of-use tariffs split a run across applicable intervals. Currency conversion, if used, is a separately versioned input captured at comparison time; the original tariff and currency remain in the report.

Hosted alternatives are captured from a public provider price page or API with product/SKU, region, commitment class, included resources, billing quantum, timestamp, currency and source URL/hash. Spot and reserved prices are never compared with on-demand prices without a visible label. A stale capture older than 30 days cannot support a go/no-go claim.

The pilot publishes p50/p95 provider cost and margin by hardware/backend and compares accepted useful stages, not raw claimed GPU hours. Quality, retry and verification costs remain visible.
