#!/bin/sh
set -e

SECRETS_FILE="/app/data/.secrets.env"

# Generate and persist JWT secrets on first boot.
# Stored in /app/data/ which should be a persistent volume in Coolify.
if [ ! -f "$SECRETS_FILE" ]; then
    echo "First boot: generating JWT secrets..."
    ACCESS_SECRET=$(head -c 48 /dev/urandom | base64 | tr -d '/+\n' | head -c 64)
    REFRESH_SECRET=$(head -c 48 /dev/urandom | base64 | tr -d '/+\n' | head -c 64)
    cat > "$SECRETS_FILE" <<SECRETS
DALLAS_PDS_JWT__ACCESS_SECRET=${ACCESS_SECRET}
DALLAS_PDS_JWT__REFRESH_SECRET=${REFRESH_SECRET}
SECRETS
    chmod 600 "$SECRETS_FILE"
    echo "Secrets saved to $SECRETS_FILE"
fi

# Source persisted secrets — won't override if already set via Coolify env vars
while IFS='=' read -r key value; do
    [ -z "$key" ] && continue
    eval current=\${$key:-}
    if [ -z "$current" ]; then
        export "$key=$value"
    fi
done < "$SECRETS_FILE"

exec "$@"
