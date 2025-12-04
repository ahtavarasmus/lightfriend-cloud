use yew::prelude::*;
use web_sys::HtmlInputElement;
use crate::utils::api::Api;
use wasm_bindgen_futures::spawn_local;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug)]
pub enum TwoFactorState {
    Loading,
    Disabled,
    Enabled { remaining_backup_codes: i64 },
    Setting { qr_code_url: String, secret: String },
    ShowingBackupCodes { codes: Vec<String> },
    Error(String),
}

#[derive(Deserialize, Debug, Clone)]
pub struct TotpStatusResponse {
    pub enabled: bool,
    pub remaining_backup_codes: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TotpSetupResponse {
    pub qr_code_data_url: String,
    pub secret: String,
}

#[derive(Serialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TotpVerifyResponse {
    pub success: bool,
    pub backup_codes: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RegenerateBackupCodesResponse {
    pub backup_codes: Vec<String>,
}

#[function_component]
pub fn SecuritySettings() -> Html {
    let state = use_state(|| TwoFactorState::Loading);
    let verification_code = use_state(String::new);
    let disable_code = use_state(String::new);
    let regenerate_code = use_state(String::new);
    let show_secret = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let is_saving = use_state(|| false);
    let show_disable_modal = use_state(|| false);
    let show_regenerate_modal = use_state(|| false);
    let codes_copied = use_state(|| false);

    // Load TOTP status on mount
    {
        let state = state.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                match Api::get("/api/totp/status").send().await {
                    Ok(resp) if resp.ok() => {
                        if let Ok(status) = resp.json::<TotpStatusResponse>().await {
                            if status.enabled {
                                state.set(TwoFactorState::Enabled {
                                    remaining_backup_codes: status.remaining_backup_codes,
                                });
                            } else {
                                state.set(TwoFactorState::Disabled);
                            }
                        }
                    }
                    _ => {
                        state.set(TwoFactorState::Error("Failed to load 2FA status".to_string()));
                    }
                }
            });
            || ()
        }, ());
    }

    // Start 2FA setup
    let on_enable_click = {
        let state = state.clone();
        let error_message = error_message.clone();
        Callback::from(move |_: MouseEvent| {
            let state = state.clone();
            let error_message = error_message.clone();
            spawn_local(async move {
                match Api::post("/api/totp/setup/start")
                    .header("Content-Type", "application/json")
                    .body("{}")
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        if let Ok(setup) = resp.json::<TotpSetupResponse>().await {
                            state.set(TwoFactorState::Setting {
                                qr_code_url: setup.qr_code_data_url,
                                secret: setup.secret,
                            });
                        }
                    }
                    Ok(resp) => {
                        error_message.set(Some(format!("Failed to start 2FA setup: {}", resp.status())));
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {:?}", e)));
                    }
                }
            });
        })
    };

    // Verify setup code
    let on_verify_setup = {
        let state = state.clone();
        let verification_code = verification_code.clone();
        let error_message = error_message.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |_: MouseEvent| {
            let code = (*verification_code).clone();
            if code.len() != 6 {
                error_message.set(Some("Please enter a 6-digit code".to_string()));
                return;
            }
            let state = state.clone();
            let error_message = error_message.clone();
            let is_saving = is_saving.clone();
            let verification_code = verification_code.clone();
            is_saving.set(true);
            spawn_local(async move {
                let request = TotpVerifyRequest { code };
                match Api::post("/api/totp/setup/verify")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        if let Ok(result) = resp.json::<TotpVerifyResponse>().await {
                            verification_code.set(String::new());
                            state.set(TwoFactorState::ShowingBackupCodes {
                                codes: result.backup_codes,
                            });
                        }
                    }
                    Ok(_) => {
                        error_message.set(Some("Invalid verification code".to_string()));
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                is_saving.set(false);
            });
        })
    };

    // Finish viewing backup codes
    let on_backup_codes_done = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            state.set(TwoFactorState::Enabled {
                remaining_backup_codes: 10,
            });
        })
    };

    // Cancel setup
    let on_cancel_setup = {
        let state = state.clone();
        let verification_code = verification_code.clone();
        let show_secret = show_secret.clone();
        Callback::from(move |_: MouseEvent| {
            verification_code.set(String::new());
            show_secret.set(false);
            state.set(TwoFactorState::Disabled);
        })
    };

    // Show disable modal
    let on_disable_click = {
        let show_disable_modal = show_disable_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_disable_modal.set(true);
        })
    };

    // Cancel disable
    let on_cancel_disable = {
        let show_disable_modal = show_disable_modal.clone();
        let disable_code = disable_code.clone();
        Callback::from(move |_: MouseEvent| {
            disable_code.set(String::new());
            show_disable_modal.set(false);
        })
    };

    // Confirm disable
    let on_confirm_disable = {
        let state = state.clone();
        let disable_code = disable_code.clone();
        let show_disable_modal = show_disable_modal.clone();
        let error_message = error_message.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |_: MouseEvent| {
            let code = (*disable_code).clone();
            if code.is_empty() {
                error_message.set(Some("Please enter your 2FA code".to_string()));
                return;
            }
            let state = state.clone();
            let disable_code = disable_code.clone();
            let show_disable_modal = show_disable_modal.clone();
            let error_message = error_message.clone();
            let is_saving = is_saving.clone();
            is_saving.set(true);
            spawn_local(async move {
                let request = TotpVerifyRequest { code };
                match Api::post("/api/totp/disable")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        disable_code.set(String::new());
                        show_disable_modal.set(false);
                        state.set(TwoFactorState::Disabled);
                    }
                    Ok(_) => {
                        error_message.set(Some("Invalid code".to_string()));
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                is_saving.set(false);
            });
        })
    };

    // Show regenerate modal
    let on_regenerate_click = {
        let show_regenerate_modal = show_regenerate_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_regenerate_modal.set(true);
        })
    };

    // Cancel regenerate
    let on_cancel_regenerate = {
        let show_regenerate_modal = show_regenerate_modal.clone();
        let regenerate_code = regenerate_code.clone();
        Callback::from(move |_: MouseEvent| {
            regenerate_code.set(String::new());
            show_regenerate_modal.set(false);
        })
    };

    // Confirm regenerate
    let on_confirm_regenerate = {
        let state = state.clone();
        let regenerate_code = regenerate_code.clone();
        let show_regenerate_modal = show_regenerate_modal.clone();
        let error_message = error_message.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |_: MouseEvent| {
            let code = (*regenerate_code).clone();
            if code.is_empty() {
                error_message.set(Some("Please enter your 2FA code".to_string()));
                return;
            }
            let state = state.clone();
            let regenerate_code = regenerate_code.clone();
            let show_regenerate_modal = show_regenerate_modal.clone();
            let error_message = error_message.clone();
            let is_saving = is_saving.clone();
            is_saving.set(true);
            spawn_local(async move {
                let request = TotpVerifyRequest { code };
                match Api::post("/api/totp/backup-codes/regenerate")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        if let Ok(result) = resp.json::<RegenerateBackupCodesResponse>().await {
                            regenerate_code.set(String::new());
                            show_regenerate_modal.set(false);
                            state.set(TwoFactorState::ShowingBackupCodes {
                                codes: result.backup_codes,
                            });
                        }
                    }
                    Ok(_) => {
                        error_message.set(Some("Invalid code".to_string()));
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                is_saving.set(false);
            });
        })
    };

    // Copy backup codes to clipboard
    let on_copy_codes = {
        let codes_copied = codes_copied.clone();
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            if let TwoFactorState::ShowingBackupCodes { codes } = &*state {
                let codes_text = codes.join("\n");
                if let Some(window) = web_sys::window() {
                    let navigator = window.navigator();
                    let clipboard = navigator.clipboard();
                    let codes_copied = codes_copied.clone();
                    spawn_local(async move {
                        let _ = wasm_bindgen_futures::JsFuture::from(
                            clipboard.write_text(&codes_text)
                        ).await;
                        codes_copied.set(true);
                    });
                }
            }
        })
    };

    // Toggle show secret
    let on_toggle_secret = {
        let show_secret = show_secret.clone();
        Callback::from(move |_: MouseEvent| {
            show_secret.set(!*show_secret);
        })
    };

    // Clear error
    let on_clear_error = {
        let error_message = error_message.clone();
        Callback::from(move |_: MouseEvent| {
            error_message.set(None);
        })
    };

    html! {
        <div class="security-settings">
            <h3 class="security-title">{"Two-Factor Authentication"}</h3>

            // Error message
            {
                if let Some(error) = &*error_message {
                    html! {
                        <div class="security-error">
                            <span>{error}</span>
                            <button onclick={on_clear_error}>{"×"}</button>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Main content based on state
            {
                match &*state {
                    TwoFactorState::Loading => html! {
                        <div class="security-loading">{"Loading..."}</div>
                    },
                    TwoFactorState::Disabled => html! {
                        <div class="security-disabled">
                            <p class="security-description">
                                {"Protect your account with two-factor authentication. "}
                                {"You'll need an authenticator app like Google Authenticator or Authy."}
                            </p>
                            <button class="security-btn primary" onclick={on_enable_click}>
                                {"Enable 2FA"}
                            </button>
                        </div>
                    },
                    TwoFactorState::Enabled { remaining_backup_codes } => html! {
                        <div class="security-enabled">
                            <div class="security-status">
                                <span class="status-badge enabled">{"Enabled"}</span>
                                <span class="backup-count">
                                    {format!("{} backup codes remaining", remaining_backup_codes)}
                                </span>
                            </div>
                            <div class="security-actions">
                                <button class="security-btn secondary" onclick={on_regenerate_click}>
                                    {"Regenerate Backup Codes"}
                                </button>
                                <button class="security-btn danger" onclick={on_disable_click}>
                                    {"Disable 2FA"}
                                </button>
                            </div>
                        </div>
                    },
                    TwoFactorState::Setting { qr_code_url, secret } => html! {
                        <div class="security-setup">
                            <p class="setup-instruction">
                                {"Scan this QR code with your authenticator app:"}
                            </p>
                            <img class="qr-code" src={qr_code_url.clone()} alt="QR Code" />

                            <div class="secret-section">
                                <button class="show-secret-btn" onclick={on_toggle_secret.clone()}>
                                    {if *show_secret { "Hide secret key" } else { "Can't scan? Show secret key" }}
                                </button>
                                {
                                    if *show_secret {
                                        html! {
                                            <div class="secret-display">
                                                <code>{secret.clone()}</code>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>

                            <div class="verify-section">
                                <label>{"Enter the 6-digit code from your app:"}</label>
                                <input
                                    type="text"
                                    class="verify-input"
                                    maxlength="6"
                                    placeholder="000000"
                                    value={(*verification_code).clone()}
                                    oninput={
                                        let verification_code = verification_code.clone();
                                        move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            let value = input.value().chars().filter(|c| c.is_numeric()).collect::<String>();
                                            verification_code.set(value);
                                        }
                                    }
                                />
                            </div>

                            <div class="setup-buttons">
                                <button class="security-btn secondary" onclick={on_cancel_setup}>
                                    {"Cancel"}
                                </button>
                                <button
                                    class="security-btn primary"
                                    onclick={on_verify_setup}
                                    disabled={*is_saving || verification_code.len() != 6}
                                >
                                    {if *is_saving { "Verifying..." } else { "Verify & Enable" }}
                                </button>
                            </div>
                        </div>
                    },
                    TwoFactorState::ShowingBackupCodes { codes } => html! {
                        <div class="backup-codes-display">
                            <h4>{"Save Your Backup Codes"}</h4>
                            <p class="backup-warning">
                                {"These codes can be used to access your account if you lose your authenticator. "}
                                {"Each code can only be used once. Save them somewhere safe!"}
                            </p>
                            <div class="backup-codes-grid">
                                {
                                    codes.iter().map(|code| {
                                        html! { <code class="backup-code">{code}</code> }
                                    }).collect::<Html>()
                                }
                            </div>
                            <div class="backup-codes-actions">
                                <button class="security-btn secondary" onclick={on_copy_codes.clone()}>
                                    {if *codes_copied { "Copied!" } else { "Copy All" }}
                                </button>
                                <button class="security-btn primary" onclick={on_backup_codes_done}>
                                    {"I've Saved These Codes"}
                                </button>
                            </div>
                        </div>
                    },
                    TwoFactorState::Error(msg) => html! {
                        <div class="security-error-state">
                            <p>{msg}</p>
                            <button class="security-btn primary" onclick={on_enable_click}>
                                {"Retry"}
                            </button>
                        </div>
                    },
                }
            }

            // Disable modal
            {
                if *show_disable_modal {
                    html! {
                        <div class="modal-overlay">
                            <div class="modal-content">
                                <h4>{"Disable Two-Factor Authentication"}</h4>
                                <p>{"Enter your current 2FA code to disable:"}</p>
                                <input
                                    type="text"
                                    class="verify-input"
                                    maxlength="6"
                                    placeholder="000000"
                                    value={(*disable_code).clone()}
                                    oninput={
                                        let disable_code = disable_code.clone();
                                        move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            let value = input.value().chars().filter(|c| c.is_numeric()).collect::<String>();
                                            disable_code.set(value);
                                        }
                                    }
                                />
                                <div class="modal-buttons">
                                    <button class="security-btn secondary" onclick={on_cancel_disable}>
                                        {"Cancel"}
                                    </button>
                                    <button
                                        class="security-btn danger"
                                        onclick={on_confirm_disable}
                                        disabled={*is_saving}
                                    >
                                        {if *is_saving { "Disabling..." } else { "Disable 2FA" }}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Regenerate modal
            {
                if *show_regenerate_modal {
                    html! {
                        <div class="modal-overlay">
                            <div class="modal-content">
                                <h4>{"Regenerate Backup Codes"}</h4>
                                <p>{"This will invalidate your old backup codes. Enter your current 2FA code to continue:"}</p>
                                <input
                                    type="text"
                                    class="verify-input"
                                    maxlength="6"
                                    placeholder="000000"
                                    value={(*regenerate_code).clone()}
                                    oninput={
                                        let regenerate_code = regenerate_code.clone();
                                        move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            let value = input.value().chars().filter(|c| c.is_numeric()).collect::<String>();
                                            regenerate_code.set(value);
                                        }
                                    }
                                />
                                <div class="modal-buttons">
                                    <button class="security-btn secondary" onclick={on_cancel_regenerate}>
                                        {"Cancel"}
                                    </button>
                                    <button
                                        class="security-btn primary"
                                        onclick={on_confirm_regenerate}
                                        disabled={*is_saving}
                                    >
                                        {if *is_saving { "Regenerating..." } else { "Regenerate Codes" }}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            <style>
            {r#"
                .security-settings {
                    padding: 16px;
                    border: 1px solid #e0e0e0;
                    border-radius: 8px;
                    margin-top: 24px;
                    background: #fafafa;
                }
                .security-title {
                    margin: 0 0 16px 0;
                    font-size: 18px;
                    color: #333;
                }
                .security-description {
                    color: #666;
                    margin-bottom: 16px;
                    line-height: 1.5;
                }
                .security-error {
                    background: #fee2e2;
                    border: 1px solid #ef4444;
                    color: #dc2626;
                    padding: 12px;
                    border-radius: 6px;
                    margin-bottom: 16px;
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                }
                .security-error button {
                    background: none;
                    border: none;
                    color: #dc2626;
                    cursor: pointer;
                    font-size: 18px;
                }
                .security-loading {
                    color: #666;
                    padding: 20px;
                    text-align: center;
                }
                .security-btn {
                    padding: 10px 20px;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 14px;
                    font-weight: 500;
                    transition: background-color 0.2s;
                }
                .security-btn:disabled {
                    opacity: 0.6;
                    cursor: not-allowed;
                }
                .security-btn.primary {
                    background: #1E90FF;
                    color: white;
                }
                .security-btn.primary:hover:not(:disabled) {
                    background: #1a7ae0;
                }
                .security-btn.secondary {
                    background: #e0e0e0;
                    color: #333;
                }
                .security-btn.secondary:hover:not(:disabled) {
                    background: #d0d0d0;
                }
                .security-btn.danger {
                    background: #ef4444;
                    color: white;
                }
                .security-btn.danger:hover:not(:disabled) {
                    background: #dc2626;
                }
                .security-status {
                    display: flex;
                    align-items: center;
                    gap: 12px;
                    margin-bottom: 16px;
                }
                .status-badge {
                    padding: 4px 12px;
                    border-radius: 20px;
                    font-size: 14px;
                    font-weight: 500;
                }
                .status-badge.enabled {
                    background: #dcfce7;
                    color: #166534;
                }
                .backup-count {
                    color: #666;
                    font-size: 14px;
                }
                .security-actions {
                    display: flex;
                    gap: 12px;
                }
                .qr-code {
                    width: 200px;
                    height: 200px;
                    display: block;
                    margin: 16px auto;
                    border: 1px solid #e0e0e0;
                    border-radius: 8px;
                }
                .secret-section {
                    text-align: center;
                    margin: 16px 0;
                }
                .show-secret-btn {
                    background: none;
                    border: none;
                    color: #1E90FF;
                    cursor: pointer;
                    font-size: 14px;
                    text-decoration: underline;
                }
                .secret-display {
                    margin-top: 12px;
                    padding: 12px;
                    background: #f5f5f5;
                    border-radius: 6px;
                }
                .secret-display code {
                    font-family: monospace;
                    font-size: 14px;
                    word-break: break-all;
                }
                .verify-section {
                    margin: 20px 0;
                }
                .verify-section label {
                    display: block;
                    margin-bottom: 8px;
                    color: #333;
                }
                .verify-input {
                    width: 100%;
                    max-width: 200px;
                    padding: 12px;
                    font-size: 24px;
                    text-align: center;
                    letter-spacing: 8px;
                    border: 2px solid #e0e0e0;
                    border-radius: 8px;
                    font-family: monospace;
                }
                .verify-input:focus {
                    outline: none;
                    border-color: #1E90FF;
                }
                .setup-buttons {
                    display: flex;
                    gap: 12px;
                    justify-content: center;
                    margin-top: 20px;
                }
                .backup-codes-display h4 {
                    margin: 0 0 12px 0;
                }
                .backup-warning {
                    background: #fef3c7;
                    border: 1px solid #f59e0b;
                    color: #92400e;
                    padding: 12px;
                    border-radius: 6px;
                    margin-bottom: 16px;
                    line-height: 1.5;
                }
                .backup-codes-grid {
                    display: grid;
                    grid-template-columns: repeat(2, 1fr);
                    gap: 8px;
                    margin-bottom: 16px;
                }
                .backup-code {
                    background: #f5f5f5;
                    padding: 8px 12px;
                    border-radius: 4px;
                    font-family: monospace;
                    font-size: 14px;
                    text-align: center;
                }
                .backup-codes-actions {
                    display: flex;
                    gap: 12px;
                    justify-content: center;
                }
                .modal-overlay {
                    position: fixed;
                    top: 0;
                    left: 0;
                    right: 0;
                    bottom: 0;
                    background: rgba(0, 0, 0, 0.5);
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    z-index: 1000;
                }
                .modal-content {
                    background: white;
                    padding: 24px;
                    border-radius: 12px;
                    max-width: 400px;
                    width: 90%;
                }
                .modal-content h4 {
                    margin: 0 0 12px 0;
                }
                .modal-content p {
                    color: #666;
                    margin-bottom: 16px;
                }
                .modal-buttons {
                    display: flex;
                    gap: 12px;
                    justify-content: flex-end;
                    margin-top: 16px;
                }
            "#}
            </style>
        </div>
    }
}
