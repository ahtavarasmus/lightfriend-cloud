#!/bin/bash
# Script to get your Tesla virtual key pairing link
# Usage: ./get_tesla_pairing_link.sh <your-access-token>

set -e

if [ -z "$1" ]; then
    echo "Usage: ./get_tesla_pairing_link.sh <your-access-token>"
    echo ""
    echo "Get your access token by logging in to your app and checking"
    echo "the Authorization header or localStorage."
    exit 1
fi

ACCESS_TOKEN="$1"
BACKEND_URL="${BACKEND_URL:-http://localhost:3000}"

echo "🔑 Fetching Tesla virtual key pairing information..."
echo ""

RESPONSE=$(curl -s -H "Authorization: Bearer $ACCESS_TOKEN" \
    "$BACKEND_URL/api/auth/tesla/virtual-key")

# Check if jq is available
if command -v jq &> /dev/null; then
    echo "✅ Response:"
    echo "$RESPONSE" | jq .
    echo ""

    PAIRING_LINK=$(echo "$RESPONSE" | jq -r '.pairing_link')
    QR_CODE_URL=$(echo "$RESPONSE" | jq -r '.qr_code_url')

    if [ "$PAIRING_LINK" != "null" ]; then
        echo "📱 Open this link on your mobile device:"
        echo "   $PAIRING_LINK"
        echo ""
        echo "📷 Or scan this QR code in your Tesla app:"
        echo "   $QR_CODE_URL"
        echo ""
        echo "🚗 After authorizing in the Tesla mobile app, you can send commands to your vehicle!"
    else
        echo "❌ Error: Could not get pairing link"
        echo "   Make sure you have connected your Tesla account first"
    fi
else
    echo "⚠️  jq not installed, showing raw response:"
    echo "$RESPONSE"
    echo ""
    echo "Install jq for better formatting: brew install jq (macOS) or apt install jq (Linux)"
fi
