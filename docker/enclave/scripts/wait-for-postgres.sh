#!/bin/bash
# Wait for PostgreSQL to be ready before starting services

set -e

PGBIN="/usr/lib/postgresql/15/bin"
MAX_RETRIES=30
RETRY_INTERVAL=1

echo "Waiting for PostgreSQL to be ready..."

for i in $(seq 1 $MAX_RETRIES); do
    if $PGBIN/pg_isready -q -h localhost -p 5432; then
        echo "PostgreSQL is ready"
        exit 0
    fi
    echo "PostgreSQL not ready yet (attempt $i/$MAX_RETRIES)..."
    sleep $RETRY_INTERVAL
done

echo "ERROR: PostgreSQL did not become ready in time"
exit 1
