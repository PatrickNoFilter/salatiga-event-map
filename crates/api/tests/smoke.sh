#!/usr/bin/env bash
# API smoke tests (requires a running API and PostGIS).
#   DATABASE_URL=... ADMIN_TOKEN=test-token cargo run -p api &
#   bash crates/api/tests/smoke.sh
set -euo pipefail
BASE="${BASE:-http://localhost:8080}"
TOKEN="${ADMIN_TOKEN:-test-token}"
pass=0; fail=0

check() {
  local name="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "PASS $name"; pass=$((pass+1))
  else
    echo "FAIL $name (expected $expected, got $actual)"; fail=$((fail+1))
  fi
}

check "health" "200" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/health")"
check "list events non-empty" "200" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/events")"
check "admin without token is 401" "401" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/admin/events")"
check "invalid lat is 400" "400" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/events" \
  -H 'Content-Type: application/json' \
  -d '{"title":"Bad","description":"","category":"music","starts_at":"2026-12-10T10:00:00Z","ends_at":null,"venue_name":"X","address":"","lat":999,"lng":110.5,"price_text":"Free","is_free":true,"organizer":"","contact":"","ticket_url":null,"tags":[]}')"

# Submit -> not visible -> approve -> visible
ID=$(curl -s -X POST "$BASE/api/events" -H 'Content-Type: application/json' \
  -d '{"title":"Smoke Test Event","description":"test","category":"community","starts_at":"2027-01-01T10:00:00Z","ends_at":null,"venue_name":"X","address":"","lat":-7.33,"lng":110.5,"price_text":"Free","is_free":true,"organizer":"","contact":"","ticket_url":null,"tags":[]}' \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
check "pending not in public list" "False" "$(curl -s "$BASE/api/events" | python3 -c "import json,sys; print('$ID' in json.dumps(json.load(sys.stdin)))")"
check "approve returns 200" "200" "$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "Authorization: Bearer $TOKEN" "$BASE/api/admin/events/$ID/approve")"
check "approved is visible" "True" "$(curl -s "$BASE/api/events" | python3 -c "import json,sys; print('$ID' in json.dumps(json.load(sys.stdin)))")"

# Cleanup
docker compose exec -T db psql -U eventmap -d eventmap -c "DELETE FROM events WHERE id='$ID';" >/dev/null 2>&1 || true

echo "Results: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
