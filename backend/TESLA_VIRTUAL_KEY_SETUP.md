# Tesla Virtual Key Setup - Critical Missing Step

## Problem

You're getting this error when trying to send commands to your Tesla:

```
vehicle rejected request: your public key has not been paired with the vehicle
```

## Root Cause

Your Tesla integration is **90% complete** but missing one critical step: **Adding the virtual key to your vehicle**.

### What's Working ✅
- OAuth authentication (access tokens)
- Public/private key generation
- Public key being served at `/.well-known/appspecific/com.tesla.3p.public-key.pem`
- Tesla HTTP proxy running and signing commands
- Partner account registration with Tesla

### What's Missing ❌
- **Virtual key pairing** - The vehicle doesn't know about your public key yet

## Solution

Tesla requires an explicit authorization step where you add your app's public key to your vehicle using the Tesla mobile app. This is a security measure to prevent unauthorized command access.

## Implementation Added

I've added a new endpoint to help you complete this step:

### New API Endpoint

**GET** `/api/auth/tesla/virtual-key`

**Headers:**
```
Authorization: Bearer <your-access-token>
```

**Response:**
```json
{
  "pairing_link": "https://www.tesla.com/_ak/your-domain.com",
  "domain": "your-domain.com",
  "instructions": "Open this link on your mobile device or scan the QR code in your Tesla mobile app to authorize vehicle commands. This is required before you can control your vehicle remotely.",
  "qr_code_url": "https://api.qrserver.com/v1/create-qr-code/?size=300x300&data=https%3A%2F%2Fwww.tesla.com%2F_ak%2Fyour-domain.com"
}
```

## How to Complete the Setup

### Step 1: Get Your Pairing Link

Call the new endpoint (you'll need to be authenticated as user who has Tesla connected):

```bash
curl -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:3000/api/auth/tesla/virtual-key
```

### Step 2: Add Key to Vehicle

**Option A: Direct Link (Recommended)**
1. Copy the `pairing_link` from the response
2. Open it on your **mobile device** (the one with Tesla app installed)
3. The link will automatically open the Tesla mobile app
4. Follow the prompts to authorize vehicle commands
5. Select which vehicle(s) to grant access to

**Option B: QR Code**
1. Use the `qr_code_url` from the response to display a QR code
2. Scan it with your Tesla mobile app
3. Follow the authorization prompts

### Step 3: Test

After completing the authorization, retry your Tesla command:

```
"Turn on climate control in my Tesla"
```

It should now work! You'll see in the logs:
```
Sending signed command 'auto_conditioning_start' via proxy to vehicle 5YJ3E7EA1LF771842
```

## Frontend Integration

You should add this to your Tesla connection UI:

1. **After successful OAuth connection**, show a message:
   ```
   ⚠️ One more step required: Authorize vehicle commands
   ```

2. **Display the pairing link and QR code** from the `/api/auth/tesla/virtual-key` endpoint

3. **Instructions:**
   ```
   To control your Tesla remotely, you need to authorize this app in your Tesla mobile app.

   Tap the link below or scan the QR code:
   [pairing_link]

   This is a one-time setup and can be revoked anytime in your Tesla app.
   ```

4. **Optional**: Add a "Test Connection" button that tries a simple command (like getting vehicle data) to verify the key was added successfully

## Security Notes

- This is a Tesla security requirement - you cannot bypass it
- Users maintain full control and can revoke access anytime via:
  - Vehicle's Locks screen in the Tesla app
  - Tesla Account Security page at tesla.com
- The virtual key is tied to your domain (from `SERVER_URL` env var)
- Multiple vehicles can be authorized with the same key

## Technical Details

### What is a Virtual Key?

A virtual key is Tesla's term for authorizing a third-party application to send signed commands to a vehicle. It's separate from OAuth (which grants API access) and provides an additional security layer specifically for vehicle commands.

### The Flow

1. **OAuth** → Grants API access to read data
2. **Virtual Key** → Grants ability to send commands
3. **Command Signing** → Your proxy signs each command with your private key
4. **Vehicle Verification** → Vehicle verifies signature using your public key (which you added via the mobile app)

### Why is this necessary?

- OAuth tokens can be stolen/leaked
- Virtual keys require physical access to the Tesla mobile app
- Provides defense-in-depth security
- Allows granular per-vehicle authorization
- Users can easily revoke access

## Testing Locally

For local development, your `SERVER_URL` might be `http://localhost:3000`. This will create a pairing link like:

```
https://www.tesla.com/_ak/localhost:3000
```

**Note**: Tesla may have restrictions on localhost domains. For testing:
1. Use a tunneling service (ngrok, cloudflare tunnel)
2. Update `SERVER_URL` to your tunnel URL
3. Restart your backend
4. Get a new pairing link

## Files Modified

1. **`backend/src/handlers/tesla_auth.rs`**
   - Added `get_virtual_key_link()` function

2. **`backend/src/main.rs`**
   - Added route: `GET /api/auth/tesla/virtual-key`

3. **`backend/TESLA_SETUP.md`**
   - Added documentation for virtual key pairing step
   - Added troubleshooting section for pairing errors

## Next Steps

1. **Backend**: Already done! ✅
2. **Frontend**: Add UI to show pairing link/QR code after Tesla connection
3. **User Flow**: Guide users through the pairing process
4. **Testing**: Complete the pairing on your own vehicle to test

## References

- Tesla Virtual Keys Overview: https://developer.tesla.com/docs/fleet-api/virtual-keys/overview
- Tesla Fleet API: https://developer.tesla.com/docs/fleet-api
- Tesla Vehicle Commands: https://developer.tesla.com/docs/fleet-api/endpoints/vehicle-commands
