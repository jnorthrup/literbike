#!/usr/bin/env bash
set -euo pipefail

COUCHDB_URL=${COUCHDB_URL:-http://127.0.0.1:5984}
DB=${COUCHDB_DB:-literbike}
DOC=${1:-literbike_repo}

# Create database if missing
curl -s -X PUT "$COUCHDB_URL/$DB" || true
# Create document if missing
curl -s -X PUT "$COUCHDB_URL/$DB/$DOC" -H "Content-Type: application/json" -d '{}' || true

echo "Ensured DB='$DB' and doc='$DOC' exist at $COUCHDB_URL"
