use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::functional::UseStateHandle;
use crate::utils::api::Api;
use wasm_bindgen::JsCast;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct VehicleInfo {
    pub vin: String,
    pub id: String,
    pub vehicle_id: String,
    pub name: String,
    pub state: String,
    pub selected: bool,
    pub paired: bool,
}

#[derive(Properties, PartialEq)]
pub struct TeslaConnectProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
}

#[function_component(TeslaConnect)]
pub fn tesla_connect(props: &TeslaConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let tesla_connected = use_state(|| false);
    let connecting = use_state(|| false);
    let pairing_link = use_state(|| None::<String>);
    let qr_code_url = use_state(|| None::<String>);
    let show_pairing = use_state(|| false);
    let lock_loading = use_state(|| false);
    let climate_loading = use_state(|| false);
    let defrost_loading = use_state(|| false);
    let command_result = use_state(|| None::<String>);
    let battery_level = use_state(|| None::<i32>);
    let battery_range = use_state(|| None::<f64>);
    let charging_state = use_state(|| None::<String>);
    let battery_loading = use_state(|| false);
    let is_locked = use_state(|| None::<bool>);
    let inside_temp = use_state(|| None::<f64>);
    let outside_temp = use_state(|| None::<f64>);
    let is_climate_on = use_state(|| None::<bool>);
    let is_front_defroster_on = use_state(|| None::<bool>);
    let is_rear_defroster_on = use_state(|| None::<bool>);

    // Vehicle selection state
    let available_vehicles = use_state(|| Vec::<VehicleInfo>::new());
    let selected_vehicle_name = use_state(|| None::<String>);
    let show_vehicle_selector = use_state(|| false);
    let vehicle_loading = use_state(|| false);

    // Per-vehicle pairing state
    let vehicle_pairing_vin = use_state(|| None::<String>); // VIN of vehicle whose pairing is shown
    let vehicle_pairing_link = use_state(|| None::<String>);
    let vehicle_qr_code_url = use_state(|| None::<String>);

    // Disconnect confirmation modal state
    let show_disconnect_modal = use_state(|| false);
    let is_disconnecting = use_state(|| false);

    // Check Tesla connection status on mount
    {
        let tesla_connected = tesla_connected.clone();
        let error = error.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Api::get("/api/auth/tesla/status")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(status) = response.json::<serde_json::Value>().await {
                                    if let Some(has_tesla) = status["has_tesla"].as_bool() {
                                        tesla_connected.set(has_tesla);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("Failed to check Tesla status: {}", e)));
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    // Fetch virtual key pairing info when connected
    {
        let tesla_connected = tesla_connected.clone();
        let pairing_link = pairing_link.clone();
        let qr_code_url = qr_code_url.clone();
        let show_pairing = show_pairing.clone();
        let error = error.clone();

        use_effect_with_deps(
            move |connected| {
                if **connected {
                    spawn_local(async move {
                        match Api::get("/api/auth/tesla/virtual-key")
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(link) = data["pairing_link"].as_str() {
                                            pairing_link.set(Some(link.to_string()));
                                        }
                                        if let Some(qr_url) = data["qr_code_url"].as_str() {
                                            qr_code_url.set(Some(qr_url.to_string()));
                                        }

                                        // Don't auto-show pairing instructions - user can click to see them
                                        show_pairing.set(false);
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Failed to fetch pairing info: {}", e)));
                            }
                        }
                    });
                }
                || ()
            },
            tesla_connected.clone(),
        );
    }

    // Fetch available vehicles when connected
    {
        let tesla_connected = tesla_connected.clone();
        let available_vehicles = available_vehicles.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();

        use_effect_with_deps(
            move |connected| {
                if **connected {
                    spawn_local(async move {
                        match Api::get("/api/tesla/vehicles")
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(vehicles_array) = data["vehicles"].as_array() {
                                            let vehicles: Vec<VehicleInfo> = vehicles_array
                                                .iter()
                                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                                .collect();

                                            // Find selected vehicle name
                                            let selected_name = vehicles.iter()
                                                .find(|v| v.selected)
                                                .map(|v| v.name.clone());

                                            available_vehicles.set(vehicles);
                                            selected_vehicle_name.set(selected_name);
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                // Silently fail - vehicles list is optional
                            }
                        }
                    });
                }
                || ()
            },
            tesla_connected.clone(),
        );
    }

    // Clear all state when disconnected (handles edge cases like external disconnects)
    {
        let tesla_connected = tesla_connected.clone();
        let pairing_link = pairing_link.clone();
        let qr_code_url = qr_code_url.clone();
        let show_pairing = show_pairing.clone();
        let battery_level = battery_level.clone();
        let battery_range = battery_range.clone();
        let charging_state = charging_state.clone();
        let is_locked = is_locked.clone();
        let inside_temp = inside_temp.clone();
        let outside_temp = outside_temp.clone();
        let is_climate_on = is_climate_on.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let available_vehicles = available_vehicles.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();
        let show_vehicle_selector = show_vehicle_selector.clone();
        let vehicle_pairing_vin = vehicle_pairing_vin.clone();
        let vehicle_pairing_link = vehicle_pairing_link.clone();
        let vehicle_qr_code_url = vehicle_qr_code_url.clone();
        let command_result = command_result.clone();

        use_effect_with_deps(
            move |connected| {
                if !**connected {
                    // Clear all Tesla-related state when disconnected
                    pairing_link.set(None);
                    qr_code_url.set(None);
                    show_pairing.set(false);
                    battery_level.set(None);
                    battery_range.set(None);
                    charging_state.set(None);
                    is_locked.set(None);
                    inside_temp.set(None);
                    outside_temp.set(None);
                    is_climate_on.set(None);
                    is_front_defroster_on.set(None);
                    is_rear_defroster_on.set(None);
                    available_vehicles.set(Vec::new());
                    selected_vehicle_name.set(None);
                    show_vehicle_selector.set(false);
                    vehicle_pairing_vin.set(None);
                    vehicle_pairing_link.set(None);
                    vehicle_qr_code_url.set(None);
                    command_result.set(None);
                }
                || ()
            },
            tesla_connected.clone(),
        );
    }

    // Handle connect button click
    let onclick_connect = {
        let error = error.clone();
        let connecting = connecting.clone();
        Callback::from(move |_: MouseEvent| {
            let error = error.clone();
            let connecting = connecting.clone();

            connecting.set(true);
            spawn_local(async move {
                match Api::get("/api/auth/tesla/login")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(auth_url) = data["auth_url"].as_str() {
                                    if let Some(window) = window() {
                                        let _ = window.location().set_href(auth_url);
                                    }
                                }
                            }
                        } else {
                            if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                if let Some(error_msg) = error_data["error"].as_str() {
                                    error.set(Some(error_msg.to_string()));
                                }
                            } else {
                                error.set(Some("Failed to initiate Tesla login".to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                connecting.set(false);
            });
        })
    };

    // Handle disconnect button click - shows confirmation modal
    let onclick_disconnect = {
        let show_disconnect_modal = show_disconnect_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_disconnect_modal.set(true);
        })
    };

    // Handle confirmed disconnect
    let handle_confirmed_disconnect = {
        let tesla_connected = tesla_connected.clone();
        let error = error.clone();
        let pairing_link = pairing_link.clone();
        let qr_code_url = qr_code_url.clone();
        let show_pairing = show_pairing.clone();
        let battery_level = battery_level.clone();
        let battery_range = battery_range.clone();
        let charging_state = charging_state.clone();
        let is_locked = is_locked.clone();
        let inside_temp = inside_temp.clone();
        let outside_temp = outside_temp.clone();
        let is_climate_on = is_climate_on.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let available_vehicles = available_vehicles.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();
        let show_vehicle_selector = show_vehicle_selector.clone();
        let vehicle_pairing_vin = vehicle_pairing_vin.clone();
        let vehicle_pairing_link = vehicle_pairing_link.clone();
        let vehicle_qr_code_url = vehicle_qr_code_url.clone();
        let command_result = command_result.clone();
        let show_disconnect_modal = show_disconnect_modal.clone();
        let is_disconnecting = is_disconnecting.clone();

        Callback::from(move |_: MouseEvent| {
            let tesla_connected = tesla_connected.clone();
            let error = error.clone();
            let pairing_link = pairing_link.clone();
            let qr_code_url = qr_code_url.clone();
            let show_pairing = show_pairing.clone();
            let battery_level = battery_level.clone();
            let battery_range = battery_range.clone();
            let charging_state = charging_state.clone();
            let is_locked = is_locked.clone();
            let inside_temp = inside_temp.clone();
            let outside_temp = outside_temp.clone();
            let is_climate_on = is_climate_on.clone();
            let is_front_defroster_on = is_front_defroster_on.clone();
            let is_rear_defroster_on = is_rear_defroster_on.clone();
            let available_vehicles = available_vehicles.clone();
            let selected_vehicle_name = selected_vehicle_name.clone();
            let show_vehicle_selector = show_vehicle_selector.clone();
            let vehicle_pairing_vin = vehicle_pairing_vin.clone();
            let vehicle_pairing_link = vehicle_pairing_link.clone();
            let vehicle_qr_code_url = vehicle_qr_code_url.clone();
            let command_result = command_result.clone();
            let show_disconnect_modal = show_disconnect_modal.clone();
            let is_disconnecting = is_disconnecting.clone();

            is_disconnecting.set(true);

            spawn_local(async move {
                let request = Api::delete("/api/auth/tesla/connection")
                    .send()
                    .await;
                match request {
                    Ok(response) => {
                        if response.ok() {
                            // Clear all Tesla-related state
                            tesla_connected.set(false);
                            pairing_link.set(None);
                            qr_code_url.set(None);
                            show_pairing.set(false);
                            battery_level.set(None);
                            battery_range.set(None);
                            charging_state.set(None);
                            is_locked.set(None);
                            inside_temp.set(None);
                            outside_temp.set(None);
                            is_climate_on.set(None);
                            is_front_defroster_on.set(None);
                            is_rear_defroster_on.set(None);
                            available_vehicles.set(Vec::new());
                            selected_vehicle_name.set(None);
                            show_vehicle_selector.set(false);
                            vehicle_pairing_vin.set(None);
                            vehicle_pairing_link.set(None);
                            vehicle_qr_code_url.set(None);
                            command_result.set(None);
                            show_disconnect_modal.set(false);
                            is_disconnecting.set(false);
                        } else {
                            if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                    error.set(Some(error_msg.to_string()));
                                } else {
                                    error.set(Some(format!("Failed to delete connection: {}", response.status())));
                                }
                            }
                            is_disconnecting.set(false);
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                        is_disconnecting.set(false);
                    }
                }
            });
        })
    };

    // Handle lock/unlock button click
    let handle_lock = {
        let lock_loading = lock_loading.clone();
        let command_result = command_result.clone();
        let is_locked = is_locked.clone();

        Callback::from(move |_: MouseEvent| {
            let lock_loading = lock_loading.clone();
            let command_result = command_result.clone();
            let is_locked = is_locked.clone();

            lock_loading.set(true);
            command_result.set(None);

            spawn_local(async move {
                // Determine command based on current lock state
                let command = match *is_locked {
                    Some(true) => "unlock",  // If locked, unlock it
                    Some(false) => "lock",   // If unlocked, lock it
                    None => "lock",          // If unknown, default to lock
                };

                let body = serde_json::json!({
                    "command": command
                });

                let request = match Api::post("/api/tesla/command")
                    .json(&body)
                {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed to create request: {}", e)));
                        lock_loading.set(false);
                        return;
                    }
                };

                match request {
                    Ok(response) => {
                        if response.ok() {
                            // Update state optimistically after successful command
                            match command {
                                "lock" => is_locked.set(Some(true)),
                                "unlock" => is_locked.set(Some(false)),
                                _ => {}
                            }

                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                                    command_result.set(Some(msg.to_string()));
                                }
                            }
                        } else {
                            command_result.set(Some("Failed to execute lock command".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Network error: {}", e)));
                    }
                }
                lock_loading.set(false);
            });
        })
    };

    // Handle climate button click
    let handle_climate = {
        let climate_loading = climate_loading.clone();
        let command_result = command_result.clone();
        let is_climate_on = is_climate_on.clone();

        Callback::from(move |_: MouseEvent| {
            let climate_loading = climate_loading.clone();
            let command_result = command_result.clone();
            let is_climate_on = is_climate_on.clone();

            climate_loading.set(true);
            command_result.set(None);

            spawn_local(async move {
                // Determine command based on current climate state
                let command = match *is_climate_on {
                    Some(true) => "climate_off",  // If on, turn it off
                    Some(false) => "climate_on",  // If off, turn it on
                    None => "climate_on",         // If unknown, default to on
                };

                let body = serde_json::json!({
                    "command": command
                });

                let request = match Api::post("/api/tesla/command")
                    .json(&body)
                {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed to create request: {}", e)));
                        climate_loading.set(false);
                        return;
                    }
                };

                match request {
                    Ok(response) => {
                        if response.ok() {
                            // Update state optimistically after successful command
                            match command {
                                "climate_on" => is_climate_on.set(Some(true)),
                                "climate_off" => is_climate_on.set(Some(false)),
                                _ => {}
                            }

                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                                    command_result.set(Some(msg.to_string()));
                                }
                            }
                        } else {
                            command_result.set(Some("Failed to execute climate command".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Network error: {}", e)));
                    }
                }
                climate_loading.set(false);
            });
        })
    };

    // Handle defrost button click
    let handle_defrost = {
        let defrost_loading = defrost_loading.clone();
        let command_result = command_result.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let is_climate_on = is_climate_on.clone();

        Callback::from(move |_: MouseEvent| {
            let defrost_loading = defrost_loading.clone();
            let command_result = command_result.clone();
            let is_front_defroster_on = is_front_defroster_on.clone();
            let is_rear_defroster_on = is_rear_defroster_on.clone();
            let is_climate_on = is_climate_on.clone();

            defrost_loading.set(true);
            command_result.set(None);

            spawn_local(async move {
                // Determine command based on current defrost state
                let front_on = (*is_front_defroster_on).unwrap_or(false);
                let rear_on = (*is_rear_defroster_on).unwrap_or(false);
                let any_defrost_on = front_on || rear_on;

                // If defrost is on, turn off climate (which turns off defrost)
                // If defrost is off, activate defrost
                let command = if any_defrost_on {
                    "climate_off"  // Turn off climate to deactivate defrost
                } else {
                    "defrost"      // Activate max defrost
                };

                let body = serde_json::json!({
                    "command": command
                });

                let request = match Api::post("/api/tesla/command")
                    .json(&body)
                {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed to create request: {}", e)));
                        defrost_loading.set(false);
                        return;
                    }
                };

                            match request {
                                Ok(response) => {
                                    if response.ok() {
                                        // Update state optimistically based on command
                                        match command {
                                            "defrost" => {
                                                // Defrost activates both front and rear defrosters and turns on climate
                                                is_front_defroster_on.set(Some(true));
                                                is_rear_defroster_on.set(Some(true));
                                                is_climate_on.set(Some(true));
                                            }
                                            "climate_off" => {
                                                // Turning off climate deactivates all defrosters
                                                is_front_defroster_on.set(Some(false));
                                                is_rear_defroster_on.set(Some(false));
                                                is_climate_on.set(Some(false));
                                            }
                                            _ => {}
                                        }

                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                                                command_result.set(Some(msg.to_string()));
                                            }
                                        }
                                    } else {
                                        command_result.set(Some("Failed to execute defrost command".to_string()));
                                    }
                                }
                                Err(e) => {
                                    command_result.set(Some(format!("Network error: {}", e)));
                                }
                            }
                defrost_loading.set(false);
            });
        })
    };

    // Handle vehicle selection
    let handle_vehicle_select = {
        let vehicle_loading = vehicle_loading.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();
        let command_result = command_result.clone();
        let show_vehicle_selector = show_vehicle_selector.clone();
        let available_vehicles = available_vehicles.clone();

        Callback::from(move |vehicle: VehicleInfo| {
            let vehicle_loading = vehicle_loading.clone();
            let selected_vehicle_name = selected_vehicle_name.clone();
            let command_result = command_result.clone();
            let show_vehicle_selector = show_vehicle_selector.clone();
            let available_vehicles = available_vehicles.clone();
            let vehicle_clone = vehicle.clone();

            vehicle_loading.set(true);

            spawn_local(async move {
                let body = serde_json::json!({
                    "vin": vehicle_clone.vin,
                    "name": vehicle_clone.name,
                    "vehicle_id": vehicle_clone.vehicle_id,
                });

                let request = match Api::post("/api/tesla/select-vehicle")
                    .json(&body)
                {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed to select vehicle: {}", e)));
                        vehicle_loading.set(false);
                        return;
                    }
                };

                match request {
                    Ok(response) => {
                        if response.ok() {
                            // Update local state
                            selected_vehicle_name.set(Some(vehicle_clone.name.clone()));

                            // Update selected flag in vehicles list
                            let mut vehicles = (*available_vehicles).clone();
                            for v in vehicles.iter_mut() {
                                v.selected = v.vin == vehicle_clone.vin;
                            }
                            available_vehicles.set(vehicles);

                            // Close selector
                            show_vehicle_selector.set(false);

                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                                    command_result.set(Some(msg.to_string()));
                                }
                            }
                        } else {
                            command_result.set(Some("Failed to select vehicle".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Network error: {}", e)));
                    }
                }
                vehicle_loading.set(false);
            });
        })
    };

    // Handle showing vehicle-specific pairing QR code
    let handle_show_vehicle_pairing = {
        let vehicle_pairing_vin = vehicle_pairing_vin.clone();
        let vehicle_pairing_link = vehicle_pairing_link.clone();
        let vehicle_qr_code_url = vehicle_qr_code_url.clone();
        let command_result = command_result.clone();

        Callback::from(move |vin: String| {
            let vehicle_pairing_vin = vehicle_pairing_vin.clone();
            let vehicle_pairing_link = vehicle_pairing_link.clone();
            let vehicle_qr_code_url = vehicle_qr_code_url.clone();
            let command_result = command_result.clone();
            let vin_clone = vin.clone();

            spawn_local(async move {
                match Api::get(&format!("/api/auth/tesla/virtual-key?vin={}", urlencoding::encode(&vin_clone)))
                    .send()
                    .await
                {
                                Ok(response) => {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(link) = data["pairing_link"].as_str() {
                                                vehicle_pairing_link.set(Some(link.to_string()));
                                            }
                                            if let Some(qr_url) = data["qr_code_url"].as_str() {
                                                vehicle_qr_code_url.set(Some(qr_url.to_string()));
                                            }
                                            vehicle_pairing_vin.set(Some(vin_clone));
                                        }
                                    } else {
                                        command_result.set(Some("Failed to fetch pairing info".to_string()));
                                    }
                                }
                    Err(e) => {
                        command_result.set(Some(format!("Failed to fetch pairing info: {}", e)));
                    }
                }
            })
        })
    };

    // Handle pairing dismiss button click
    let onclick_dismiss_pairing = {
        let show_pairing = show_pairing.clone();
        Callback::from(move |_: MouseEvent| {
            show_pairing.set(false);
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("tesla_pairing_dismissed", "true");
                }
            }
        })
    };

    // Handle battery refresh button click
    let handle_battery_refresh = {
        let battery_loading = battery_loading.clone();
        let battery_level = battery_level.clone();
        let battery_range = battery_range.clone();
        let charging_state = charging_state.clone();
        let is_locked = is_locked.clone();
        let inside_temp = inside_temp.clone();
        let outside_temp = outside_temp.clone();
        let is_climate_on = is_climate_on.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let available_vehicles = available_vehicles.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();

        Callback::from(move |_: MouseEvent| {
            let battery_loading = battery_loading.clone();
            let battery_level = battery_level.clone();
            let battery_range = battery_range.clone();
            let charging_state = charging_state.clone();
            let is_locked = is_locked.clone();
            let inside_temp = inside_temp.clone();
            let outside_temp = outside_temp.clone();
            let is_climate_on = is_climate_on.clone();
            let is_front_defroster_on = is_front_defroster_on.clone();
            let is_rear_defroster_on = is_rear_defroster_on.clone();
            let available_vehicles = available_vehicles.clone();
            let selected_vehicle_name = selected_vehicle_name.clone();

            battery_loading.set(true);

            spawn_local(async move {
                let request = Api::get("/api/tesla/battery-status")
                    .send()
                    .await;

                match request {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(level) = data["battery_level"].as_i64() {
                                    battery_level.set(Some(level as i32));
                                }
                                if let Some(range) = data["battery_range"].as_f64() {
                                    battery_range.set(Some(range));
                                }
                                if let Some(state) = data["charging_state"].as_str() {
                                    charging_state.set(Some(state.to_string()));
                                }
                                if let Some(locked) = data["locked"].as_bool() {
                                    is_locked.set(Some(locked));
                                }
                                if let Some(temp) = data["inside_temp"].as_f64() {
                                    inside_temp.set(Some(temp));
                                }
                                if let Some(temp) = data["outside_temp"].as_f64() {
                                    outside_temp.set(Some(temp));
                                }
                                if let Some(climate) = data["is_climate_on"].as_bool() {
                                    is_climate_on.set(Some(climate));
                                }
                                if let Some(front_defrost) = data["is_front_defroster_on"].as_bool() {
                                    is_front_defroster_on.set(Some(front_defrost));
                                }
                                if let Some(rear_defrost) = data["is_rear_defroster_on"].as_bool() {
                                    is_rear_defroster_on.set(Some(rear_defrost));
                                }
                            }

                            // Also fetch vehicles list to update selected vehicle
                            let available_vehicles = available_vehicles.clone();
                            let selected_vehicle_name = selected_vehicle_name.clone();
                            spawn_local(async move {
                                if let Ok(vehicles_response) = Api::get("/api/tesla/vehicles")
                                    .send()
                                    .await
                                {
                                    if vehicles_response.ok() {
                                        if let Ok(data) = vehicles_response.json::<serde_json::Value>().await {
                                            if let Some(vehicles_array) = data["vehicles"].as_array() {
                                                let vehicles: Vec<VehicleInfo> = vehicles_array
                                                    .iter()
                                                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                                    .collect();

                                                // Find selected vehicle name
                                                let selected_name = vehicles.iter()
                                                    .find(|v| v.selected)
                                                    .map(|v| v.name.clone());

                                                available_vehicles.set(vehicles);
                                                selected_vehicle_name.set(selected_name);
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                    Err(_e) => {
                        // Error handling - could set an error state here
                    }
                }
                battery_loading.set(false);
            });
        })
    };

    html! {
        <div class="service-item">
            <div class="service-header">
                <div class="service-name">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/b/bb/Tesla_T_symbol.svg" alt="Tesla" width="24" height="24"/>
                    {"Tesla"}
                </div>
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("tesla-info"))
                    {
                        let display = element.get_attribute("style")
                            .unwrap_or_else(|| "display: none".to_string());

                        if display.contains("none") {
                            let _ = element.set_attribute("style", "display: block");
                        } else {
                            let _ = element.set_attribute("style", "display: none");
                        }
                    }
                })}>
                    {"ⓘ"}
                </button>
                if *tesla_connected {
                    <span class="service-status">{"Connected ✓"}</span>
                }
            </div>
            <p class="service-description">
                {"Control your Tesla vehicle remotely through SMS or voice calls."}
            </p>
            <div id="tesla-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"Available Commands"}</h5>
                    <ul>
                        <li>{"Lock/Unlock: Secure or open your vehicle remotely"}</li>
                        <li>{"Climate Control: Start or stop preconditioning"}</li>
                        <li>{"Remote Start: Enable keyless driving for 2 minutes"}</li>
                        <li>{"Charge Status: Check battery level and range"}</li>
                    </ul>
                </div>
                <div class="info-subsection">
                    <h5>{"Example Commands"}</h5>
                    <ul>
                        <li>{"\"Lock my Tesla\""}</li>
                        <li>{"\"Start climate control in my car\""}</li>
                        <li>{"\"What's my Tesla's battery level?\""}</li>
                        <li>{"\"Precondition my vehicle\""}</li>
                    </ul>
                </div>
                <p class="info-note">
                    {"Your Tesla credentials are encrypted and never stored in plain text."}
                </p>
            </div>

            if let Some(error_msg) = (*error).as_ref() {
                <div class="error-message">
                    {error_msg}
                </div>
            }

            // Check subscription tier
            if props.sub_tier == Some("tier 2".to_string()) || props.sub_tier == Some("tier 3".to_string()) {
                if !*tesla_connected {
                    <button
                        class="connect-button"
                        onclick={onclick_connect}
                        disabled={*connecting}
                    >
                        {if *connecting { "Connecting..." } else { "Connect Tesla" }}
                    </button>
                } else {
                    <div class="connection-actions">
                        // Unified Vehicle Status & Controls section
                        <div style="
                            background: rgba(0, 0, 0, 0.2);
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            border-radius: 12px;
                            padding: 1.5rem;
                            margin: 15px 0;
                        ">
                            <h4 style="margin-top: 0; color: #7EB2FF; font-size: 16px; font-weight: 500;">{"Vehicle Status & Controls"}</h4>

                            // Vehicle selector - show selected vehicle, only show change button if multiple vehicles
                            {
                                if !available_vehicles.is_empty() {
                                    let show_vehicle_selector_clone = show_vehicle_selector.clone();
                                    let has_multiple_vehicles = available_vehicles.len() > 1;
                                    let current_vin = available_vehicles.iter()
                                        .find(|v| v.selected)
                                        .map(|v| v.vin.clone());
                                    let is_paired = available_vehicles.iter()
                                        .find(|v| v.selected)
                                        .map(|v| v.paired)
                                        .unwrap_or(false);
                                    let handle_pairing = handle_show_vehicle_pairing.clone();
                                    let current_vin_for_badge = current_vin.clone();
                                    html! {
                                        <div style="margin-bottom: 15px;">
                                            <div style="
                                                display: flex;
                                                align-items: center;
                                                justify-content: space-between;
                                                padding: 10px 15px;
                                                background: rgba(30, 144, 255, 0.1);
                                                border: 1px solid rgba(30, 144, 255, 0.2);
                                                border-radius: 8px;
                                            ">
                                                <div style="display: flex; align-items: center; gap: 10px;">
                                                    <div>
                                                        <div style="color: #999; font-size: 12px;">{"Selected Vehicle"}</div>
                                                        <div style="color: #fff; font-size: 14px; font-weight: 500;">
                                                            {selected_vehicle_name.as_ref().map(|n| n.as_str()).unwrap_or("None")}
                                                        </div>
                                                    </div>
                                                    {
                                                        // Show pairing status badge
                                                        if is_paired {
                                                            html! {
                                                                <div style="
                                                                    padding: 4px 10px;
                                                                    background: rgba(105, 240, 174, 0.15);
                                                                    color: #69f0ae;
                                                                    border: 1px solid rgba(105, 240, 174, 0.3);
                                                                    border-radius: 12px;
                                                                    font-size: 11px;
                                                                    font-weight: 600;
                                                                    white-space: nowrap;
                                                                ">
                                                                    {"✓ Paired"}
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {
                                                                <button
                                                                    onclick={Callback::from(move |e: MouseEvent| {
                                                                        e.stop_propagation();
                                                                        if let Some(vin) = current_vin_for_badge.clone() {
                                                                            handle_pairing.emit(vin);
                                                                        }
                                                                    })}
                                                                    style="
                                                                        padding: 4px 10px;
                                                                        background: rgba(255, 152, 0, 0.15);
                                                                        color: #ff9800;
                                                                        border: 1px solid rgba(255, 152, 0, 0.3);
                                                                        border-radius: 12px;
                                                                        font-size: 11px;
                                                                        font-weight: 600;
                                                                        cursor: pointer;
                                                                        white-space: nowrap;
                                                                    "
                                                                >
                                                                    {"⚠️ Setup Key"}
                                                                </button>
                                                            }
                                                        }
                                                    }
                                                </div>
                                                {
                                                    if has_multiple_vehicles {
                                                        html! {
                                                            <button
                                                                onclick={
                                                                    let show_vehicle_selector_inner = show_vehicle_selector_clone.clone();
                                                                    Callback::from(move |_| {
                                                                        show_vehicle_selector_inner.set(!*show_vehicle_selector_inner);
                                                                    })
                                                                }
                                                                style="
                                                                    padding: 8px 16px;
                                                                    background: rgba(30, 144, 255, 0.2);
                                                                    color: #7EB2FF;
                                                                    border: 1px solid rgba(30, 144, 255, 0.3);
                                                                    border-radius: 6px;
                                                                    cursor: pointer;
                                                                    font-size: 13px;
                                                                "
                                                            >
                                                                {"Change Vehicle"}
                                                            </button>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </div>

                                            // Vehicle selector dropdown (only if multiple vehicles)
                                            {
                                                if has_multiple_vehicles && *show_vehicle_selector {
                                                    html! {
                                                        <div style="
                                                            margin-top: 10px;
                                                            padding: 15px;
                                                            background: rgba(0, 0, 0, 0.3);
                                                            border: 1px solid rgba(30, 144, 255, 0.2);
                                                            border-radius: 8px;
                                                        ">
                                                            <div style="color: #7EB2FF; font-size: 13px; font-weight: 600; margin-bottom: 10px;">
                                                                {"Select Vehicle:"}
                                                            </div>
                                                            {
                                                                available_vehicles.iter().map(|vehicle| {
                                                                    let is_selected = vehicle.selected;
                                                                    let handle_select = handle_vehicle_select.clone();
                                                                    let vehicle_clone = vehicle.clone();

                                                                    html! {
                                                                        <div
                                                                            key={vehicle.vin.clone()}
                                                                            onclick={Callback::from(move |_| {
                                                                                handle_select.emit(vehicle_clone.clone());
                                                                            })}
                                                                            style={format!("
                                                                                padding: 12px;
                                                                                margin-bottom: 8px;
                                                                                background: {};
                                                                                border: 1px solid {};
                                                                                border-radius: 6px;
                                                                                cursor: pointer;
                                                                                transition: all 0.2s;
                                                                            ",
                                                                                if is_selected { "rgba(30, 144, 255, 0.2)" } else { "rgba(0, 0, 0, 0.2)" },
                                                                                if is_selected { "rgba(30, 144, 255, 0.4)" } else { "rgba(30, 144, 255, 0.2)" }
                                                                            )}
                                                                        >
                                                                            <div style="display: flex; align-items: center; justify-content: space-between;">
                                                                                <div>
                                                                                    <div style="color: #fff; font-size: 14px; font-weight: 500;">
                                                                                        {&vehicle.name}
                                                                                    </div>
                                                                                    <div style="color: #999; font-size: 12px;">
                                                                                        {format!("VIN: ...{}", &vehicle.vin[vehicle.vin.len()-6..])}
                                                                                    </div>
                                                                                    <div style={format!("color: {}; font-size: 11px;",
                                                                                        if vehicle.state == "online" { "#69f0ae" } else { "#999" }
                                                                                    )}>
                                                                                        {format!("State: {}", vehicle.state)}
                                                                                    </div>
                                                                                </div>
                                                                                {
                                                                                    if is_selected {
                                                                                        html! {
                                                                                            <div style="color: #69f0ae; font-size: 18px;">{"✓"}</div>
                                                                                        }
                                                                                    } else {
                                                                                        html! {}
                                                                                    }
                                                                                }
                                                                            </div>
                                                                        </div>
                                                                    }
                                                                }).collect::<Html>()
                                                            }
                                                        </div>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }

                                            // Virtual key QR code display (shown when badge is clicked)
                                            {
                                                if let Some(vin) = current_vin {
                                                    let show_pairing_for_this = (*vehicle_pairing_vin).as_ref().map(|v| v == &vin).unwrap_or(false);

                                                    if show_pairing_for_this {
                                                        html! {
                                                            <div style="
                                                                margin-top: 10px;
                                                                padding: 20px;
                                                                background: rgba(0, 0, 0, 0.4);
                                                                border: 1px solid rgba(126, 178, 255, 0.3);
                                                                border-radius: 8px;
                                                            ">
                                                                <div style="color: #7EB2FF; font-size: 14px; font-weight: 600; margin-bottom: 15px;">
                                                                    {"Virtual Key Setup Instructions"}
                                                                </div>
                                                                <div style="color: #ccc; font-size: 13px; line-height: 1.6; margin-bottom: 15px;">
                                                                    <div style="margin-bottom: 10px;">
                                                                        {"To enable vehicle commands, you must pair a virtual key with this vehicle:"}
                                                                    </div>
                                                                    <ol style="margin: 10px 0; padding-left: 20px;">
                                                                        <li>{"Open your Tesla mobile app"}</li>
                                                                        <li>{"Scan the QR code below OR tap the button"}</li>
                                                                        <li>{"Approve the pairing request in your Tesla app"}</li>
                                                                        <li>{"Commands will work once pairing is complete"}</li>
                                                                    </ol>
                                                                </div>

                                                                {
                                                                    if let Some(qr_url) = (*vehicle_qr_code_url).as_ref() {
                                                                        html! {
                                                                            <div style="text-align: center; margin: 20px 0;">
                                                                                <img
                                                                                    src={qr_url.clone()}
                                                                                    alt="Tesla Pairing QR Code"
                                                                                    style="max-width: 300px; width: 100%; height: auto; border-radius: 8px;"
                                                                                />
                                                                                <div style="color: #999; font-size: 12px; margin-top: 10px;">
                                                                                    {"Scan this QR code with your Tesla mobile app"}
                                                                                </div>
                                                                            </div>
                                                                        }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }

                                                                {
                                                                    if let Some(link) = (*vehicle_pairing_link).as_ref() {
                                                                        html! {
                                                                            <div style="text-align: center;">
                                                                                <a
                                                                                    href={link.clone()}
                                                                                    target="_blank"
                                                                                    class="pairing-button"
                                                                                    style="
                                                                                        display: inline-block;
                                                                                        padding: 12px 24px;
                                                                                        background: linear-gradient(135deg, #1e90ff 0%, #0066cc 100%);
                                                                                        color: white;
                                                                                        text-decoration: none;
                                                                                        border-radius: 8px;
                                                                                        font-weight: 600;
                                                                                        font-size: 14px;
                                                                                        transition: all 0.3s;
                                                                                    "
                                                                                >
                                                                                    {"Open in Tesla App"}
                                                                                </a>
                                                                            </div>
                                                                        }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }

                                                                // Pairing confirmation
                                                                <div style="
                                                                    margin-top: 20px;
                                                                    padding-top: 20px;
                                                                    border-top: 1px solid rgba(126, 178, 255, 0.2);
                                                                ">
                                                                    <div style="color: #7EB2FF; font-size: 13px; font-weight: 600; margin-bottom: 10px; text-align: center;">
                                                                        {"Did you complete the pairing in the Tesla app?"}
                                                                    </div>
                                                                    <div style="display: flex; gap: 10px; justify-content: center;">
                                                                        <button
                                                                            onclick={{
                                                                                let vehicle_pairing_vin = vehicle_pairing_vin.clone();
                                                                                Callback::from(move |_| {
                                                                                    let vehicle_pairing_vin = vehicle_pairing_vin.clone();
                                                                                    spawn_local(async move {
                                                                                        // Send mark-paired request
                                                                                        match Api::post("/api/tesla/mark-paired")
                                                                                            .json(&serde_json::json!({"paired": true}))
                                                                                        {
                                                                                            Ok(req) => {
                                                                                                match req.send().await {
                                                                                                    Ok(response) => {
                                                                                                        if response.ok() {
                                                                                                            // Success - close UI and refresh
                                                                                                            vehicle_pairing_vin.set(None);
                                                                                                            if let Some(window) = web_sys::window() {
                                                                                                                let _ = window.location().reload();
                                                                                                            }
                                                                                                        }
                                                                                                    }
                                                                                                    Err(_) => {
                                                                                                        // Request failed - close UI anyway
                                                                                                        vehicle_pairing_vin.set(None);
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                            Err(_) => {
                                                                                                // Failed to create request - close UI
                                                                                                vehicle_pairing_vin.set(None);
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            style="
                                                                                padding: 8px 20px;
                                                                                background: rgba(105, 240, 174, 0.2);
                                                                                color: #69f0ae;
                                                                                border: 1px solid rgba(105, 240, 174, 0.3);
                                                                                border-radius: 6px;
                                                                                font-weight: 600;
                                                                                cursor: pointer;
                                                                            "
                                                                        >
                                                                            {"✓ Yes, Paired"}
                                                                        </button>
                                                                        <button
                                                                            onclick={{
                                                                                let vehicle_pairing_vin = vehicle_pairing_vin.clone();
                                                                                Callback::from(move |_| {
                                                                                    vehicle_pairing_vin.set(None);
                                                                                })
                                                                            }}
                                                                            style="
                                                                                padding: 8px 20px;
                                                                                background: rgba(0, 0, 0, 0.2);
                                                                                color: #999;
                                                                                border: 1px solid rgba(255, 255, 255, 0.1);
                                                                                border-radius: 6px;
                                                                                font-weight: 600;
                                                                                cursor: pointer;
                                                                            "
                                                                        >
                                                                            {"Not Yet"}
                                                                        </button>
                                                                    </div>
                                                                </div>
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }

                            // Refresh button at top
                            <button
                                onclick={handle_battery_refresh.clone()}
                                disabled={*battery_loading}
                                style="
                                    width: 100%;
                                    margin-bottom: 20px;
                                    padding: 12px 20px;
                                    background: rgba(30, 144, 255, 0.15);
                                    color: #7EB2FF;
                                    border: 1px solid rgba(30, 144, 255, 0.3);
                                    border-radius: 8px;
                                    font-size: 15px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    transition: all 0.2s;
                                    opacity: {if *battery_loading { \"0.6\" } else { \"1\" }};
                                "
                            >
                                {if *battery_loading { "🔄 Refreshing..." } else { "🔄 Refresh Status" }}
                            </button>

                            <h5 style="color: #7EB2FF; font-size: 14px; font-weight: 500; margin: 15px 0 10px 0;">{"Battery Status"}</h5>
                            {
                                if battery_level.is_some() {
                                    html! {
                                        <>
                                            <div style="display: flex; align-items: center; gap: 15px; flex-wrap: wrap;">
                                                // Battery icon (dynamic based on level)
                                                {{
                                                    let level = (*battery_level).unwrap_or(0);
                                                    let icon_class = if level <= 10 {
                                                        "fa-solid fa-battery-empty"
                                                    } else if level <= 35 {
                                                        "fa-solid fa-battery-quarter"
                                                    } else if level <= 60 {
                                                        "fa-solid fa-battery-half"
                                                    } else if level <= 90 {
                                                        "fa-solid fa-battery-three-quarters"
                                                    } else {
                                                        "fa-solid fa-battery-full"
                                                    };
                                                    html! {
                                                        <i class={icon_class} style="font-size: 32px; color: #7EB2FF;"></i>
                                                    }
                                                }}
                                                <div style="flex: 1;">
                                                    <div style="color: #fff; font-size: 18px; font-weight: 600;">
                                                        {format!("{}%", (*battery_level).unwrap_or(0))}
                                                    </div>
                                                    {
                                                        if let Some(range) = *battery_range {
                                                            html! {
                                                                <div style="color: #999; font-size: 14px;">
                                                                    {format!("{:.0} mi range", range)}
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        if let Some(state) = (*charging_state).as_ref() {
                                                            html! {
                                                                <div style="color: #69f0ae; font-size: 13px; margin-top: 4px;">
                                                                    {state}
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        if let Some(temp) = *inside_temp {
                                                            html! {
                                                                <div style="color: #999; font-size: 13px; margin-top: 4px;">
                                                                    {format!("Inside: {:.1}°C", temp)}
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        if let Some(temp) = *outside_temp {
                                                            html! {
                                                                <div style="color: #999; font-size: 13px;">
                                                                    {format!("Outside: {:.1}°C", temp)}
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        if let Some(climate) = *is_climate_on {
                                                            if climate {
                                                                html! {
                                                                    <div style="color: #69f0ae; font-size: 13px; margin-top: 4px;">
                                                                        {"🌡️ Climate On"}
                                                                    </div>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        if let Some(front_defrost) = *is_front_defroster_on {
                                                            if front_defrost {
                                                                html! {
                                                                    <div style="color: #69f0ae; font-size: 13px; margin-top: 4px;">
                                                                        {"❄️ Front Defrost On"}
                                                                    </div>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        if let Some(rear_defrost) = *is_rear_defroster_on {
                                                            if rear_defrost {
                                                                html! {
                                                                    <div style="color: #69f0ae; font-size: 13px; margin-top: 4px;">
                                                                        {"❄️ Rear Defrost On"}
                                                                    </div>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>
                                            </div>
                                        </>
                                    }
                                } else {
                                    html! {
                                        <div style="color: #999; font-size: 14px; text-align: center; padding: 20px 0;">
                                            {"Click Refresh to load battery status"}
                                        </div>
                                    }
                                }
                            }

                            <h5 style="color: #7EB2FF; font-size: 14px; font-weight: 500; margin: 20px 0 10px 0;">{"Quick Controls"}</h5>
                            <div style="display: flex; gap: 12px; margin-bottom: 15px; flex-wrap: wrap;">
                                <button
                                    onclick={handle_lock.clone()}
                                    disabled={*lock_loading}
                                    class="tesla-control-button"
                                    style="
                                        flex: 1;
                                        min-width: 120px;
                                        padding: 14px 20px;
                                        background: rgba(30, 144, 255, 0.1);
                                        color: #7EB2FF;
                                        border: 1px solid rgba(30, 144, 255, 0.2);
                                        border-radius: 8px;
                                        font-size: 15px;
                                        cursor: pointer;
                                        transition: all 0.2s;
                                        opacity: {if *lock_loading { \"0.6\" } else { \"1\" }};
                                    "
                                >
                                    {
                                        if *lock_loading {
                                            html! { <><i class="fas fa-spinner fa-spin"></i>{" Loading..."}</> }
                                        } else if let Some(locked) = *is_locked {
                                            if locked {
                                                html! { <><i class="fas fa-lock"></i>{" Locked"}</> }
                                            } else {
                                                html! { <><i class="fas fa-unlock"></i>{" Unlocked"}</> }
                                            }
                                        } else {
                                            html! { <><i class="fas fa-question"></i>{" Lock"}</> }
                                        }
                                    }
                                </button>

                                <button
                                    onclick={handle_climate.clone()}
                                    disabled={*climate_loading}
                                    class="tesla-control-button"
                                    style="
                                        flex: 1;
                                        min-width: 120px;
                                        padding: 14px 20px;
                                        background: rgba(30, 144, 255, 0.1);
                                        color: #7EB2FF;
                                        border: 1px solid rgba(30, 144, 255, 0.2);
                                        border-radius: 8px;
                                        font-size: 15px;
                                        cursor: pointer;
                                        transition: all 0.2s;
                                        opacity: {if *climate_loading { \"0.6\" } else { \"1\" }};
                                    "
                                >
                                    {
                                        if *climate_loading {
                                            html! { <><i class="fas fa-spinner fa-spin"></i>{" Loading..."}</> }
                                        } else if let Some(climate_on) = *is_climate_on {
                                            if climate_on {
                                                html! { <><i class="fas fa-fan"></i>{" Climate On"}</> }
                                            } else {
                                                html! { <><i class="fas fa-fan"></i>{" Climate Off"}</> }
                                            }
                                        } else {
                                            html! { <><i class="fas fa-question"></i>{" Climate"}</> }
                                        }
                                    }
                                </button>

                                <button
                                    onclick={handle_defrost.clone()}
                                    disabled={*defrost_loading}
                                    class="tesla-control-button"
                                    style="
                                        flex: 1;
                                        min-width: 120px;
                                        padding: 14px 20px;
                                        background: rgba(30, 144, 255, 0.1);
                                        color: #7EB2FF;
                                        border: 1px solid rgba(30, 144, 255, 0.2);
                                        border-radius: 8px;
                                        font-size: 15px;
                                        cursor: pointer;
                                        transition: all 0.2s;
                                        opacity: {if *defrost_loading { \"0.6\" } else { \"1\" }};
                                    "
                                >
                                    {
                                        if *defrost_loading {
                                            html! { <><i class="fas fa-spinner fa-spin"></i>{" Loading..."}</> }
                                        } else {
                                            // Show defrost status if we have data
                                            let front_on = is_front_defroster_on.unwrap_or(false);
                                            let rear_on = is_rear_defroster_on.unwrap_or(false);
                                            let any_on = front_on || rear_on;

                                            if is_front_defroster_on.is_none() && is_rear_defroster_on.is_none() {
                                                html! { <><i class="fas fa-question"></i>{" Defrost"}</> }
                                            } else if any_on {
                                                html! { <><i class="fas fa-snowflake"></i>{" Defrost On"}</> }
                                            } else {
                                                html! { <><i class="fas fa-snowflake"></i>{" Defrost Off"}</> }
                                            }
                                        }
                                    }
                                </button>
                            </div>

                            // Command result feedback
                            if let Some(result) = (*command_result).as_ref() {
                                <div style="
                                    padding: 10px;
                                    background: rgba(105, 240, 174, 0.1);
                                    color: #69f0ae;
                                    border-radius: 8px;
                                    font-size: 14px;
                                    border: 1px solid rgba(105, 240, 174, 0.2);
                                ">
                                    {result}
                                </div>
                            }
                        </div>

                        <button
                            class="disconnect-button"
                            onclick={onclick_disconnect}
                        >
                            {"Disconnect"}
                        </button>

                        // Disconnect confirmation modal
                        if *show_disconnect_modal {
                            <div class="modal-overlay">
                                <div class="modal-content">
                                    <h3>{"Confirm Disconnection"}</h3>
                                    <p>{"Are you sure you want to disconnect Tesla? This will:"}</p>
                                    <ul>
                                        <li>{"Remove your Tesla OAuth tokens from our servers"}</li>
                                        <li>{"Delete your selected vehicle and pairing status"}</li>
                                        <li>{"Stop all Tesla vehicle control features"}</li>
                                        <li>{"Require reconnection to use Tesla features again"}</li>
                                    </ul>
                                    <p style="margin-top: 15px; color: #7EB2FF; font-size: 13px;">
                                        {"Note: To fully revoke access, you may also want to visit "}
                                        <a
                                            href="https://auth.tesla.com/user/revoke/consent"
                                            target="_blank"
                                            style="color: #69f0ae; text-decoration: underline;"
                                        >
                                            {"Tesla's consent management page"}
                                        </a>
                                        {" after disconnecting."}
                                    </p>
                                    if *is_disconnecting {
                                        <p class="disconnecting-message">{"Disconnecting Tesla... Please wait."}</p>
                                    }
                                    <div class="modal-buttons">
                                        <button onclick={
                                            let show_disconnect_modal = show_disconnect_modal.clone();
                                            Callback::from(move |_| show_disconnect_modal.set(false))
                                        } class="cancel-button" disabled={*is_disconnecting}>
                                            {"Cancel"}
                                        </button>
                                        <button onclick={handle_confirmed_disconnect.clone()}
                                            class="confirm-disconnect-button" disabled={*is_disconnecting}>
                                            if *is_disconnecting {
                                                <span class="button-spinner"></span> {"Disconnecting..."}
                                            } else {
                                                {"Yes, Disconnect"}
                                            }
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }
                    </div>
                }
            } else {
                <div class="subscription-notice">
                    <p>{"Tesla integration requires a paid subscription."}</p>
                    <a href="/profile" class="upgrade-link">{"Upgrade Now"}</a>
                </div>
            }

            <style>
                {r#"
                    .modal-overlay {
                        position: fixed;
                        top: 0;
                        left: 0;
                        right: 0;
                        bottom: 0;
                        background: rgba(0, 0, 0, 0.85);
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        z-index: 1000;
                    }
                    .modal-content {
                        background: #1a1a1a;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 12px;
                        padding: 2rem;
                        max-width: 500px;
                        width: 90%;
                        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
                    }
                    .modal-content h3 {
                        color: #FF6347;
                        margin-bottom: 1rem;
                    }
                    .modal-content p {
                        color: #CCC;
                        margin-bottom: 1rem;
                    }
                    .modal-content ul {
                        margin-bottom: 2rem;
                        padding-left: 1.5rem;
                    }
                    .modal-content li {
                        color: #999;
                        margin-bottom: 0.5rem;
                    }
                    .modal-buttons {
                        display: flex;
                        gap: 1rem;
                        justify-content: flex-end;
                    }
                    .cancel-button {
                        background: transparent;
                        border: 1px solid rgba(204, 204, 204, 0.3);
                        color: #CCC;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                    }
                    .cancel-button:hover {
                        background: rgba(204, 204, 204, 0.1);
                        transform: translateY(-2px);
                    }
                    .cancel-button:disabled {
                        opacity: 0.5;
                        cursor: not-allowed;
                    }
                    .confirm-disconnect-button {
                        background: linear-gradient(45deg, #FF6347, #FF4500);
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                    .confirm-disconnect-button:hover:not(:disabled) {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(255, 99, 71, 0.3);
                    }
                    .confirm-disconnect-button:disabled {
                        opacity: 0.6;
                        cursor: not-allowed;
                    }
                    .button-spinner {
                        display: inline-block;
                        width: 14px;
                        height: 14px;
                        border: 2px solid rgba(255, 255, 255, 0.3);
                        border-radius: 50%;
                        border-top-color: white;
                        animation: spin 1s ease-in-out infinite;
                    }
                    .disconnecting-message {
                        color: #7EB2FF;
                        font-style: italic;
                        text-align: center;
                        margin: 1rem 0;
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                "#}
            </style>
        </div>
    }
}
