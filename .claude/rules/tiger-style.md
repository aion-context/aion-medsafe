---
paths:
  - "system/src/**/*.rs"
  - "system/tests/**/*.rs"
  - "system/Cargo.toml"
---

# Tiger Style — Zero Tolerance Engineering

## Scope
All Rust code in `system/`.

## Rules

### No Panics
- `unwrap()` is FORBIDDEN in production code
- `expect()` is FORBIDDEN in production code
- `panic!()` is FORBIDDEN
- `unreachable!()` is FORBIDDEN
- `todo!()` is FORBIDDEN
- Every fallible path returns `Result<T, E>`
- Use `?` propagation, never implicit unwinding

### No Unsafe
- `unsafe` blocks require explicit justification and are a review flag
- Prefer safe abstractions from vetted crates

### Bounded Resource Usage
- No unbounded allocations (`Vec::new()` is fine; unbounded `.collect()` from untrusted input is not)
- Cap loop iterations where input is external
- All file reads must check size before allocation

### Deterministic Behavior
- No hidden side effects
- Isolate I/O from pure logic
- Prefer explicit error returns over implicit state mutation
- Functions should be total where possible

### Observability
- Every significant operation emits a `tracing::info!` event
- Events use bounded field names (fixed vocabulary)
- Log cardinality must remain tractable (no per-record logging in hot paths)
- Use structured fields: `event`, `file_id`, `author`, `reason`

### Error Messages
- Errors must be actionable: tell the operator what went wrong AND what to do
- Include relevant context (file paths, IDs, sizes)
- Never expose internal state to end users

## Enforcement
These are enforced via `[lints.clippy]` in `Cargo.toml`:
```toml
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unreachable = "deny"
```
