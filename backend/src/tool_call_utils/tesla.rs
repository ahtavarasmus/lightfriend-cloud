use std::sync::Arc;
use serde_json::{json, Value};
use tracing::{info, error};

use crate::{
    api::tesla::TeslaClient,
    handlers::tesla_auth::get_valid_tesla_access_token,
    AppState,
};

// Tool definition for OpenAI function calling
pub fn get_tesla_control_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut properties = HashMap::new();

    properties.insert(
        "command".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Command to execute: 'lock', 'unlock', 'climate_on', 'climate_off', 'remote_start', or 'charge_status'".to_string()),
            enum_values: Some(vec![
                "lock".to_string(),
                "unlock".to_string(),
                "climate_on".to_string(),
                "climate_off".to_string(),
                "remote_start".to_string(),
                "charge_status".to_string(),
            ]),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("control_tesla"),
            description: Some(String::from(
                "Control Tesla vehicle functions: lock/unlock doors, start/stop climate control, remote start driving, or check charge status",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("command")]),
            },
        },
    }
}

// Handle Tesla tool call from AI assistant
pub async fn handle_tesla_command(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    // Parse arguments
    let args_value: Value = match serde_json::from_str(args) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to parse Tesla command args: {}", e);
            return format!("Error: Invalid command format");
        }
    };

    let command = args_value["command"]
        .as_str()
        .unwrap_or("unknown");

    info!("Executing Tesla command '{}' for user {}", command, user_id);

    // Check if user has Tier 2 subscription
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(u)) => u,
        Ok(None) => return "Error: User not found".to_string(),
        Err(e) => {
            error!("Failed to get user: {}", e);
            return "Error: Failed to verify user".to_string();
        }
    };

    if user.sub_tier != Some("tier 2".to_string()) {
        return "Tesla control requires a Tier 2 (Sentinel) subscription. Please upgrade your plan to use this feature.".to_string();
    }

    // Check if user has Tesla connected
    let has_tesla = match state.user_repository.has_active_tesla(user_id) {
        Ok(has) => has,
        Err(e) => {
            error!("Failed to check Tesla connection: {}", e);
            return "Error: Failed to check Tesla connection".to_string();
        }
    };

    if !has_tesla {
        return "You haven't connected your Tesla account yet. Please connect it first in the app settings.".to_string();
    }

    // Get valid access token
    let access_token = match get_valid_tesla_access_token(state, user_id).await {
        Ok(token) => token,
        Err((_, msg)) => {
            error!("Failed to get Tesla access token: {}", msg);
            return format!("Error: Failed to authenticate with Tesla - {}", msg);
        }
    };

    // Get user's Tesla region
    let region = match state.user_repository.get_tesla_region(user_id) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get user's Tesla region: {}", e);
            return "Error: Failed to get your Tesla region settings".to_string();
        }
    };

    // Create Tesla client with user's region and proxy support
    let tesla_client = TeslaClient::new_with_proxy(&region);

    // Get first vehicle (for simplicity, we're using the first vehicle)
    let vehicles = match tesla_client.get_vehicles(&access_token).await {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to get vehicles: {}", e);
            return format!("Failed to get your vehicles: {}", e);
        }
    };

    if vehicles.is_empty() {
        return "No vehicles found on your Tesla account".to_string();
    }

    // Log found vehicles
    info!("Found {} vehicle(s) for user {}", vehicles.len(), user_id);
    for (i, v) in vehicles.iter().enumerate() {
        info!("Vehicle {}: {} (VIN: {}, State: {})", i + 1, v.display_name.as_deref().unwrap_or("Unknown"), v.vin, v.state);
    }

    let vehicle = &vehicles[0];
    let vehicle_id = vehicle.id.to_string();
    let vehicle_vin = &vehicle.vin;  // VIN is required for signed commands
    let vehicle_name = vehicle.display_name.as_deref().unwrap_or("your Tesla");

    info!("Using vehicle: {} (ID: {}, VIN: {}, State: {})", vehicle_name, vehicle_id, vehicle_vin, vehicle.state);

    // Wake up vehicle if it's asleep (except for charge_status which works on sleeping vehicles)
    if command != "charge_status" && vehicle.state != "online" {
        info!("Vehicle is {}, attempting to wake it up...", vehicle.state);
        match tesla_client.wake_up(&access_token, vehicle_vin).await {
            Ok(true) => info!("Vehicle is now online"),
            Ok(false) => {
                return format!("Your {} is asleep and couldn't be woken up. Please try again in a moment.", vehicle_name);
            }
            Err(e) => {
                error!("Failed to wake up vehicle: {}", e);
                return format!("Your {} is asleep. Please wake it up using the Tesla app first.", vehicle_name);
            }
        }
    }

    // Execute command
    match command {
        "lock" => {
            match tesla_client.lock_vehicle(&access_token, vehicle_vin).await {
                Ok(true) => format!("Successfully locked your {}", vehicle_name),
                Ok(false) => format!("Failed to lock your {}", vehicle_name),
                Err(e) => format!("Error locking vehicle: {}", e),
            }
        }
        "unlock" => {
            match tesla_client.unlock_vehicle(&access_token, vehicle_vin).await {
                Ok(true) => format!("Successfully unlocked your {}", vehicle_name),
                Ok(false) => format!("Failed to unlock your {}", vehicle_name),
                Err(e) => format!("Error unlocking vehicle: {}", e),
            }
        }
        "climate_on" => {
            match tesla_client.start_climate(&access_token, vehicle_vin).await {
                Ok(true) => format!("Climate control started in your {}. The car will start warming up or cooling down to your preset temperature.", vehicle_name),
                Ok(false) => format!("Failed to start climate in your {}", vehicle_name),
                Err(e) => format!("Error starting climate: {}", e),
            }
        }
        "climate_off" => {
            match tesla_client.stop_climate(&access_token, vehicle_vin).await {
                Ok(true) => format!("Climate control stopped in your {}", vehicle_name),
                Ok(false) => format!("Failed to stop climate in your {}", vehicle_name),
                Err(e) => format!("Error stopping climate: {}", e),
            }
        }
        "remote_start" => {
            match tesla_client.remote_start(&access_token, vehicle_vin).await {
                Ok(true) => format!("Remote start activated for your {}. You can now drive without the key for 2 minutes. Make sure you're near the vehicle.", vehicle_name),
                Ok(false) => format!("Failed to activate remote start for your {}", vehicle_name),
                Err(e) => format!("Error activating remote start: {}", e),
            }
        }
        "charge_status" => {
            match tesla_client.get_vehicle_data(&access_token, vehicle_vin).await {
                Ok(data) => {
                    if let Some(charge_state) = data.charge_state {
                        let charging_status = if charge_state.charging_state == "Charging" {
                            format!(" Currently charging, {} minutes to full.",
                                charge_state.minutes_to_full_charge.unwrap_or(0))
                        } else {
                            String::new()
                        };

                        format!("Your {} battery is at {}% with {:.0} miles of range. Charge limit set to {}%.{}",
                            vehicle_name,
                            charge_state.battery_level,
                            charge_state.battery_range,
                            charge_state.charge_limit_soc,
                            charging_status
                        )
                    } else {
                        format!("Unable to get charge information for your {}", vehicle_name)
                    }
                }
                Err(e) => format!("Error getting charge status: {}", e),
            }
        }
        _ => {
            format!("Unknown Tesla command: '{}'. Available commands are: lock, unlock, climate_on, climate_off, remote_start, charge_status", command)
        }
    }
}