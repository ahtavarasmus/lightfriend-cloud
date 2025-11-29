use yew::prelude::*;
use web_sys::window;
use web_sys::{MouseEvent, Window, Navigator, Clipboard};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use serde_json::json;
use gloo_timers::future::TimeoutFuture;
use web_sys::js_sys::eval;
use crate::config;
use crate::utils::api::Api;

#[derive(Properties, PartialEq)]
pub struct ServerSelfHostInstructionsProps {
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub sub_tier: Option<String>,
    #[prop_or_default]
    pub server_ip: Option<String>,
    #[prop_or_default]
    pub user_id: Option<String>,
    #[prop_or_default]
    pub message: String,
    #[prop_or_default]
    pub on_update: Option<Callback<()>>,
}

#[function_component(ServerSelfHostInstructions)]
pub fn server_self_host_instructions(props: &ServerSelfHostInstructionsProps) -> Html {
    let has_valid_server_ip = props.server_ip.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    let show_wizard = use_state(|| !has_valid_server_ip);
    let current_step = use_state(|| 0u32);
    let server_ip = use_state(|| props.server_ip.clone().unwrap_or_default());
    let save_status = use_state(|| None::<Result<(), String>>);
    let is_mobile = use_state(|| false);

    {
        let is_mobile = is_mobile.clone();
        use_effect_with_deps(move |_| {
            if let Some(win) = window() {
                let width = win.inner_width().map(|v| v.as_f64().unwrap() as i32).unwrap_or(0);
                is_mobile.set(width < 968);
                let is_mobile = is_mobile.clone();
                let win = win.clone();
                let closure = Closure::wrap(Box::new(move || {
                    if let Some(w) = window() {
                        let width = w.inner_width().map(|v| v.as_f64().unwrap() as i32).unwrap_or(0);
                        is_mobile.set(width < 968);
                    }
                }) as Box<dyn FnMut()>);
                let _ = win.add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref());
                closure.forget();
            }
            || ()
        }, ());
    }

    let on_input_change = {
        let server_ip = server_ip.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            server_ip.set(input.value());
        })
    };

    let on_save = {
        let server_ip = server_ip.clone();
        let save_status = save_status.clone();
        let show_wizard = show_wizard.clone();
        let on_update = props.on_update.clone();
        Callback::from(move |_: ()| {
            let server_ip = server_ip.clone();
            let save_status = save_status.clone();
            let show_wizard = show_wizard.clone();
            let on_update = on_update.clone();
         
            save_status.set(None);
         
            spawn_local(async move {
                let result = Api::post("/api/profile/server-ip")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&json!({
                        "server_ip": *server_ip
                    })).unwrap())
                    .send()
                    .await;
                match result {
                    Ok(response) => {
                        if response.status() == 401 {
                            if let Some(window) = window() {
                                if let Ok(Some(storage)) = window.local_storage() {
                                    let _ = storage.remove_item("token");
                                }
                            }
                            save_status.set(Some(Err("Session expired. Please log in again.".to_string())));
                        } else if response.ok() {
                            save_status.set(Some(Ok(())));
                            if let Some(cb) = &on_update {
                                cb.emit(());
                            }
                            show_wizard.set(false);
                        } else {
                            save_status.set(Some(Err("Failed to save server IP".to_string())));
                        }
                    }
                    Err(_) => {
                        save_status.set(Some(Err("Network error occurred".to_string())));
                    }
                }
            });
        })
    };

    let on_step_change = {
        let current_step = current_step.clone();
        Callback::from(move |step: u32| {
            current_step.set(step);
        })
    };

    let on_back = {
        let current_step = current_step.clone();
        Callback::from(move |_| {
            let new_step = (*current_step).saturating_sub(1);
            current_step.set(new_step);
        })
    };

    let on_next_or_finish = {
        let current_step = current_step.clone();
        let on_save = on_save.clone();
        Callback::from(move |_| {
            let step = *current_step;
            if step == 4 {
                on_save.emit(());
            } else {
                let new_step = step + 1;
                current_step.set(new_step);
            }
        })
    };

    let on_change_server = {
        let show_wizard = show_wizard.clone();
        let current_step = current_step.clone();
        Callback::from(move |_| {
            show_wizard.set(true);
            current_step.set(4);
        })
    };

    let domain = if let Some(user_id) = &props.user_id {
        format!("{}.lightfriend.ai", user_id)
    } else {
        "Loading...".to_string()
    };
    let copied = use_state(|| false);
    let on_copy = {
        let copied = copied.clone();
        let domain = domain.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(window) = window() {
                let clipboard = window.navigator().clipboard();
                let _ = clipboard.write_text(&domain);
                copied.set(true);
                let copied = copied.clone();
                spawn_local(async move {
                    TimeoutFuture::new(2000).await;
                    copied.set(false);
                });
            }
        })
    };

    let is_tier_3 = props.sub_tier.as_deref() == Some("tier 3");

    let step_titles = vec![
        "Setting Up Your Server on Hostinger".to_string(),
        "Choose Correct Server".to_string(),
        "Choose settings for the server".to_string(),
        "Access Your Server".to_string(),
        "Copy your server's IP address".to_string(),
    ];

    let open_pricing = {
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let url = "https://www.hostinger.com/pricing/vps-hosting".to_string();
            let js = format!(r#"
                let popup = window.open('{}', 'rightWindow', 'resizable=yes,scrollbars=yes');
                if (popup) {{
                    const screenWidth = window.screen.availWidth;
                    const screenHeight = window.screen.availHeight;
                    const width = Math.floor(screenWidth * 2 / 3);
                    const height = screenHeight;
                    const left = Math.floor(screenWidth / 3);
                    const top = 0;
                    popup.resizeTo(width, height);
                    popup.moveTo(left, top);
                }}
            "#, url);
            let _ = eval(&js);
        })
    };

    let step_text = {
        let on_input_change = on_input_change.clone();
        let on_save = on_save.clone();
        let server_ip = server_ip.clone();
        let save_status = save_status.clone();
        let domain = domain.clone();
        let on_copy = on_copy.clone();
        let copied = copied.clone();
        let is_tier_3 = is_tier_3;
        let open_pricing = open_pricing.clone();
        move |step: u32| -> Html {
            match step {
                0 => html! {
                    <div class="step-text">
                        <p>{"Follow these steps to set up your own server for Lightfriend. No prerequisite knowledge is needed."}</p>
                        <p>{"As shown in the animation, it's best to drag this window to the left side of your screen, leaving space on the right for the Hostinger page in the next steps."}</p>
                    </div>
                },
                1 => html! {
                    <div class="step-text">
                        <ul>
                            <li>
                                {"Go to "}
                                <a
                                    href="#"
                                    style="color: #1E90FF; text-decoration: underline;"
                                    onclick={open_pricing.clone()}
                                >
                                    {"Hostinger's pricing page"}
                                </a>
                            </li>
                            <li>{"1. Choose your preferred length of subscription"}</li>
                            <li>{"2. Click on the KVM 1 Plan"}</li>
                        </ul>
                    </div>
                },
                2 => html! {
                    <div class="step-text">
                        <ul>
                            <li>{"1. With 'OS With Panel' selected choose Coolify"}</li>
                            <li>{"3. Click 'Continue', make the payment and wait for 5 minutes for the server to build"}</li>
                        </ul>
                    </div>
                },
                3 => html! {
                    <div class="step-text">
                        <ul>
                            <li>{"Once the server is ready (wait about 5 minutes), click on 'Manage VPS'"}</li>
                        </ul>
                    </div>
                },
                4 => html! {
                    <div class="step-text">
                        <ul>
                            <li>{"Make sure 'Overview' is selected, scroll down and copy the IPV4 address"}</li>
                        </ul>
                        { if is_tier_3 {
                            html! {
                                <div class="input-field">
                                    <label for="server-ip">{"Your Server's IP Address:"}</label>
                                    <div class="input-with-button">
                                        <input
                                            type="text"
                                            id="server-ip"
                                            placeholder="Enter your server's IP address"
                                            value={(*server_ip).clone()}
                                            onchange={on_input_change.clone()}
                                        />
                                        <button
                                            class="save-button"
                                            onclick={let on_save = on_save.clone(); Callback::from(move |_: MouseEvent| on_save.emit(()))}
                                        >
                                            {"Save"}
                                        </button>
                                        {
                                            match &*save_status {
                                                Some(Ok(_)) => html! {
                                                    <span class="save-status success">{"✓ Saved"}</span>
                                                },
                                                Some(Err(err)) => html! {
                                                    <span class="save-status error">{format!("Error: {}", err)}</span>
                                                },
                                                None => html! {}
                                            }
                                        }
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        } }
                        <div class="domain-container">
                            <p class="highlight-text">{"Your Lightfriend Subdomain: "}{ domain.clone() }</p>
                            <button class="copy-button" onclick={on_copy.clone()}>{"Copy"}</button>
                            { if *copied { html! { <span class="copy-status">{"Copied!"}</span> } } else { html! {} } }
                        </div>
                        <p class="note-text">
                            {"Your subdomain is configured to point to your server. After setup with Coolify, access Lightfriend at your subdomain."}
                        </p>
                    </div>
                },
                _ => html! {},
            }
        }
    };

    let step_image = {
        let on_step_change = on_step_change.clone();
        move |step: u32| -> Html {
            match step {
                0 => html! { 
                    <div class="instruction-image">
                        <img
                            src="/assets/hostingersetupstep1.gif"
                            alt="Hostinger Pricing Page"
                            loading="lazy"
                        />

                    </div> 
                },
                1 => html! {
                    <div class="instruction-image">
                        <img
                            src="/assets/billing-hostinger.png"
                            alt="Hostinger Pricing Page"
                            loading="lazy"
                        />
                    </div>
                },
                2 => html! {
                    <div class="instruction-image">
                        <img
                            src="/assets/coolify-hostinger-settings.png"
                            alt="Server settings selection"
                            loading="lazy"
                        />
                    </div>
                },
                3 => html! {
                    <div class="instruction-image">
                        <img
                            src="/assets/server-ready-hostinger.png"
                            alt="Manage VPS Page"
                            loading="lazy"
                        />
                    </div>
                },
                4 => html! {
                    <div class="instruction-image">
                        <img
                            src="/assets/server-ip-hostinger.png"
                            alt="Server IP Location"
                            loading="lazy"
                        />
                    </div>
                },
                _ => html! { <div class="image-placeholder">{"No image"}</div> },
            }
        }
    };

    html! {
        <div class="self-host-section">
            { if !props.message.is_empty() {
                html! {
                    <div class="applicable-message">
                        { props.message.clone() }
                    </div>
                }
            } else {
                html! {}
            } }
            { if *show_wizard {
                html! {
                    <div class="tutorial-container">
                        <div class="wizard-header">
                            <h2 class="sidebar-title">{"Setting Up Your Server on Hostinger"}</h2>
                            <div class="estimated-time">{"Estimated time: 8 mins"}</div>
                            <div class="progress-bar">
                                { (0..5u32).map(|i| {
                                    let is_completed = i < *current_step;
                                    let is_current = i == *current_step;
                                    html! {
                                        <div 
                                            class={if is_completed { "progress-step completed" } else if is_current { "progress-step current" } else { "progress-step" }}
                                            onclick={let on_step_change = on_step_change.clone(); Callback::from(move |_| on_step_change.emit(i))}
                                        >
                                            <span class="step-number">{ i + 1 }</span>
                                            { if is_completed {
                                                html! { <span class="checkmark">{"✓"}</span> }
                                            } else {
                                                html! {}
                                            }}
                                        </div>
                                    }
                                }).collect::<Html>() }
                            </div>
                        </div>
                        <div class="nav-top">
                            <button class="nav-button back" disabled={*current_step == 0} onclick={on_back.clone()}>
                                {"Back"}
                            </button>
                            <button class="nav-button next" onclick={on_next_or_finish.clone()}>
                                { if *current_step == 4 { "Finish" } else { "Next" } }
                            </button>
                        </div>
                        <div class="image-area">
                            { step_image(*current_step) }
                        </div>
                        <div class="step-text-container">
                            { step_text(*current_step) }
                        </div>
                    </div>
                }
            } else if has_valid_server_ip {
                html! {
                    <>
                        <div class="instruction-block compact-block">
                            <div class="instruction-content">
                                <h2>{"Self-Hosted Server"}</h2>
                                <p>{"Server IP: "}{props.server_ip.as_ref().unwrap()}</p>
                                <div class="domain-container">
                                    <p class="highlight-text">{"Your Lightfriend Subdomain: "}{ domain.clone() }</p>
                                    <button class="copy-button" onclick={on_copy.clone()}>{"Copy"}</button>
                                    { if *copied { html! { <span class="copy-status">{"Copied!"}</span> } } else { html! {} } }
                                </div>
                                <p class="note-text">
                                    {"Your subdomain is configured to point to your server."}
                                </p>
                                <button
                                    class="save-button change-button"
                                    onclick={on_change_server.clone()}
                                >
                                    {"Change Server IP"}
                                </button>
                            </div>
                        </div>
                    </>
                }
            } else {
                html! {}
            } }
            <style>
                {r#"
                .self-host-section {
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }
                .tutorial-container {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                    padding: 2rem;
                    width: 100%;
                    max-width: 800px;
                    margin: 0 auto;
                }
                .wizard-header {
                    text-align: center;
                    padding-bottom: 1rem;
                    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                }
                .progress-bar {
                    display: flex;
                    justify-content: space-between;
                    margin: 1rem 0;
                }
                .progress-step {
                    flex: 1;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    gap: 0.5rem;
                    padding: 0.5rem;
                    border-radius: 4px;
                    transition: all 0.3s ease;
                    color: #ccc;
                    cursor: pointer;
                }
                .progress-step:hover {
                    background: rgba(255, 255, 255, 0.05);
                }
                .progress-step.completed {
                    color: #4CAF50;
                    background: rgba(76, 175, 80, 0.1);
                }
                .progress-step.current {
                    color: #1E90FF;
                    background: rgba(30, 144, 255, 0.1);
                    border: 1px solid #1E90FF;
                }
                .step-number {
                    font-weight: bold;
                    font-size: 1.1rem;
                }
                .checkmark {
                    font-size: 1.2rem;
                }
                .nav-top {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    padding: 1rem;
                    background: rgba(26, 26, 26, 0.95);
                    border-radius: 8px;
                }
                .image-area {
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    padding: 1rem;
                    min-height: 400px;
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    border-radius: 12px;
                    overflow: hidden;
                }
                .image-placeholder {
                    width: 100%;
                    height: 300px;
                    background: rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    color: #666;
                    font-style: italic;
                }
                .instruction-image {
                    width: 100%;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }
                .instruction-image img {
                    max-width: 100%;
                    height: auto;
                    border-radius: 8px;
                    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
                }
                .step-text-container {
                    padding: 1rem;
                    background: rgba(255, 255, 255, 0.02);
                    border-radius: 12px;
                    border: 1px solid rgba(255, 255, 255, 0.1);
                }
                .step-text ul {
                    list-style: none;
                    padding: 0;
                    margin: 1rem 0;
                }
                .step-text li {
                    color: #999;
                    padding: 0.5rem 0;
                    padding-left: 1.5rem;
                    position: relative;
                    line-height: 1.6;
                }
                .step-text li::before {
                    content: '•';
                    position: absolute;
                    left: 0.5rem;
                    color: #1E90FF;
                }
                .step-text p {
                    color: #ccc;
                    line-height: 1.6;
                    margin: 1rem 0;
                }
                .step-text a {
                    color: #1E90FF;
                    text-decoration: underline;
                }
                .nav-button {
                    padding: 0.75rem 1.5rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }
                .nav-button:hover:not(:disabled) {
                    background: #1976D2;
                }
                .nav-button:disabled {
                    opacity: 0.5;
                    cursor: not-allowed;
                }
                .nav-button.back {
                    background: #666;
                }
                .nav-button.back:hover:not(:disabled) {
                    background: #555;
                }
                .self-host-section .instruction-block {
                    display: flex;
                    align-items: center;
                    gap: 2rem;
                    margin-bottom: 2rem;
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    padding: 2rem;
                    transition: all 0.3s ease;
                }
                .self-host-section .instruction-block:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                }
                .self-host-section .instruction-block.compact-block {
                    gap: 1rem;
                    padding: 1.5rem;
                }
                .self-host-section .instruction-content {
                    flex: 1;
                    order: 1;
                }
                .self-host-section .instruction-image {
                    flex: 1;
                    order: 2;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }
                .self-host-section .instruction-content h2 {
                    font-size: 1.75rem;
                    margin-bottom: 1rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .self-host-section .instruction-content ul {
                    list-style: none;
                    padding: 0;
                }
                .self-host-section .instruction-content li {
                    color: #999;
                    padding: 0.5rem 0;
                    padding-left: 1.5rem;
                    position: relative;
                    line-height: 1.6;
                }
                .self-host-section .instruction-content li::before {
                    content: '•';
                    position: absolute;
                    left: 0.5rem;
                    color: #1E90FF;
                }
                .self-host-section .instruction-image img {
                    max-width: 100%;
                    height: auto;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }
                .self-host-section .instruction-image img:hover {
                    transform: scale(1.02);
                }
                .self-host-section .input-field {
                    margin-top: 1rem;
                }
                .self-host-section .input-field label {
                    display: block;
                    margin-bottom: 0.5rem;
                    color: #7EB2FF;
                }
                .self-host-section .input-with-button {
                    display: flex;
                    gap: 0.5rem;
                }
                .self-host-section .input-with-button input {
                    flex: 1;
                    padding: 0.75rem;
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    border-radius: 6px;
                    background: rgba(26, 26, 26, 0.5);
                    color: #fff;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }
                .self-host-section .input-with-button input:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.2);
                }
                .self-host-section .input-with-button input::placeholder {
                    color: rgba(255, 255, 255, 0.3);
                }
                .self-host-section .save-button {
                    padding: 0.75rem 1.5rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }
                .self-host-section .save-button.change-button {
                    padding: 0.5rem 1rem;
                    font-size: 0.9rem;
                    align-self: flex-start;
                }
                .self-host-section .save-button:hover {
                    background: #1976D2;
                }
                .self-host-section .save-button:active {
                    transform: translateY(1px);
                }
                .self-host-section .save-status {
                    margin-left: 1rem;
                    padding: 0.5rem 1rem;
                    border-radius: 4px;
                    font-size: 0.9rem;
                }
                .self-host-section .save-status.success {
                    color: #4CAF50;
                    background: rgba(76, 175, 80, 0.1);
                }
                .self-host-section .save-status.error {
                    color: #f44336;
                    background: rgba(244, 67, 54, 0.1);
                }
                .self-host-section .highlight-text {
                    font-size: 1.2rem;
                    color: #1E90FF;
                    padding: 0.5rem;
                    background: rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    margin: 0.5rem 0;
                    text-align: left;
                }
                .self-host-section .note-text {
                    color: #7EB2FF;
                    font-style: italic;
                    margin-top: 1rem;
                    padding-left: 1rem;
                    border-left: 3px solid rgba(126, 178, 255, 0.3);
                }
                .self-host-section .domain-container {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 1rem;
                    margin: 1rem 0;
                }
                .self-host-section .copy-button {
                    padding: 0.5rem 1rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }
                .self-host-section .copy-button:hover {
                    background: #1976D2;
                }
                .self-host-section .copy-status {
                    color: #4CAF50;
                    font-size: 1rem;
                }
                .self-host-section .applicable-message {
                    color: #ffcc00;
                    font-size: 1.2rem;
                    margin-bottom: 1rem;
                    text-align: center;
                    padding: 1rem;
                    background: rgba(255, 204, 0, 0.1);
                    border: 1px solid rgba(255, 204, 0, 0.3);
                    border-radius: 6px;
                }
                .sidebar-title {
                    font-size: 1.5rem;
                    font-weight: bold;
                    margin-bottom: 0.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .estimated-time {
                    color: #999;
                    margin-bottom: 1rem;
                    font-size: 0.9rem;
                }
                @media (max-width: 968px) {
                    .tutorial-container {
                        padding: 1rem;
                    }
                    .image-area {
                        min-height: 250px;
                    }
                    .self-host-section .instruction-block {
                        flex-direction: column;
                        gap: 1rem;
                    }
                    .self-host-section .instruction-content {
                        order: 1;
                    }
                    .self-host-section .instruction-image {
                        order: 2;
                    }
                    .self-host-section .instruction-content h2 {
                        font-size: 1.5rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}
