# Ferris Aegis — Additional Visual Diagrams

This document contains high-quality Mermaid diagrams for key architectural flows, state machines, and security models not covered in the main specification.

---

## 1. Trust Level Hierarchy & Capability Progression

```mermaid
graph TD
    subgraph TrustLevels["Trust Level Hierarchy"]
        U[Unverified<br/>0.00–0.19] --> P[Probationary<br/>0.20–0.49]
        P --> S[Standard<br/>0.50–0.74]
        S --> E[Elevated<br/>0.75–0.94]
        E --> Sov[Sovereign<br/>0.95–1.00]
    end

    subgraph Capabilities["Progressive Capabilities"]
        U -->|Timer, InterAgentComm| P
        P -->|FileSystemRead| S
        S -->|NetworkAccess, Environment, Audit, ExtendedMemory| E
        E -->|FileSystemWrite, ProcessSpawn, Crypto| Sov
        Sov -->|PolicyModify, AgentManagement| Sov
    end

    style U fill:#ffcccc
    style P fill:#ffe6cc
    style S fill:#ffffcc
    style E fill:#ccffcc
    style Sov fill:#cce5ff
```

---

## 2. Agent Lifecycle State Machine

```mermaid
stateDiagram-v2
    [*] --> Spawning
    Spawning --> Running : spawn()
    Running --> Suspended : suspend()
    Running --> Quarantined : guard intervention
    Running --> Completed : task finished
    Running --> Terminated : terminate()
    Running --> Failed : error

    Suspended --> Running : resume()
    Quarantined --> Terminated : severe violation

    Suspended --> Terminated : terminate()
    Completed --> [*]
    Terminated --> [*]
    Failed --> [*]

    note right of Running
        All transitions are audited
        and trust score is updated
    end note
```

---

## 3. Policy Evaluation Flow

```mermaid
flowchart TD
    Start[Action Request<br/>action + target] --> Sort[Sort Policies<br/>by Priority DESC]
    Sort --> Loop{For each enabled<br/>Policy}
    Loop --> RuleCheck{For each Rule:<br/>matches_action && matches_target?}
    RuleCheck -->|Yes| Return[Return Verdict<br/>Allow / Deny]
    RuleCheck -->|No| NextRule
    NextRule --> Loop
    Loop -->|No more policies| Default[Use Highest Priority<br/>Policy's default_effect]
    Default --> Return

    style Return fill:#d4edda
    style Default fill:#fff3cd
```

---

## 4. Credential Protection Flow (INV-001)

```mermaid
sequenceDiagram
    participant LLM as LLM
    participant TC as ToolCall
    participant AC as AuthenticatedCall
    participant Exec as Tool Executor
    participant Vault as CredentialVault

    LLM->>TC: Propose ToolCall<br/>(no credential in arguments)
    TC->>AC: with_credential(ProtectedSecret)
    Note over AC: call: ToolCall (safe)<br/>credential: ProtectedSecret (never serialized)
    AC->>Exec: Execute with separate fields
    Exec->>Vault: expose_secret() only here
    Vault-->>Exec: Decrypted secret
    Exec->>External: Use secret (e.g. Authorization header)
```

---

## 5. Guard Escalation Ladder

```mermaid
graph TD
    subgraph Monitoring["Continuous Monitoring"]
        Action[Action Rate]
        Trust[Trust Score]
        Violation[Policy Violations]
        Resource[Resource Usage]
    end

    subgraph Thresholds["Escalation Thresholds"]
        Action -->|≥500| Alert[Alert]
        Action -->|≥750| Throttle[Throttle]
        Action -->|≥1000| Quarantine[Quarantine]

        Trust -->|< min_score| Quarantine
        Violation -->|≥10/min| Quarantine
    end

    subgraph Actions["Guard Actions"]
        Alert --> Log[Log Warning]
        Throttle --> Slow[Slow Execution<br/>throttle_factor < 1.0]
        Quarantine --> Isolate[Strip Capabilities<br/>+ Suspend]
        Quarantine --> Terminate[Terminate]
    end

    style Alert fill:#fff3cd
    style Throttle fill:#ffe5b4
    style Quarantine fill:#f8d7da
    style Terminate fill:#f5c6cb
```

