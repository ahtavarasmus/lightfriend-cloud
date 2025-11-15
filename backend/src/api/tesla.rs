use reqwest;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

// Get Tesla API base URL from env or default to EU region
fn get_tesla_api_base() -> String {
    std::env::var("TESLA_API_BASE").unwrap_or_else(|_| {
        // Default to EU region for development
        "https://fleet-api.prd.eu.vn.cloud.tesla.com".to_string()
    })
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TeslaVehicle {
    pub id: i64,
    pub vehicle_id: i64,
    pub vin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_state: Option<ChargeState>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChargeState {
    pub battery_level: i32,
    pub battery_range: f64,
    pub charge_limit_soc: i32,
    pub charging_state: String,
    pub minutes_to_full_charge: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct VehiclesResponse {
    pub response: Vec<TeslaVehicle>,
    pub count: i32,
}

#[derive(Debug, Deserialize)]
pub struct VehicleDataResponse {
    pub response: TeslaVehicle,
}

#[derive(Debug, Deserialize)]
pub struct CommandResponse {
    pub response: CommandResult,
}

#[derive(Debug, Deserialize)]
pub struct CommandResult {
    pub result: bool,
    pub reason: Option<String>,
}

pub struct TeslaClient {
    client: reqwest::Client,
    base_url: String,
    proxy_url: Option<String>,
    proxy_client: Option<reqwest::Client>,
}

impl TeslaClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: get_tesla_api_base(),
            proxy_url: None,
            proxy_client: None,
        }
    }

    pub fn new_with_region(region: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: region.to_string(),
            proxy_url: None,
            proxy_client: None,
        }
    }

    pub fn new_with_proxy(region: &str) -> Self {
        // Try to get proxy URL from environment
        let proxy_url = std::env::var("TESLA_HTTP_PROXY_URL")
            .ok()
            .filter(|s| !s.is_empty());

        // Build proxy client if URL is provided
        let proxy_client = proxy_url.as_ref().map(|_| {
            // Create client that accepts self-signed certificates (for local proxy)
            // 95 second timeout - slightly higher than proxy's 90s timeout
            reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .timeout(Duration::from_secs(95))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        });

        if let Some(ref url) = proxy_url {
            tracing::info!("Tesla proxy enabled at: {}", url);
        } else {
            tracing::warn!("Tesla proxy not configured - write commands will fail. Set TESLA_HTTP_PROXY_URL environment variable.");
        }

        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: region.to_string(),
            proxy_url,
            proxy_client,
        }
    }

    // Register the app in the current region (required by Tesla)
    // This requires a partner authentication token, not a user token
    pub async fn register_in_region(&self) -> Result<bool, Box<dyn Error>> {
        use crate::handlers::tesla_auth::get_partner_access_token;

        // Get partner token (app-level authentication)
        let partner_token = get_partner_access_token().await?;

        // Get domain from environment variable and strip protocol
        let domain = std::env::var("SERVER_URL")
            .or_else(|_| std::env::var("SERVER_URL_OAUTH"))
            .unwrap_or_else(|_| "localhost:3000".to_string());

        // Remove protocol (https:// or http://) if present
        let domain = domain
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();

        let url = format!("{}/api/1/partner_accounts", self.base_url);

        tracing::info!("Attempting to register app in region: {} with domain: {}", self.base_url, domain);

        // Tesla requires domain in the request body
        let body = serde_json::json!({
            "domain": domain
        });

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", partner_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        tracing::info!("Registration response ({}): {}", status, response_text);

        if status.is_success() {
            tracing::info!("Successfully registered app in region");
            Ok(true)
        } else if response_text.contains("already registered") {
            tracing::info!("App already registered in region");
            Ok(true)
        } else {
            tracing::error!("Failed to register app: {}", response_text);
            Err(format!("Registration failed: {}", response_text).into())
        }
    }

    // Get list of vehicles
    pub async fn get_vehicles(&self, access_token: &str) -> Result<Vec<TeslaVehicle>, Box<dyn Error>> {
        let url = format!("{}/api/1/vehicles", self.base_url);

        let response = self.client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error: {}", error_text).into());
        }

        let vehicles_response: VehiclesResponse = response.json().await?;
        Ok(vehicles_response.response)
    }

    // Get vehicle data including charge state
    pub async fn get_vehicle_data(&self, access_token: &str, vehicle_id: &str) -> Result<TeslaVehicle, Box<dyn Error>> {
        let url = format!("{}/api/1/vehicles/{}/vehicle_data", self.base_url, vehicle_id);

        let response = self.client
            .get(&url)
            .bearer_auth(access_token)
            .query(&[("endpoints", "charge_state")])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error: {}", error_text).into());
        }

        let vehicle_data: VehicleDataResponse = response.json().await?;
        Ok(vehicle_data.response)
    }

    // Lock vehicle
    pub async fn lock_vehicle(&self, access_token: &str, vehicle_id: &str) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "door_lock").await
    }

    // Unlock vehicle
    pub async fn unlock_vehicle(&self, access_token: &str, vehicle_id: &str) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "door_unlock").await
    }

    // Start climate preconditioning
    pub async fn start_climate(&self, access_token: &str, vehicle_id: &str) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "auto_conditioning_start").await
    }

    // Stop climate preconditioning
    pub async fn stop_climate(&self, access_token: &str, vehicle_id: &str) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "auto_conditioning_stop").await
    }

    // Remote start drive
    pub async fn remote_start(&self, access_token: &str, vehicle_id: &str) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "remote_start_drive").await
    }

    // Generic command sender
    async fn send_command(&self, access_token: &str, vehicle_id: &str, command: &str) -> Result<bool, Box<dyn Error>> {
        // Use proxy for signed commands if available, otherwise fall back to direct API
        let (client, base_url) = if let (Some(proxy_client), Some(proxy_url)) = (&self.proxy_client, &self.proxy_url) {
            tracing::info!("Sending signed command '{}' via proxy to vehicle {}", command, vehicle_id);
            (proxy_client, proxy_url.as_str())
        } else {
            tracing::warn!("Proxy not available - attempting direct command (will likely fail with Protocol error)");
            (&self.client, self.base_url.as_str())
        };

        let url = format!("{}/api/1/vehicles/{}/command/{}", base_url, vehicle_id, command);

        let response = client
            .post(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error for command {}: {}", command, error_text).into());
        }

        let command_response: CommandResponse = response.json().await?;

        if !command_response.response.result {
            let reason = command_response.response.reason.unwrap_or_else(|| "Unknown error".to_string());
            return Err(format!("Command {} failed: {}", command, reason).into());
        }

        Ok(command_response.response.result)
    }

    // Wake up vehicle (if needed before sending commands)
    pub async fn wake_up(&self, access_token: &str, vehicle_id: &str) -> Result<bool, Box<dyn Error>> {
        tracing::info!("Waking up vehicle {}", vehicle_id);

        // Use proxy for signed commands if available, otherwise fall back to direct API
        let (client, base_url) = if let (Some(proxy_client), Some(proxy_url)) = (&self.proxy_client, &self.proxy_url) {
            (proxy_client, proxy_url.as_str())
        } else {
            (&self.client, self.base_url.as_str())
        };

        let url = format!("{}/api/1/vehicles/{}/wake_up", base_url, vehicle_id);

        let response = client
            .post(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await?;
            tracing::error!("Wake-up failed with status {}: {}", status, error_text);
            return Err(format!("Failed to wake vehicle: {}", error_text).into());
        }

        // Get response as JSON value first to handle different response formats
        let response_text = response.text().await?;
        tracing::debug!("Wake-up response: {}", response_text);

        let response_json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse wake-up response: {}", e))?;

        // Check if vehicle is online
        let state = response_json["response"]["state"]
            .as_str()
            .unwrap_or("unknown");

        tracing::info!("Vehicle state after wake-up: {}", state);

        // Give vehicle time to fully wake up if it's waking
        if state == "waking" || state == "asleep" {
            tracing::info!("Vehicle is waking up, waiting a moment...");
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            // Consider it successful if we initiated wake-up
            Ok(true)
        } else {
            Ok(state == "online")
        }
    }
}