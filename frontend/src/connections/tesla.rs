use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::functional::UseStateHandle;
use crate::config;

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

    // Check Tesla connection status on mount
    {
        let tesla_connected = tesla_connected.clone();
        let error = error.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    let token = if let Some(window) = window() {
                        if let Ok(Some(storage)) = window.local_storage() {
                            storage.get_item("token").ok().flatten()
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(token) = token {
                        match Request::get(&format!("{}/api/auth/tesla/status", config::get_backend_url()))
                            .header("Authorization", &format!("Bearer {}", token))
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
                        let token = if let Some(window) = window() {
                            if let Ok(Some(storage)) = window.local_storage() {
                                storage.get_item("token").ok().flatten()
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if let Some(token) = token {
                            match Request::get(&format!("{}/api/auth/tesla/virtual-key", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
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

                                            // Check if user has dismissed pairing notice before
                                            if let Some(window) = window() {
                                                if let Ok(Some(storage)) = window.local_storage() {
                                                    let dismissed = storage.get_item("tesla_pairing_dismissed")
                                                        .ok()
                                                        .flatten()
                                                        .unwrap_or_else(|| "false".to_string());
                                                    show_pairing.set(dismissed != "true");
                                                } else {
                                                    show_pairing.set(true);
                                                }
                                            } else {
                                                show_pairing.set(true);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to fetch pairing info: {}", e)));
                                }
                            }
                        }
                    });
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
                let token = if let Some(window) = window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        storage.get_item("token").ok().flatten()
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(token) = token {
                    match Request::get(&format!("{}/api/auth/tesla/login", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
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
                } else {
                    error.set(Some("No auth token found".to_string()));
                }
                connecting.set(false);
            });
        })
    };

    // Handle disconnect button click
    let onclick_disconnect = {
        let tesla_connected = tesla_connected.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let tesla_connected = tesla_connected.clone();
            let error = error.clone();

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::delete(&format!("{}/api/auth/tesla/connection", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;
                            match request {
                                Ok(response) => {
                                    if response.ok() {
                                        tesla_connected.set(false);
                                    } else {
                                        if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                            if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                error.set(Some(error_msg.to_string()));
                                            } else {
                                                error.set(Some(format!("Failed to delete connection: {}", response.status())));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                        });
                    }
                }
            }
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

    // Handle show pairing again button click
    let onclick_show_pairing = {
        let show_pairing = show_pairing.clone();
        Callback::from(move |_: MouseEvent| {
            show_pairing.set(true);
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("tesla_pairing_dismissed", "false");
                }
            }
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
                        <p class="success-message">{"✓ Tesla connected successfully"}</p>

                        // Show virtual key pairing section if needed
                        if *show_pairing && pairing_link.is_some() {
                            <div class="pairing-section" style="
                                background: linear-gradient(135deg, #fff3cd 0%, #fff8dc 100%);
                                border: 2px solid #ffc107;
                                border-radius: 8px;
                                padding: 20px;
                                margin: 15px 0;
                                box-shadow: 0 2px 8px rgba(255, 193, 7, 0.2);
                            ">
                                <h4 style="margin-top: 0; color: #856404; display: flex; align-items: center; gap: 8px;">
                                    <span style="font-size: 24px;">{"⚠️"}</span>
                                    {"One More Step Required"}
                                </h4>
                                <p style="color: #856404; margin-bottom: 15px;">
                                    {"To control your Tesla remotely, you must authorize this app in your Tesla mobile app."}
                                </p>

                                <div style="background: white; padding: 15px; border-radius: 6px; margin-bottom: 15px;">
                                    <h5 style="margin-top: 0; color: #333;">{"Setup Instructions:"}</h5>
                                    <ol style="color: #333; margin: 10px 0; padding-left: 20px;">
                                        <li>{"Open the Tesla mobile app on your phone"}</li>
                                        <li>{"Scan the QR code below OR tap the button"}</li>
                                        <li>{"Approve the pairing request in your Tesla app"}</li>
                                        <li>{"Select which vehicle(s) to grant access to"}</li>
                                    </ol>
                                </div>

                                if let Some(qr_url) = (*qr_code_url).as_ref() {
                                    <div style="text-align: center; margin: 20px 0;">
                                        <img
                                            src={qr_url.clone()}
                                            alt="Tesla Pairing QR Code"
                                            style="width: 250px; height: 250px; border: 4px solid #ffc107; border-radius: 8px; background: white; padding: 10px;"
                                        />
                                        <p style="color: #856404; margin-top: 10px; font-size: 14px;">
                                            {"Scan this QR code with your Tesla mobile app"}
                                        </p>
                                    </div>
                                }

                                if let Some(link) = (*pairing_link).as_ref() {
                                    <div style="text-align: center; margin: 15px 0;">
                                        <a
                                            href={link.clone()}
                                            target="_blank"
                                            class="pairing-button"
                                            style="
                                                display: inline-block;
                                                background: #28a745;
                                                color: white;
                                                padding: 12px 24px;
                                                border-radius: 6px;
                                                text-decoration: none;
                                                font-weight: bold;
                                                font-size: 16px;
                                                box-shadow: 0 2px 4px rgba(0,0,0,0.2);
                                            "
                                        >
                                            {"📱 Open Tesla App to Pair"}
                                        </a>
                                        <p style="color: #856404; margin-top: 8px; font-size: 13px;">
                                            {"(Best on mobile device)"}
                                        </p>
                                    </div>
                                }

                                <div style="margin-top: 20px; padding-top: 15px; border-top: 1px solid #ffc107; display: flex; gap: 10px; justify-content: center;">
                                    <button
                                        onclick={onclick_dismiss_pairing.clone()}
                                        style="
                                            background: #28a745;
                                            color: white;
                                            border: none;
                                            padding: 10px 20px;
                                            border-radius: 5px;
                                            cursor: pointer;
                                            font-size: 14px;
                                        "
                                    >
                                        {"✓ I've Completed Pairing"}
                                    </button>
                                    <button
                                        onclick={onclick_dismiss_pairing.clone()}
                                        style="
                                            background: transparent;
                                            color: #856404;
                                            border: 1px solid #856404;
                                            padding: 10px 20px;
                                            border-radius: 5px;
                                            cursor: pointer;
                                            font-size: 14px;
                                        "
                                    >
                                        {"Remind Me Later"}
                                    </button>
                                </div>
                            </div>
                        } else if !*show_pairing && pairing_link.is_some() {
                            <div style="margin: 10px 0;">
                                <button
                                    onclick={onclick_show_pairing}
                                    style="
                                        background: transparent;
                                        color: #ffc107;
                                        border: 1px solid #ffc107;
                                        padding: 8px 16px;
                                        border-radius: 5px;
                                        cursor: pointer;
                                        font-size: 13px;
                                    "
                                >
                                    {"🔑 Show Virtual Key Pairing Instructions"}
                                </button>
                            </div>
                        }

                        <button
                            class="disconnect-button"
                            onclick={onclick_disconnect}
                        >
                            {"Disconnect"}
                        </button>
                    </div>
                }
            } else {
                <div class="subscription-notice">
                    <p>{"Tesla integration requires a paid subscription."}</p>
                    <a href="/profile" class="upgrade-link">{"Upgrade Now"}</a>
                </div>
            }

            // Admin test controls for user_id 1
            if props.user_id == 1 && *tesla_connected {
                <div class="admin-test-section" style="border-top: 1px solid #ddd; margin-top: 20px; padding-top: 20px;">
                    <h4>{"Admin Test Controls"}</h4>
                    <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; margin-top: 10px;">
                        <button
                            class="admin-test-button"
                            style="padding: 10px; background-color: #4CAF50; color: white; border: none; border-radius: 5px; cursor: pointer;"
                            onclick={create_command_handler("lock", error.clone())}
                        >
                            {"🔒 Lock"}
                        </button>
                        <button
                            class="admin-test-button"
                            style="padding: 10px; background-color: #2196F3; color: white; border: none; border-radius: 5px; cursor: pointer;"
                            onclick={create_command_handler("unlock", error.clone())}
                        >
                            {"🔓 Unlock"}
                        </button>
                        <button
                            class="admin-test-button"
                            style="padding: 10px; background-color: #FF9800; color: white; border: none; border-radius: 5px; cursor: pointer;"
                            onclick={create_command_handler("climate_on", error.clone())}
                        >
                            {"❄️ Start Climate"}
                        </button>
                        <button
                            class="admin-test-button"
                            style="padding: 10px; background-color: #f44336; color: white; border: none; border-radius: 5px; cursor: pointer;"
                            onclick={create_command_handler("climate_off", error.clone())}
                        >
                            {"🔥 Stop Climate"}
                        </button>
                        <button
                            class="admin-test-button"
                            style="padding: 10px; background-color: #9C27B0; color: white; border: none; border-radius: 5px; cursor: pointer;"
                            onclick={create_command_handler("remote_start", error.clone())}
                        >
                            {"🚗 Remote Start"}
                        </button>
                        <button
                            class="admin-test-button"
                            style="padding: 10px; background-color: #607D8B; color: white; border: none; border-radius: 5px; cursor: pointer;"
                            onclick={create_command_handler("charge_status", error.clone())}
                        >
                            {"🔋 Charge Status"}
                        </button>
                    </div>
                    <div id="command-result" style="margin-top: 10px; padding: 10px; background-color: #f0f0f0; border-radius: 5px; display: none;">
                    </div>
                </div>
            }
        </div>
    }
}

// Helper function to create command handlers
fn create_command_handler(command: &str, error: UseStateHandle<Option<String>>) -> Callback<MouseEvent> {
    let command = command.to_string();
    Callback::from(move |_: MouseEvent| {
        let command = command.clone();
        let error = error.clone();

        spawn_local(async move {
            // Show loading state
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    if let Some(result_div) = document.get_element_by_id("command-result") {
                        let _ = result_div.set_inner_html("⏳ Sending command...");
                        let _ = result_div.set_attribute("style", "margin-top: 10px; padding: 10px; background-color: #fff3cd; border-radius: 5px; display: block;");
                    }
                }
            }

            let token = if let Some(window) = window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    storage.get_item("token").ok().flatten()
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(token) = token {
                let request_body = serde_json::json!({
                    "command": command,
                    "vehicle_id": null
                });

                match Request::post(&format!("{}/api/tesla/command", config::get_backend_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request_body)
                    .expect("Failed to set JSON body")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                if let Some(result_div) = document.get_element_by_id("command-result") {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            let success = data["success"].as_bool().unwrap_or(false);
                                            let message = data["message"].as_str().unwrap_or("Command sent");

                                            let bg_color = if success { "#d4edda" } else { "#f8d7da" };
                                            let text_color = if success { "#155724" } else { "#721c24" };
                                            let icon = if success { "✅" } else { "❌" };

                                            let _ = result_div.set_inner_html(&format!("{} {}", icon, message));
                                            let _ = result_div.set_attribute("style", &format!("margin-top: 10px; padding: 10px; background-color: {}; color: {}; border-radius: 5px; display: block;", bg_color, text_color));
                                        }
                                    } else {
                                        if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                            let error_msg = error_data["error"].as_str().unwrap_or("Command failed");
                                            let _ = result_div.set_inner_html(&format!("❌ {}", error_msg));
                                            let _ = result_div.set_attribute("style", "margin-top: 10px; padding: 10px; background-color: #f8d7da; color: #721c24; border-radius: 5px; display: block;");
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                if let Some(result_div) = document.get_element_by_id("command-result") {
                                    let _ = result_div.set_inner_html(&format!("❌ Network error: {}", e));
                                    let _ = result_div.set_attribute("style", "margin-top: 10px; padding: 10px; background-color: #f8d7da; color: #721c24; border-radius: 5px; display: block;");
                                }
                            }
                        }
                    }
                }
            } else {
                error.set(Some("No auth token found".to_string()));
            }
        });
    })
}