---

## 6. Full System Data Flow

```mermaid
flowchart LR
    subgraph Agent["Agent Request"]
        Req[Action]
    end

    subgraph Core["Kernel Layer"]
        Policy[PolicyEngine]
        Sandbox[Sandbox]
        Guard[Guard]
        Audit[AuditLedger]
        Trust[TrustKernel]
    end

    subgraph Security["Security Layer"]
        Allow[Allowlist]
        Inject[InjectionScanner]
        SSRF[SsrfGuard]
        Vault[ProtectedSecret Vault]
    end

    subgraph Execution["Execution Layer"]
        Skills[SkillExecutor]
        WASM[WasmSandbox]
        MCP[MCP Server]
    end

    subgraph Observability["Observability"]
        OTel[OTel Spans]
        Metrics[Prometheus]
        Log[JSON stderr]
    end

    Req --> Policy
    Policy -->|Allowed| Sandbox
    Sandbox --> Guard
    Guard -->|No intervention| Security
    Security --> Execution
    Execution --> Audit
    Audit --> Trust
    Trust -->|reinforce/penalize| Observability

    Guard -->|Quarantine/Terminate| Audit
    Security -->|Denied| Audit

    style Core fill:#e3f2fd
    style Security fill:#fff3e0
    style Execution fill:#f3e5f5
    style Observability fill:#e8f5e9
```

---

## 7. Resilience Composition Pipeline

```mermaid
flowchart TD
    Start[Operation Call] --> RL[RateLimiter<br/>Token Bucket]
    RL --> CB[CircuitBreaker<br/>State Check]
    CB --> TO[Timeout Wrapper]
    TO --> Retry[RetryPolicy<br/>Exponential Backoff + Jitter]
    Retry --> Exec[Execute Operation]
    Exec -->|Success| Success[Return Result]
    Exec -->|Failure| CBUpdate[Update Circuit State]
    CBUpdate --> Retry

    style RL fill:#d1ecf1
    style CB fill:#d4edda
    style TO fill:#fff3cd
    style Retry fill:#f8d7da
```

---

## 8. Skill Execution with Trust Gating

```mermaid
sequenceDiagram
    participant CLI as CLI / MCP
    participant Reg as SkillRegistry
    participant Val as SkillValidator
    participant Exec as SkillExecutor
    participant Ctx as ExecutionContext
    participant TK as TrustKernel

    CLI->>Reg: Load skills from directory
    CLI->>Reg: get_skill(skill_id)
    Reg-->>CLI: Skill manifest
    CLI->>Val: validate_execution(skill, context)
    Val->>TK: Check agent_trust_score >= trust_level_minimum
    Val->>Ctx: Verify capabilities intersection
    Val->>Ctx: Check sandbox_boundary
    Val-->>CLI: Validation OK
    CLI->>Exec: execute(skill, context, input)
    Exec->>Ctx: Enforce resource_limits
    Exec-->>CLI: SkillExecutionResult
```

---

These diagrams complement the main `SPECIFICATION.md` and can be rendered in any Markdown viewer that supports Mermaid (GitHub, GitLab, VS Code, etc.).

**Next Steps Recommendation**: Render these diagrams using a Mermaid live editor or integrate them into documentation using tools like `mmdc` (Mermaid CLI) for PNG/SVG export.

---

## 9. WASM Sandbox Execution Flow

```mermaid
flowchart TD
    Start[WASM Module Request] --> Verify[Plugin Verification<br/>Ed25519 Signature + SHA-256 Hash]
    Verify -->|Valid| Load[Load Module into Wasmtime]
    Verify -->|Invalid| Reject[Reject Execution]

    Load --> Limits[Apply Limits<br/>Fuel + Memory + Epoch]
    Limits --> Execute[Execute with Input]
    Execute --> FuelCheck{Fuel Exhausted?}
    FuelCheck -->|Yes| Terminate[Terminate with Fuel Error]
    FuelCheck -->|No| MemoryCheck{Memory Exceeded?}
    MemoryCheck -->|Yes| Terminate
    MemoryCheck -->|No| EpochCheck{Epoch Deadline?}
    EpochCheck -->|Yes| Terminate
    EpochCheck -->|No| Result[Return Execution Result]

    style Verify fill:#d4edda
    style Limits fill:#fff3cd
    style Terminate fill:#f8d7da
```

