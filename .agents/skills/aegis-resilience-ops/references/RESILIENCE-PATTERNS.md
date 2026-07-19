# Ferris Aegis — Resilience Patterns Reference

> Loaded on demand by the aegis-resilience-ops skill.

## Circuit Breaker State Machine

```
                 N failures
   ┌────────┐ ─────────────▶ ┌────────┐
   │ Closed │                │  Open   │
   └────┬───┘ ◀───────────── └────┬───┘
        │      M successes        │ timeout
        │      (via HalfOpen)     │ elapsed
        │                        ▼
        │                  ┌──────────┐
        └─────────────────▶│ HalfOpen │◀── probe request
                           └──────────┘
                                │ failure
                                ▼
                           ┌────────┐
                           │  Open  │
                           └────────┘
```

## Retry Backoff Formula

```
delay = base_delay_ms * 2^attempt
if use_jitter:
    delay = delay * random(0.75, 1.25)  // ±25%
delay = min(delay, max_delay_ms)
```

## Rate Limiter: Token Bucket

```
Capacity: max tokens in bucket
Refill: refill_rate tokens per refill_interval_ms

on try_acquire():
    if tokens >= 1:
        tokens -= 1
        return true
    else:
        return false

on refill():
    tokens = min(capacity, tokens + refill_rate * elapsed_intervals)
```

## Composite: execute_resilient()

Layers from outside in:
1. Circuit Breaker — gate (reject if open)
2. Timeout — deadline enforcement
3. Retry + Backoff — transient failure recovery
4. Operation — actual work

## Default Configurations

| Primitive | Default | Reasoning |
|-----------|---------|-----------|
| Circuit Breaker | 5 failures, 30s recovery, 2 half-open successes | Conservative for production |
| Retry | 3 retries, 100ms base, 10s max, jitter on | Standard backoff |
| Timeout | None (0) | Must be set per-operation |
| Rate Limiter | 100 capacity, 10/s refill | Moderate throughput |
