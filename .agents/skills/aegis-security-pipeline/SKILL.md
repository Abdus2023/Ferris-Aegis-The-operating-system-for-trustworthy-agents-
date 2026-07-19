---
name: aegis-security-pipeline
description: >
  Runs the Ferris Aegis security pipeline: tool allowlist checks, injection scanning,
  SSRF guard, credential vault, and WASM sandbox operations. Use when the user says
  "injection scan", "SSRF check", "credential vault", "tool allowlist", "WASM sandbox",
  "ProtectedSecret", or "security pipeline". Do NOT use for trust scoring or policy rules.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+, ferris-aegis-security, ferris-aegis-sandbox-wasm crates
metadata:
  aegis-crate: "ferris-aegis-security ferris-aegis-sandbox-wasm"
  aegis-phase: "3"
  aegis-invariants: "INV-001 INV-002 INV-003 INV-005 INV-007 INV-008 INV-009"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "security injection ssrf allowlist sandbox credential"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — Security Pipeline

Run the full security pipeline: allowlist → injection scan → SSRF guard → credential vault → WASM sandbox.

## When to Use

- Checking if a tool is on the allowlist
- Scanning user input for prompt injection patterns
- Validating URLs against SSRF attacks
- Managing credentials with ProtectedSecret
- Running code in a WASM sandbox

## Security Pipeline Order

```
User Input → [1. Allowlist] → [2. Injection Scan] → [3. SSRF Guard]
                                                      ↓
Tool Call → [4. Credential Vault (ProtectedSecret)] → [5. Execute]
                                                      ↓
WASM Code → [6. Sandbox (fuel + memory + epoch)]  → Complete
```

## Workflow

1. **Allowlist check** (INV-005): Deny-by-default. Only `file_read`, `web_search`, etc. are allowed.
2. **Injection scan**: 11 regex patterns, 9 attack categories. Returns `Clean` or `Suspicious`.
3. **SSRF guard** (INV-008): Blocks private IPs, cloud metadata endpoints.
4. **Credential vault** (INV-001, INV-002, INV-003): `ProtectedSecret` never serializes, never traces.
5. **WASM sandbox** (INV-007): Fuel metering, memory cap, epoch interruption.

## Code Pattern — Full Pipeline

```rust
use ferris_aegis_security::{
    ToolAllowlist, AllowlistVerdict,
    InjectionScanner, InjectionVerdict,
    SsrfGuard, SsrfVerdict,
    CredentialVault, ToolCall, ProtectedSecret,
};

// 1. Allowlist
let allowlist = ToolAllowlist::default_safe();
if allowlist.check("unknown_tool") == AllowlistVerdict::Denied {
    return Err("Tool not on allowlist");
}

// 2. Injection scan
let scanner = InjectionScanner::new();
let verdict = scanner.scan(user_input);
if !verdict.is_clean() {
    return Err("Injection pattern detected");
}

// 3. SSRF guard
let ssrf = SsrfGuard::new();
let verdict = ssrf.check_url(&url);
if !verdict.is_safe() { return Err("SSRF blocked"); }

// 4. Credential vault — ProtectedSecret never serializes
let call = ToolCall::new("http_request", serde_json::json!({"url": url}));
let auth_call = call.with_credential(ProtectedSecret::new("sk-key"));
// auth_call.call is safe to trace — no credential in it
// auth_call.credential is ProtectedSecret — cannot serialize
```

## Code Pattern — WASM Sandbox

```rust
use ferris_aegis_sandbox_wasm::{WasmSandbox, WasmSandboxConfig, infinite_loop_wasm};

let config = WasmSandboxConfig { max_fuel: 10_000_000, ..Default::default() };
let sandbox = WasmSandbox::new(config).unwrap();
let module = sandbox.compile_module(&wasm_bytes).unwrap();
let result = sandbox.execute(&module, "run").unwrap();
if result.interrupted {
    // Fuel exhausted — INV-007 enforced
}
```

## Invariants

- **INV-001**: Credentials cannot reach LLM context (credential never in `call.arguments`)
- **INV-002**: `ProtectedSecret` has no `Serialize` impl (compile-time guarantee)
- **INV-003**: No `secrecy/serde` feature enabled anywhere in workspace
- **INV-005**: Tool allowlist is deny-by-default
- **INV-007**: WASM execution terminates on fuel exhaustion
- **INV-008**: SSRF guard blocks 127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16
- **INV-009**: Plugins must be Ed25519-signed before loading

## Common Mistakes

- Never call `serde_json::to_string(&protected_secret)` — it won't compile (INV-002)
- Never enable `secrecy/serde` feature — breaks the whole workspace
- Always check allowlist BEFORE scanning input — fail fast on unknown tools