**Safety Layers**:
- **Fuel Metering** (default 10M instructions)
- **Memory Cap** (default 64 MiB)
- **Epoch Interruption** (hard deadline)
- **Plugin Attestation** (Ed25519 + module hash)

---

## 10. A2A Routing with Trust Gating

```mermaid
flowchart TD
    Sender[Sender Agent] --> Card[Fetch Target AgentCard<br/>/.well-known/agent-card.json]
    Card --> Router[A2aRouter]
    Router --> TrustCheck{Trust Level Check<br/>sender.score >= skill.min_trust?}
    TrustCheck -->|Yes| CapabilityCheck{Capability Intersection?}
    TrustCheck -->|No| Deny[Deny Route<br/>Insufficient Trust]
    CapabilityCheck -->|Yes| Route[Route Message<br/>to Target Skill]
    CapabilityCheck -->|No| Deny

    Router --> Registry[Agent Registry<br/>+ Skill Discovery]
    Registry --> Route

    style TrustCheck fill:#fff3cd
    style Deny fill:#f8d7da
    style Route fill:#d4edda
```

**Trust-Gated Routing Logic**:
1. Sender presents `AgentCard`
2. Router queries `TrustKernel` for sender’s current score
3. Target skill declares `trust_level_minimum`
4. Message is routed only if `sender_score >= minimum`
5. Capability intersection is also validated

---

## 11. Plugin Signing & Verification Flow

```mermaid
sequenceDiagram
    participant Dev as Developer
    participant Sign as Plugin Signer
    participant Reg as Plugin Registry
    participant Loader as Plugin Loader
    participant Exec as WasmSandbox

    Dev->>Sign: Build WASM module + manifest
    Sign->>Sign: Compute SHA-256 of WASM
    Sign->>Sign: Sign manifest with Ed25519 private key
    Sign-->>Reg: Upload signed plugin

    Loader->>Reg: Request plugin
    Reg-->>Loader: Signed manifest + WASM
    Loader->>Loader: Verify Ed25519 signature
    Loader->>Loader: Verify WASM hash matches manifest
    Loader-->>Exec: Load verified module
```

**Verification Steps**:
1. Ed25519 signature validation on manifest
2. SHA-256 hash of WASM matches declared value
3. Only then is the module loaded into the sandbox

---

## 12. Observability Pipeline

```mermaid
flowchart LR
    subgraph Sources["Telemetry Sources"]
        Kernel[Kernel Components]
        MCP[MCP Handlers]
        Skills[Skill Executor]
        Guard[Guard Alerts]
    end

    subgraph Pipeline["Observability Pipeline"]
        OTel[OpenTelemetry<br/>Batch Export]
        Prom[Prometheus<br/>Registry]
        JSON[JSON Structured Logging<br/>stderr only]
    end

    subgraph Consumers["Consumers"]
        Jaeger[Jaeger / Tempo]
        Grafana[Grafana]
        Loki[Loki / Datadog]
    end

    Kernel --> OTel
    MCP --> OTel
    Skills --> OTel
    Guard --> OTel

    Kernel --> Prom
    MCP --> Prom
    Skills --> Prom

    Kernel --> JSON
    MCP --> JSON
    Skills --> JSON
    Guard --> JSON

    OTel --> Jaeger
    Prom --> Grafana
    JSON --> Loki

    style JSON fill:#f8d7da
    note right of JSON
        MCP owns stdout.
        All logs go to stderr.
    end note
```

---

## 13. MCP + Skills Integration

```mermaid
flowchart TD
    Client[MCP Client] --> Server[MCP Server<br/>stdio transport]
    Server --> Handler[SkillMcpHandler]
    Handler --> Registry[SkillRegistry]
    Registry --> Validator[SkillValidator]
    Validator --> Executor[SkillExecutor]
    Executor --> Context[ExecutionContext<br/>trust + capabilities + sandbox]
    Context --> WASM[WasmSandbox / Native]
    Executor --> Audit[AuditLedger + Metrics]
    Executor --> Result[Return to MCP Client]

    style Handler fill:#e3f2fd
    style Validator fill:#fff3cd
```

