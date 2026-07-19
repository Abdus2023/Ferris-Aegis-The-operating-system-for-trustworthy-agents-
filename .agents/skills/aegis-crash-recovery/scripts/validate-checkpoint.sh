#!/usr/bin/env bash
# Validate Ferris Aegis checkpoint integrity
# Usage: ./validate-checkpoint.sh <checkpoint-db-path>

set -euo pipefail

DB_PATH="${1:?Usage: $0 <checkpoint-db-path>}"

if ! command -v sqlite3 &>/dev/null; then
    echo "ERROR: sqlite3 not found. Install with: apt-get install sqlite3"
    exit 1
fi

if [ ! -f "$DB_PATH" ]; then
    echo "ERROR: Database not found: $DB_PATH"
    exit 1
fi

echo "╔══════════════════════════════════════════════════════╗"
echo "║   Ferris Aegis — Checkpoint Integrity Validator      ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

# Count total checkpoints
TOTAL=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM checkpoints;" 2>/dev/null || echo "0")
echo "Total checkpoints: $TOTAL"

# List all workflows
echo ""
echo "Workflows:"
sqlite3 -header -column "$DB_PATH" \
    "SELECT workflow_id, MAX(step_index) as last_step, COUNT(*) as checkpoints, created_at
     FROM checkpoints GROUP BY workflow_id ORDER BY workflow_id;" 2>/dev/null

# Find incomplete workflows
echo ""
echo "Incomplete workflows (may need recovery):"
sqlite3 -header -column "$DB_PATH" \
    "SELECT c1.workflow_id, c1.step_index, 
            json_extract(c1.checkpoint_data, '$.total_steps') as total_steps,
            json_extract(c1.checkpoint_data, '$.step_outcome.success') as last_success,
            c1.created_at
     FROM checkpoints c1
     WHERE c1.step_index = (
         SELECT MAX(c2.step_index) FROM checkpoints c2 WHERE c2.workflow_id = c1.workflow_id
     )
     AND (
         c1.step_index + 1 < json_extract(c1.checkpoint_data, '$.total_steps')
         OR json_extract(c1.checkpoint_data, '$.step_outcome.success') = 0
     );" 2>/dev/null

echo ""
echo "✓ Validation complete"
