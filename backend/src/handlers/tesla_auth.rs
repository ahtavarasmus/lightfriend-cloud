use reqwest;
use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::{Query, State},
    response::{Json, Redirect},
    http::StatusCode,
    Extension,
};
use tower_sessions::{session_store::SessionStore, session::{Id, Record}};
use oauth2::{
    PkceCodeVerifier,
    AuthorizationCode,
    CsrfToken,
    PkceCodeChallenge,
    Scope,
    TokenResponse,
    RefreshToken,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use time::OffsetDateTime;
use tracing::{info, error};

use crate::{
    AppState,
    models::user_models::{NewTesla, User},
    utils::encryption::{encrypt, decrypt},
};

#[derive(Debug, Deserialize)]
pub struct TeslaCallbackParams {
    code: String,
    state: String,
}

#[derive(Serialize)]
pub struct TeslaStatusResponse {
    has_tesla: bool,
}

// Tesla OAuth login endpoint - requires Tier 2
pub async fn tesla_login(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Tesla OAuth login initiated for user {}", auth_user.user_id);

    // Check if user has Tier 2 subscription
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| {
            error!("Failed to get user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get user information"}))
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"}))
            )
        })?;

    if user.sub_tier != Some("tier 2".to_string()) && user.sub_tier != Some("tier 3".to_string()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Tesla integration requires a paid subscription"}))
        ));
    }

    // Generate session key and CSRF token
    let session_key = Uuid::new_v4().to_string();
    let csrf_token = CsrfToken::new_random();
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Create session record
    let mut record = Record {
        id: Id(Uuid::new_v4().as_u128() as i128),
        data: Default::default(),
        expiry_date: OffsetDateTime::now_utc() + time::Duration::hours(1),
    };

    record.data.insert("session_key".to_string(), json!(session_key.clone()));
    record.data.insert("pkce_verifier".to_string(), json!(pkce_verifier.secret().to_string()));
    record.data.insert("csrf_token".to_string(), json!(csrf_token.secret().to_string()));
    record.data.insert("user_id".to_string(), json!(auth_user.user_id));

    // Store session
    if let Err(e) = state.session_store.create(&mut record).await {
        error!("Failed to store session record: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store session record: {}", e)}))
        ));
    }

    let state_token = format!("{}:{}", record.id.0, csrf_token.secret());

    // Build authorization URL
    let (auth_url, _) = state
        .tesla_oauth_client
        .authorize_url(|| CsrfToken::new(state_token.clone()))
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("offline_access".to_string()))
        .add_scope(Scope::new("vehicle_device_data".to_string()))
        .add_scope(Scope::new("vehicle_cmds".to_string()))
        .add_scope(Scope::new("vehicle_charging_cmds".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    info!("Tesla OAuth URL generated with state: {}", state_token);

    Ok(Json(json!({
        "auth_url": auth_url.to_string(),
        "message": "Tesla OAuth flow initiated successfully"
    })))
}

// Tesla OAuth callback endpoint
pub async fn tesla_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TeslaCallbackParams>,
) -> Result<Redirect, (StatusCode, Json<serde_json::Value>)> {
    info!("Tesla OAuth callback received with state: {}", params.state);

    // Parse state token
    let state_parts: Vec<&str> = params.state.split(':').collect();
    if state_parts.len() != 2 {
        error!("Invalid state format: {}", params.state);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid state format"}))
        ));
    }

    let session_id_str = state_parts[0];
    let state_csrf = state_parts[1];

    // Parse session ID
    let session_id = session_id_str.parse::<i128>()
        .map_err(|e| {
            error!("Invalid session ID format: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid session ID"}))
            )
        })?;

    // Retrieve session
    let session_record = state.session_store.load(&Id(session_id)).await
        .map_err(|e| {
            error!("Failed to load session: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to load session"}))
            )
        })?
        .ok_or_else(|| {
            error!("Session not found for ID: {}", session_id);
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Session not found or expired"}))
            )
        })?;

    // Validate CSRF token
    let stored_csrf = session_record.data.get("csrf_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "CSRF token not found in session"}))
            )
        })?;

    if state_csrf != stored_csrf {
        error!("CSRF token mismatch");
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "CSRF token mismatch"}))
        ));
    }

    // Get user ID and PKCE verifier from session
    let user_id = session_record.data.get("user_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "User ID not found in session"}))
            )
        })? as i32;

    let pkce_verifier_secret = session_record.data.get("pkce_verifier")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "PKCE verifier not found in session"}))
            )
        })?;

    // Exchange authorization code for tokens
    // Note: Tesla uses a different domain for token exchange
    let pkce_verifier = PkceCodeVerifier::new(pkce_verifier_secret.to_string());

    // Build custom HTTP client for token exchange
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // Create a custom token exchange request since Tesla uses a different domain
    // We need to use fleet-auth.prd.vn.cloud.tesla.com for token exchange
    let token_url = "https://fleet-auth.prd.vn.cloud.tesla.com/oauth2/v3/token";
    let client_id = std::env::var("TESLA_CLIENT_ID")
        .unwrap_or_else(|_| "default-tesla-client-id-for-testing".to_string());
    let client_secret = std::env::var("TESLA_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-tesla-secret-for-testing".to_string());
    let server_url = std::env::var("SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let redirect_uri = format!("{}/api/auth/tesla/callback", server_url);

    // Get the audience URL from env or default to EU region
    let audience_url = std::env::var("TESLA_API_BASE")
        .unwrap_or_else(|_| "https://fleet-api.prd.eu.vn.cloud.tesla.com".to_string());

    // Manual token exchange request for Tesla's specific requirements
    let scope = "openid offline_access vehicle_device_data vehicle_cmds vehicle_charging_cmds";
    let token_params = [
        ("grant_type", "authorization_code"),
        ("code", &params.code),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("redirect_uri", &redirect_uri),
        ("code_verifier", pkce_verifier.secret()),
        ("scope", scope),
        ("audience", &audience_url),
    ];

    let token_response = http_client
        .post(token_url)
        .form(&token_params)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to send token exchange request: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to exchange code for token: {}", e)}))
            )
        })?;

    if !token_response.status().is_success() {
        let error_text = token_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Token exchange failed: {}", error_text);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Token exchange failed: {}", error_text)}))
        ));
    }

    let token_data: serde_json::Value = token_response.json().await
        .map_err(|e| {
            error!("Failed to parse token response: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to parse token response: {}", e)}))
            )
        })?;

    let access_token = token_data["access_token"].as_str()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "No access token in response"}))
            )
        })?;

    let refresh_token = token_data["refresh_token"].as_str()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "No refresh token in response"}))
            )
        })?;

    let expires_in = token_data["expires_in"].as_i64()
        .unwrap_or(3600) as i32;

    // Encrypt tokens
    let encrypted_access_token = encrypt(access_token).map_err(|e| {
        error!("Failed to encrypt access token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to encrypt access token: {}", e)}))
        )
    })?;

    let encrypted_refresh_token = encrypt(refresh_token).map_err(|e| {
        error!("Failed to encrypt refresh token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to encrypt refresh token: {}", e)}))
        )
    })?;

    // Use the audience URL as the region (region API has DNS issues)
    // TODO: Re-enable region API when Tesla fixes DNS or we find correct endpoint
    let region = audience_url.clone();
    info!("Using region from OAuth audience: {}", region);

    // Register app in user's region
    let tesla_client = crate::api::tesla::TeslaClient::new_with_region(&region);
    if let Err(e) = tesla_client.register_in_region().await {
        error!("Failed to register in user's region {}: {}", region, e);
        // Continue anyway - registration might already be done
    }

    // Get current timestamp
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Store tokens in database
    let new_tesla = NewTesla {
        user_id,
        encrypted_access_token,
        encrypted_refresh_token,
        status: "active".to_string(),
        last_update: current_time,
        created_on: current_time,
        expires_in,
        region,
    };

    state.user_repository
        .create_tesla_connection(new_tesla)
        .map_err(|e| {
            error!("Failed to store Tesla connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to store Tesla connection: {}", e)}))
            )
        })?;

    // Clean up session
    state.session_store.delete(&Id(session_id)).await
        .map_err(|e| {
            error!("Failed to delete session: {}", e);
            // Non-critical error, continue
            e
        }).ok();

    info!("Tesla OAuth connection successfully established for user {}", user_id);

    // Redirect to frontend
    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    Ok(Redirect::to(&format!(
        "{}/connections?tesla_connected=true",
        frontend_url
    )))
}