---

These additional diagrams complete the visual documentation suite for Ferris Aegis.

---

## 14. Session Lifecycle with Budgets

```mermaid
stateDiagram-v2
    [*] --> Created
    Created --> Active : start_session()
    Active --> Active : round_completed()
    Active --> BudgetCheck{Budget Check}

    BudgetCheck -->|Tokens / Cost / Rounds / Time OK| Active
    BudgetCheck -->|Any Budget Exhausted| Terminal

    Active --> Suspended : manual suspend
    Suspended --> Active : resume()

    Terminal --> Completed : normal end
    Terminal --> Terminated : forced termination

    Completed --> [*]
    Terminated --> [*]

    note right of Active
        Budgets tracked:
        - tokens_used
        - cost_usd
        - rounds
        - wall_clock_time
    end note
```

---

## 15. Semantic Memory Pipeline

```mermaid
flowchart TD
    Conversation[Conversation Text] --> Extract[Concept Extraction<br/>Keyword Matching]
    Extract --> StoreConcept[Store Concept]
    Conversation --> Embed[Generate Embedding<br/>Vector Representation]
    Embed --> StoreEmbed[Store StoredEmbedding]
    Embed --> Similarity[Cosine Similarity Search]
    Similarity --> Retrieve[Retrieve Similar Concepts]

    Conversation --> Summarize[Conversation Summarization]
    Summarize --> StoreSummary[Store Summary]

    StoreConcept --> SemanticDB[(Semantic Memory<br/>SQLite)]
    StoreEmbed --> SemanticDB
    StoreSummary --> SemanticDB

    SemanticDB --> Supervisor[Supervisor<br/>Anomaly Detection]
    SemanticDB --> A2A[A2A Router<br/>Skill Discovery]

    style SemanticDB fill:#e8f5e9
```

**Components**:
- `Concept` extraction
- `StoredEmbedding` with cosine similarity
- `Summary` for conversation compression
- Used by Supervisor and A2A routing

---

## 16. Resilience State Machines

```mermaid
stateDiagram-v2
    direction LR

    subgraph CircuitBreaker["Circuit Breaker States"]
        Closed -->|N failures| Open
        Open -->|Timeout| HalfOpen
        HalfOpen -->|M successes| Closed
        HalfOpen -->|Failure| Open
    end

    subgraph Retry["Retry Policy"]
        Attempt -->|Failure + attempts left| Backoff[Exponential Backoff<br/>+ 25% Jitter]
        Backoff --> Attempt
        Attempt -->|Success| Success
        Attempt -->|Max attempts| Fail
    end

    subgraph RateLimiter["Rate Limiter"]
        Request --> TokenCheck{Token Available?}
        TokenCheck -->|Yes| Execute
        TokenCheck -->|No| Wait[Wait for refill]
        Wait --> TokenCheck
        Execute --> Refill[Token Refill<br/>(token bucket)]
    end
```

**Combined Resilience Execution**:
All three primitives are composed in `execute_resilient()`.

---

## 17. Consolidated Architecture Overview (Multi-View)

