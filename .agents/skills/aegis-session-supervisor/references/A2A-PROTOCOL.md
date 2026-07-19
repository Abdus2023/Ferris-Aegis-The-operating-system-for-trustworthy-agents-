# Ferris Aegis — A2A Protocol Reference

> Loaded on demand by the aegis-session-supervisor skill.

## AgentCard Schema

```json
{
    "name": "aegis-agent",
    "description": "Ferris Aegis Guardian Agent",
    "url": "https://aegis.example.com",
    "provider": { "organization": "Ferris Aegis" },
    "version": "0.4.0",
    "capabilities": {
        "streaming": true,
        "pushNotifications": false
    },
    "authentication": {
        "schemes": ["bearer"]
    },
    "skills": [
        {
            "id": "trust-evaluation",
            "name": "Trust Evaluation",
            "description": "Evaluates agent trustworthiness"
        }
    ]
}
```

## AgentCard Path

Per ADR-005: Served at `/.well-known/agent-card.json` (A2A spec + RFC 8615).
NOT `/.well-known/agent.json`.

## Task Lifecycle

```
Submitted → Working → Completed
                    → Cancelled
                    → Failed
```

## Trust-Gated Routing

`A2aRouter.route_message(sender, target, message)`:
1. Check sender trust level meets target's minimum
2. Check sender has required skills
3. Check protocol compatibility
4. If all pass → route message
5. If any fail → `RouteError` with reason

## Branch A vs Branch B

| Branch | Approach | Use Case |
|--------|----------|----------|
| A (standalone) | HTTP server serving AgentCard | External discovery, multi-agent systems |
| B (MCP) | MCP tool params integrated | Single-agent workflows via MCP server |

Both are implemented. Choice is open (ADR-008).