// Tesla disconnect endpoint
pub async fn tesla_disconnect(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    info!("Disconnecting Tesla for user {}", auth_user.user_id);

    // Delete connection from database
    state.user_repository
        .delete_tesla_connection(auth_user.user_id)
        .map_err(|e| {
            error!("Failed to delete Tesla connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete Tesla connection: {}", e),
            )
        })?;

    info!("Tesla connection successfully removed for user {}", auth_user.user_id);
    Ok(StatusCode::OK)
}

// Tesla status endpoint
pub async fn tesla_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<TeslaStatusResponse>, (StatusCode, String)> {
    let has_tesla = state.user_repository.has_active_tesla(auth_user.user_id).unwrap_or(false);

    Ok(Json(TeslaStatusResponse { has_tesla }))
}

// Helper function to get valid Tesla access token (with auto-refresh)
pub async fn get_valid_tesla_access_token(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<String, (StatusCode, String)> {
    // Get token info from database
    let (encrypted_access_token, encrypted_refresh_token, expires_in, last_update) = state
        .user_repository
        .get_tesla_token_info(user_id)
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                "No Tesla connection found".to_string(),
            )
        })?;

    // Decrypt tokens
    let access_token = decrypt(&encrypted_access_token).map_err(|e| {
        error!("Failed to decrypt access token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to decrypt access token: {}", e),
        )
    })?;

    let refresh_token = decrypt(&encrypted_refresh_token).map_err(|e| {
        error!("Failed to decrypt refresh token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to decrypt refresh token: {}", e),
        )
    })?;

    // Check if token is expired (with 5 minute buffer)
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let token_expiry = last_update + expires_in;
    let needs_refresh = current_time >= (token_expiry - 300); // 5 minute buffer

    if !needs_refresh {
        return Ok(access_token);
    }

    info!("Tesla access token expired for user {}, refreshing...", user_id);

    // Refresh the token using Tesla's specific token endpoint
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    let token_url = "https://fleet-auth.prd.vn.cloud.tesla.com/oauth2/v3/token";
    let client_id = std::env::var("TESLA_CLIENT_ID")
        .unwrap_or_else(|_| "default-tesla-client-id-for-testing".to_string());
    let client_secret = std::env::var("TESLA_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-tesla-secret-for-testing".to_string());

    // Get the user's region from the database
    let audience_url = state.user_repository.get_tesla_region(user_id).map_err(|e| {
        error!("Failed to get user's Tesla region: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get Tesla region: {}", e),
        )
    })?;

    let scope = "openid offline_access vehicle_device_data vehicle_cmds vehicle_charging_cmds";
    let refresh_params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh_token),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("scope", scope),
        ("audience", &audience_url),
    ];

    let token_response = http_client
        .post(token_url)
        .form(&refresh_params)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to refresh Tesla token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to refresh token: {}", e),
            )
        })?;

    if !token_response.status().is_success() {
        let error_text = token_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Token refresh failed: {}", error_text);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Token refresh failed: {}", error_text),
        ));
    }

    let token_data: serde_json::Value = token_response.json().await
        .map_err(|e| {
            error!("Failed to parse refresh token response: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse token response: {}", e),
            )
        })?;

    let new_access_token = token_data["access_token"].as_str()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "No access token in refresh response".to_string(),
            )
        })?;

    // Tesla returns a new refresh token on refresh
    let new_refresh_token = token_data["refresh_token"].as_str()
        .unwrap_or(&refresh_token); // Keep old if not provided

    let new_expires_in = token_data["expires_in"].as_i64()
        .unwrap_or(3600) as i32;

    // Encrypt new tokens
    let encrypted_access_token = encrypt(new_access_token).map_err(|e| {
        error!("Failed to encrypt new access token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encrypt new access token: {}", e),
        )
    })?;

    let encrypted_refresh_token = encrypt(new_refresh_token).map_err(|e| {
        error!("Failed to encrypt new refresh token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encrypt new refresh token: {}", e),
        )
    })?;

    // Update tokens in database
    state
        .user_repository
        .update_tesla_access_token(
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            new_expires_in,
            current_time,
        )
        .map_err(|e| {
            error!("Failed to update Tesla tokens: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update Tesla tokens: {}", e),
            )
        })?;

    info!("Tesla access token successfully refreshed for user {}", user_id);
    Ok(new_access_token.to_string())
}

