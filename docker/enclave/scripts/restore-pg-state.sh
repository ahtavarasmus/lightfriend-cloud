#!/bin/bash
# Restore PostgreSQL state from parent instance
# Called at startup to restore bridge state from encrypted backups

set -e

echo "Attempting to restore PostgreSQL state from parent..."

STATE_URL="http://127.0.0.1:${VSOCK_STATE_PORT:-5002}/state/postgres"
RESTORE_DIR="/tmp/pg_restore"

mkdir -p "$RESTORE_DIR"

# Try to fetch the state backup from parent
if curl -sf "$STATE_URL" -o "$RESTORE_DIR/pg_backup.tar.gz"; then
    echo "State backup received, restoring..."

    # Extract and restore each database
    cd "$RESTORE_DIR"
    tar -xzf pg_backup.tar.gz

    PGBIN="/usr/lib/postgresql/15/bin"

    for db in whatsapp signal messenger instagram telegram; do
        dump_file="${db}_db.sql"
        if [ -f "$dump_file" ]; then
            echo "Restoring ${db}_db..."
            $PGBIN/psql -h localhost -U ${db}_user -d ${db}_db < "$dump_file" 2>/dev/null || true
        fi
    done

    rm -rf "$RESTORE_DIR"
    echo "PostgreSQL state restored"
else
    echo "No state backup available (fresh start or parent unavailable)"
fi

exit 0