```mermaid
flowchart TB
    subgraph External["External World"]
        User[User / CLI]
        LLM[LLM Provider]
        MCPClient[MCP Clients]
        A2AAgent[Other A2A Agents]
    end

    subgraph CoreOS["Ferris Aegis Core OS<br/>(crates/kernel)"]
        direction TB
        TK[TrustKernel<br/>Score + Level + Attestation]
        AR[AgentRuntime<br/>Lifecycle Management]
        PE[PolicyEngine<br/>Default-Deny + Priority]
        SB[Sandbox<br/>12 Capabilities]
        GR[Guard<br/>Real-time Monitoring]
        AL[AuditLedger<br/>SHA-256 Chain]
    end

    subgraph SecurityLayer["Security Layer<br/>(crates/security)"]
        VA[Credential Vault<br/>ProtectedSecret]
        IS[Injection Scanner]
        SG[SsrfGuard]
        ALW[Tool Allowlist]
    end

    subgraph ExecutionLayer["Execution Layer"]
        Skills[Skills System<br/>Registry + Validator + Executor]
        WASM[WasmSandbox<br/>Fuel + Memory + Epoch]
        MCP[MCP Server<br/>V_2025_11_25]
    end

    subgraph Advanced["Advanced Subsystems"]
        Session[Session Manager<br/>Budgets]
        Semantic[Semantic Memory<br/>Concepts + Embeddings]
        Supervisor[Supervisor<br/>Anomaly Detection]
        A2A[A2A Protocol<br/>AgentCard + Router]
        Resilience[Resilience<br/>CircuitBreaker + Retry + RateLimiter]
    end

    subgraph Observability["Observability<br/>(crates/observability)"]
        OTel[OTel Tracing<br/>Batch Export]
        Prom[Prometheus Metrics]
        Log[JSON Logging<br/>stderr only]
    end

    %% Connections
    User --> AR
    LLM --> Skills
    MCPClient --> MCP
    A2AAgent --> A2A

    AR --> TK
    AR --> PE
    AR --> SB
    GR --> AR
    SB --> GR
    PE --> AL
    AL --> OTel

    Skills --> VA
    Skills --> IS
    Skills --> SG
    Skills --> ALW
    Skills --> WASM
    Skills --> MCP

    Session --> AR
    Semantic --> Supervisor
    Semantic --> A2A
    Supervisor --> GR
    A2A --> TK
    Resilience --> Skills
    Resilience --> WASM

    OTel --> Log
    Prom --> Log
    Log --> OTel

    style CoreOS fill:#e3f2fd
    style SecurityLayer fill:#fff3e0
    style ExecutionLayer fill:#f3e5f5
    style Advanced fill:#e8f5e9
    style Observability fill:#fce4ec
```

**Legend**:
- **Blue**: Core OS primitives
- **Orange**: Security & isolation
- **Purple**: Execution & skills
- **Green**: Advanced memory & orchestration
- **Pink**: Observability (always active, stderr-only)

---

## 18. Complete Component Dependency Graph

```mermaid
graph TD
    CLI[ferris-aegis CLI] --> Kernel
    CLI --> Observability
    CLI --> MCP
    CLI --> Security
    CLI --> Skills

    Kernel --> Observability
    MCP --> Observability
    Skills --> Kernel
    Skills --> Security
    Skills --> Observability
    A2A --> Kernel
    Resilience --> Skills
    Supervisor --> Kernel
    SemanticMemory --> Kernel

    style CLI fill:#f0f0f0
    style Kernel fill:#bbdefb
    style Observability fill:#c8e6c9
```

---

**Document Complete** — 18 diagrams covering all major subsystems of Ferris Aegis.

---

## 19. A2A Message Routing Sequence

```mermaid
sequenceDiagram
    participant Sender as Sender Agent
    participant Router as A2aRouter
    participant Registry as Agent Registry
    participant Recipient as Recipient Agent

    Sender->>Router: A2aEnvelope (with sender_card)
    Router->>Registry: lookup(recipient)
    Registry-->>Router: AgentCard or NotFound
    Router->>Router: verify_trust(sender_card)
    Router->>Router: check can_initiate()
    Router->>Recipient: Forward message
    Recipient-->>Sender: Response (via router)
```

---

## 20. Resilience Execution Layers

```mermaid
flowchart TD
    Request[Incoming Request] --> CB[Circuit Breaker<br/>allow_request()]
    CB -->|Allowed| Retry[RetryPolicy<br/>execute()]
    Retry --> Timeout[with_timeout()]
    Timeout --> Operation[Actual Operation]
    Operation -->|Success| RecordSuccess[record_success()]
    Operation -->|Failure| RecordFailure[record_failure()]

    RecordSuccess --> Return[Return Result]
    RecordFailure --> Return

    style CB fill:#bbdefb
    style Retry fill:#c8e6c9
    style Timeout fill:#fff9c4
```

---

## 21. Circuit Breaker State Transitions

```mermaid
stateDiagram-v2
    Closed -->|failure_count >= threshold| Open
    Open -->|recovery_timeout elapsed| HalfOpen
    HalfOpen -->|success_count >= threshold| Closed
    HalfOpen -->|failure| Open
    Closed -->|manual force_open| Open
    Open -->|manual force_closed| Closed
```

---

These additional diagrams provide deeper insight into A2A routing mechanics and resilience behavior.