// Get partner authentication token (for app-level operations like registration)
// Uses client_credentials grant instead of authorization_code
pub async fn get_partner_access_token() -> Result<String, Box<dyn std::error::Error>> {
    info!("Requesting Tesla partner authentication token");

    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let token_url = "https://fleet-auth.prd.vn.cloud.tesla.com/oauth2/v3/token";
    let client_id = std::env::var("TESLA_CLIENT_ID")
        .unwrap_or_else(|_| "default-tesla-client-id-for-testing".to_string());
    let client_secret = std::env::var("TESLA_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-tesla-secret-for-testing".to_string());
    let audience_url = std::env::var("TESLA_API_BASE")
        .unwrap_or_else(|_| "https://fleet-api.prd.eu.vn.cloud.tesla.com".to_string());

    // Partner token uses client_credentials grant (no user authorization)
    let token_params = [
        ("grant_type", "client_credentials"),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("scope", "openid vehicle_device_data vehicle_cmds vehicle_charging_cmds"),
        ("audience", &audience_url),
    ];

    let token_response = http_client
        .post(token_url)
        .form(&token_params)
        .send()
        .await?;

    if !token_response.status().is_success() {
        let error_text = token_response.text().await?;
        error!("Partner token request failed: {}", error_text);
        return Err(format!("Partner token request failed: {}", error_text).into());
    }

    let token_data: serde_json::Value = token_response.json().await?;

    let access_token = token_data["access_token"]
        .as_str()
        .ok_or("No access token in partner token response")?
        .to_string();

    info!("Successfully obtained Tesla partner authentication token");
    Ok(access_token)
}

// Detect user's Tesla region using their access token
// Returns the fleet_api_base_url for the user's region
pub async fn detect_user_region(access_token: &str) -> Result<String, Box<dyn std::error::Error>> {
    info!("Detecting user's Tesla region");

    let http_client = reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    // Tesla's region detection endpoint - uses global URL without regional subdomain
    let region_url = "https://fleet-api.prd.vn.cloud.tesla.com/api/1/users/region";

    info!("Calling Tesla region API: {}", region_url);

    let response = match http_client
        .get(region_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to send region detection request: {:?}", e);
                return Err(format!("Region API request failed: {}", e).into());
            }
        };

    let status = response.status();
    info!("Region API response status: {}", status);

    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Region detection failed with status {}: {}", status, error_text);
        return Err(format!("Region detection failed ({}): {}", status, error_text).into());
    }

    let response_text = response.text().await?;
    info!("Region API raw response: {}", response_text);

    let region_data: serde_json::Value = serde_json::from_str(&response_text)?;

    let fleet_api_base_url = region_data["response"]["fleet_api_base_url"]
        .as_str()
        .ok_or("No fleet_api_base_url in region response")?
        .to_string();

    info!("Detected user's Tesla region: {}", fleet_api_base_url);
    Ok(fleet_api_base_url)
}

// Serve Tesla public key for vehicle command signing
// This endpoint is required by Tesla at /.well-known/appspecific/com.tesla.3p.public-key.pem
pub async fn serve_tesla_public_key() -> Result<(StatusCode, String), (StatusCode, String)> {
    use crate::utils::tesla_keys;

    match tesla_keys::get_public_key() {
        Ok(public_key) => {
            info!("Serving Tesla public key");
            Ok((StatusCode::OK, public_key))
        }
        Err(e) => {
            error!("Failed to get Tesla public key: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to retrieve public key: {}", e)
            ))
        }
    }
}

// Get virtual key pairing link/QR code for adding key to vehicle
// Users must open this link in their Tesla mobile app to authorize commands
pub async fn get_virtual_key_link(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Generating virtual key pairing link for user {}", auth_user.user_id);

    // Check if user has Tesla connected
    let has_tesla = state.user_repository
        .has_active_tesla(auth_user.user_id)
        .unwrap_or(false);

    if !has_tesla {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "No Tesla connection found. Please connect your Tesla account first."}))
        ));
    }

    // Get domain from environment variable and strip protocol
    let domain = std::env::var("SERVER_URL")
        .or_else(|_| std::env::var("SERVER_URL_OAUTH"))
        .unwrap_or_else(|_| "localhost:3000".to_string());

    // Remove protocol (https:// or http://) if present
    let domain = domain
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string();

    // Generate the Tesla virtual key pairing link
    let pairing_link = format!("https://www.tesla.com/_ak/{}", domain);

    info!("Generated virtual key pairing link: {}", pairing_link);

    Ok(Json(json!({
        "pairing_link": pairing_link,
        "domain": domain,
        "instructions": "Open this link on your mobile device or scan the QR code in your Tesla mobile app to authorize vehicle commands. This is required before you can control your vehicle remotely.",
        "qr_code_url": format!("https://api.qrserver.com/v1/create-qr-code/?size=300x300&data={}", urlencoding::encode(&pairing_link))
    })))
}
