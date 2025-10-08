use yew::prelude::*;
use crate::auth::connect::Connect;
use yew_router::prelude::*;
use crate::Route;
use yew_router::components::Link;
use crate::config;
use web_sys::{window, HtmlInputElement};
use gloo_net::http::Request;
use serde_json::{json, Value};
use wasm_bindgen_futures::spawn_local;
use crate::pages::landing::Landing;
use crate::profile::settings::SettingsPage;
use crate::profile::billing_models::UserProfile;
use crate::pages::server_self_host_instructions::ServerSelfHostInstructions;

fn render_notification_settings(profile: Option<&UserProfile>) -> Html {
    html! {
        <div style="margin-top: 2rem; padding: 1.5rem; background: rgba(30, 30, 30, 0.7); border: 1px solid rgba(30, 144, 255, 0.1); border-radius: 12px; margin-bottom: 2rem;">
            {
                if let Some(profile) = profile {
                    html! {
                        <>
                            <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem;">
                                <span style="color: white;">{"Notifications"}</span>
                                    <label class="switch">
                                        <input
                                            type="checkbox"
                                            checked={profile.notify}
                                            onchange={{
                                                let user_id = profile.id;
                                                Callback::from(move |e: Event| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    let notify = input.checked();
                                           
                                                    if let Some(token) = window()
                                                        .and_then(|w| w.local_storage().ok())
                                                        .flatten()
                                                        .and_then(|storage| storage.get_item("token").ok())
                                                        .flatten()
                                                    {
                                                        spawn_local(async move {
                                                            let _ = Request::post(&format!("{}/api/profile/update-notify/{}", config::get_backend_url(), user_id))
                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                .header("Content-Type", "application/json")
                                                                .json(&json!({"notify": notify}))
                                                                .expect("Failed to serialize notify request")
                                                                .send()
                                                                .await;
                                                        });
                                                    }
                                                })
                                            }}
                                        />
                                        <span class="slider round"></span>
                                    </label>
                            </div>
                            <p style="color: #999; font-size: 0.9rem; margin-top: 0.5rem;">
                                {"Receive notifications about new feature updates."}
                            </p>
                        </>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

#[derive(Clone, PartialEq)]
enum DashboardTab {
    Connections,
    Personal,
}

pub fn is_logged_in() -> bool {
    if let Some(window) = window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(_token)) = storage.get_item("token") {
                return true;
            }
        }
    }
    false
}

#[derive(Properties, PartialEq, Clone)]
pub struct MagicLinkProps {
    pub link: Option<String>,
    pub error: Option<String>,
    pub on_regenerate: Callback<()>,
}

#[function_component]
fn MagicLinkSection(props: &MagicLinkProps) -> Html {
    let link = &props.link;
    let on_regenerate = props.on_regenerate.clone();
    html! {
        <div class="magic-link-item" style="position: relative; width: 100%;">
            <span class="credit-label">{"Self-Hosted Login Link"}</span>
            if let Some(l) = link.as_ref() {
                <>
                    <div style="display: flex; align-items: center; gap: 0.5rem; margin-top: 0.5rem; flex-wrap: wrap;">
                        <a href={l.clone()} target="_blank" rel="noopener noreferrer" style="color: #7EB2FF; text-decoration: none; font-size: 0.9rem; word-break: break-all; max-width: 200px;">
                            {l.clone()}
                        </a>
                        <button onclick={Callback::from(move |_| on_regenerate.emit(()))} style="padding: 0.25rem 0.5rem; background: rgba(255, 68, 68, 0.2); border: 1px solid rgba(255, 68, 68, 0.3); border-radius: 4px; color: white; font-size: 0.8rem; cursor: pointer;">
                            {"Regenerate"}
                        </button>
                    </div>
                </>
            } else if let Some(err) = props.error.as_ref() {
                <span style="color: #ff4444; font-size: 0.8rem;">{err}</span>
            } else {
                <span style="color: #999; font-size: 0.9rem;">{"Loading..."}</span>
            }
            <div class="credit-tooltip">
                {"Click the link to log in to your self-hosted instance. Regenerate if expired or compromised. Single-use for security."}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct MonthlyCreditsProps {
    pub profile: UserProfile,
}

#[function_component]
fn MonthlyCredits(props: &MonthlyCreditsProps) -> Html {
    let profile = &props.profile;
    html! {
        <div class="credit-item" tabindex="0">
            <span class="credit-label">{"Monthly Message Quota"}</span>
            <span class="credit-value">{profile.credits_left as i32}{" Messages"}</span>
            {
                if profile.digests_reserved > 0 {
                    html! {
                        <span class="credit-label">{" + ("}{profile.digests_reserved as i32}{" reserved for digests)"}</span>
                    }
                } else {
                    html! {}
                }
            }
            {
                if let Some(days) = profile.days_until_billing {
                    html! {
                        <span class="reset-timer">
                            {
                                if days == 0 {
                                    "Resets today".to_string()
                                } else if days == 1 {
                                    "Resets in 1 day".to_string()
                                } else {
                                    format!("Resets in {} days", days)
                                }
                            }
                        </span>
                    }
                } else {
                    html! {}
                }
            }
            <div class="credit-tooltip">
                {"Your monthly quota Message quota. Can be used to ask questions, voice calls(1 Message = 1 minute) or to receive priority sender notifications(1 Message = 2 notifications). Not enough? Buy overage credits or trade in unused digest slots for Messages."}
            </div>
        </div>
    }
}

#[function_component]
pub fn Home() -> Html {
    let logged_in = is_logged_in();
    let profile_data = use_state(|| None::<UserProfile>);
    let user_verified = use_state(|| true);
    let error = use_state(|| None::<String>);
    let is_expanded = use_state(|| false);
    let active_tab = use_state(|| DashboardTab::Connections);
    let navigator = use_navigator().unwrap();
    let magic_link = use_state(|| None::<String>);
    let magic_error = use_state(|| None::<String>);
    let refetch_profile = {
        let profile_data = profile_data.clone();
        let user_verified = user_verified.clone();
        let error = error.clone();
        let magic_link = magic_link.clone();
        let magic_error = magic_error.clone();
        Callback::from(move |_| {
            let profile_data = profile_data.clone();
            let user_verified = user_verified.clone();
            let error = error.clone();
            let magic_link = magic_link.clone();
            let magic_error = magic_error.clone();
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::get(&format!("{}/api/profile", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await;
                    match result {
                        Ok(response) => {
                            if response.status() == 401 {
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                    }
                                    let _ = window.location().set_href("/");
                                }
                                return;
                            }
                            match response.json::<UserProfile>().await {
                                Ok(profile) => {
                                    user_verified.set(profile.verified);
                                    profile_data.set(Some(profile.clone()));
                                    error.set(None);
                                    if profile.sub_tier.as_deref() == Some("tier 3") {
                                        let token = token.clone();
                                        spawn_local(async move {
                                            let result = Request::get(&format!("{}/api/profile/magic-link", config::get_backend_url()))
                                                .header("Authorization", &format!("Bearer {}", token))
                                                .send()
                                                .await;
                                            match result {
                                                Ok(resp) => {
                                                    if resp.ok() {
                                                        match resp.json::<Value>().await {
                                                            Ok(data) => {
                                                                if let Some(l) = data["link"].as_str() {
                                                                    magic_link.set(Some(l.to_string()));
                                                                }
                                                            }
                                                            Err(_) => {
                                                                magic_error.set(Some("Failed to parse link".to_string()));
                                                            }
                                                        }
                                                    } else {
                                                        magic_error.set(Some("Failed to fetch link".to_string()));
                                                    }
                                                }
                                                Err(_) => {
                                                    magic_error.set(Some("Network error".to_string()));
                                                }
                                            }
                                        });
                                    }
                                }
                                Err(_) => {
                                    error.set(Some("Failed to parse profile data".to_string()));
                                }
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch profile".to_string()));
                        }
                    }
                }
            });
        })
    };
    // Single profile fetch effect
    {
        let profile_data = profile_data.clone();
        let user_verified = user_verified.clone();
        let error = error.clone();
        let magic_link = magic_link.clone();
        let magic_error = magic_error.clone();
        use_effect_with_deps(move |_| {
            let profile_data = profile_data.clone();
            let user_verified = user_verified.clone();
            let error = error.clone();
            let magic_link = magic_link.clone();
            let magic_error = magic_error.clone();
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::get(&format!("{}/api/profile", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await;
                    match result {
                        Ok(response) => {
                            if response.status() == 401 {
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                    }
                                    let _ = window.location().set_href("/");
                                }
                                return;
                            }
                   
                            match response.json::<UserProfile>().await {
                                Ok(profile) => {
                                    user_verified.set(profile.verified);
                                    profile_data.set(Some(profile.clone()));
                                    error.set(None);
                                    // Fetch magic link if tier 3
                                    if profile.sub_tier.as_deref() == Some("tier 3") {
                                        let token = token.clone();
                                        spawn_local(async move {
                                            let result = Request::get(&format!("{}/api/profile/magic-link", config::get_backend_url()))
                                                .header("Authorization", &format!("Bearer {}", token))
                                                .send()
                                                .await;
                                            match result {
                                                Ok(resp) => {
                                                    if resp.ok() {
                                                        match resp.json::<Value>().await {
                                                            Ok(data) => {
                                                                if let Some(l) = data["link"].as_str() {
                                                                    magic_link.set(Some(l.to_string()));
                                                                }
                                                            }
                                                            Err(_) => {
                                                                magic_error.set(Some("Failed to parse link".to_string()));
                                                            }
                                                        }
                                                    } else {
                                                        magic_error.set(Some("Failed to fetch link".to_string()));
                                                    }
                                                }
                                                Err(_) => {
                                                    magic_error.set(Some("Network error".to_string()));
                                                }
                                            }
                                        });
                                    }
                                }
                                Err(_) => {
                                    error.set(Some("Failed to parse profile data".to_string()));
                                }
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch profile".to_string()));
                        }
                    }
                }
            });
   
            || ()
        }, ());
    }
    // If not logged in, show landing page
    if !logged_in {
        html! { <Landing /> }
    } else if !*user_verified {
        // If logged in but not verified, redirect to verify page
        navigator.push(&Route::Verify);
        html! {}
    } else {
        html! {
            <>
                <div class="dashboard-container">
                    <h1 class="panel-title">{"Dashboard"}</h1>
                    <div class="status-section">
                        {
                            if let Some(profile) = (*profile_data).as_ref() {
                                if profile.sub_tier.as_deref() != Some("tier 3") {
                                    html! {
                                        <div class="credits-info">
                                            <div class="credits-grid">
                                                {
                                                    if let Some(ref _tier) = profile.sub_tier {
                                                        html! {
                                                            <>
                                                                {
                                                                    if profile.days_until_billing.is_some() || profile.credits_left > 0.0 {
                                                                        html! { <MonthlyCredits profile={profile.clone()} /> }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }
                                                                {
                                                                    if profile.credits > 0.00 {
                                                                        html! {
                                                                            <div class="credit-item" tabindex="0">
                                                                                <span class="credit-label">{"Overage Credits"}</span>
                                                                                <span class="credit-value">{format!("{:.2}€", profile.credits)}</span>
                                                                                <div class="credit-tooltip">
                                                                                    {"Your overage credits. Check how they are used in the pricing page under 'Message Costs (Credits)'."}
                                                                                </div>
                                                                            </div>
                                                                        }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }
                                                                <div class="credit-item" tabindex="0">
                                                                    <span class="credit-label">{"Lightfriend's Number"}</span>
                                                                    {
                                                                        if let Some(num) = &profile.preferred_number {
                                                                            html! {
                                                                                <span class="credit-value" style="color: #7EB2FF;">{num}</span>
                                                                            }
                                                                        } else {
                                                                            html! {
                                                                                <span class="credit-value" style="color: #ff4444;">{"No number configured"}</span>
                                                                            }
                                                                        }
                                                                    }
                                                                    <div class="credit-tooltip">
                                                                        {"Your lightfriend's phone number for receiving messages."}
                                                                    </div>
                                                                </div>
                                                            </>
                                                        }
                                                    } else {
                                                        html! {
                                                            <>
                                                                {
                                                                    if profile.days_until_billing.is_some() || profile.credits_left > 0.0 {
                                                                        html! { <MonthlyCredits profile={profile.clone()} /> }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }
                                                                {
                                                                    if profile.credits > 0.00 {
                                                                        html! {
                                                                            <div class="credit-item" tabindex="0">
                                                                                <span class="credit-label">{"Overage Credits"}</span>
                                                                                <span class="credit-value">{format!("{:.2}€", profile.credits)}</span>
                                                                                <div class="credit-tooltip">
                                                                                    {"Your overage credits. Check how they are used in the pricing page under 'Message Costs (Credits)'."}
                                                                                </div>
                                                                            </div>
                                                                        }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }
                                                                <div class="subscription-promo">
                                                                    <Link<Route> to={Route::Pricing} classes="promo-link">
                                                                        {"Subscribe to start →"}
                                                                    </Link<Route>>
                                                                </div>
                                                            </>
                                                        }
                                                    }
                                                }
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
                        {
                            if let Some(profile) = (*profile_data).as_ref() {
                                if profile.sub_tier.as_deref() == Some("tier 3") {
                                    if profile.server_ip.is_some() {
                                        html! {
                                            <ServerSelfHostInstructions
                                                is_logged_in={true}
                                                sub_tier={profile.sub_tier.clone()}
                                                server_ip={profile.server_ip.clone()}
                                                user_id={Some(profile.id.to_string())}
                                                message={if profile.server_ip.is_none() {
                                                    "Set up your self-hosted server below.".to_string()
                                                } else {
                                                    String::new()
                                                }}
                                                on_update={Some(refetch_profile.clone())}
                                            />
                                        }
                                    } else {
                                        html! {
                                            <MagicLinkSection
                                                link={(*magic_link).clone()}
                                                error={(*magic_error).clone()}
                                                on_regenerate={{
                                                    let magic_link = magic_link.clone();
                                                    let magic_error = magic_error.clone();
                                                    let token = window().and_then(|w| w.local_storage().ok()).flatten().and_then(|s| s.get_item("token").ok()).flatten().unwrap_or_default();
                                                    Callback::from(move |_| {
                                                        let magic_link = magic_link.clone();
                                                        let magic_error = magic_error.clone();
                                                        let token = token.clone();
                                                        spawn_local(async move {
                                                            match Request::get(&format!("{}/api/profile/magic-link?regenerate=true", config::get_backend_url()))
                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                .send()
                                                                .await
                                                            {
                                                                Ok(resp) => {
                                                                    if resp.ok() {
                                                                        match resp.json::<Value>().await {
                                                                            Ok(data) => {
                                                                                if let Some(new_link) = data["link"].as_str() {
                                                                                    magic_link.set(Some(new_link.to_string()));
                                                                                    magic_error.set(None);
                                                                                }
                                                                            }
                                                                            Err(_) => {
                                                                                magic_error.set(Some("Failed to regenerate".to_string()));
                                                                            }
                                                                        }
                                                                    } else {
                                                                        magic_error.set(Some("Failed to regenerate".to_string()));
                                                                    }
                                                                }
                                                                Err(_) => {
                                                                    magic_error.set(Some("Network error".to_string()));
                                                                }
                                                            }
                                                        });
                                                    })
                                                }}
                                            />
                                        }
                                    }
                                } else {
                                    html! {}
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    {
                        if let Some(profile) = (*profile_data).as_ref() {
                            if profile.sub_tier.as_deref() != Some("tier 3") {
                                html! {
                                    <>
                                        <br/>
                                        <br/>
                                        <div class="dashboard-tabs">
                                            <button
                                                class={classes!("tab-button", (*active_tab == DashboardTab::Connections).then(|| "active"))}
                                                onclick={{
                                                    let active_tab = active_tab.clone();
                                                    Callback::from(move |_| active_tab.set(DashboardTab::Connections))
                                                }}
                                            >
                                                {"Connections"}
                                            </button>
                                            <button
                                                class={classes!("tab-button", (*active_tab == DashboardTab::Personal).then(|| "active"))}
                                                onclick={{
                                                    let active_tab = active_tab.clone();
                                                    Callback::from(move |_| active_tab.set(DashboardTab::Personal))
                                                }}
                                            >
                                                {"Personal"}
                                            </button>
                                        </div>
                                        {
                                            match *active_tab {
                                                DashboardTab::Connections => html! {
                                                    <div class="connections-tab">
                                                        {
                                                            if let Some(profile) = (*profile_data).as_ref() {
                                                                html! {
                                                                    <Connect user_id={profile.id} sub_tier={profile.sub_tier.clone()} discount={profile.discount} phone_number={profile.phone_number.clone()} estimated_monitoring_cost={profile.estimated_monitoring_cost.clone()}/>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                        <div class="feature-status">
                                                            <p class="feature-suggestion">
                                                                {"Have a feature in mind? Email your suggestions to "}
                                                                <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                                                            </p>
                                                            <h4>{"Tips"}</h4>
                                                            <ul>
                                                                <li>{"You can ask multiple questions in a single SMS to save money. Note that answers will be less detailed due to SMS character limits. Example: 'did sam altman tweet today and whats the weather?' -> 'Sam Altman hasn't tweeted today. Last tweet was on March 3, a cryptic \"!!!\" image suggesting a major AI development. Weather in Tampere: partly cloudy, 0°C, 82% humidity, wind at 4 m/s.'"}</li>
                                                                <li>{"Start your message with 'forget' to make the assistant forget previous conversation context and start fresh. Note that this only applies to that one message - the next message will again remember previous context."}</li>
                                                            </ul>
                                                        </div>
                                                        {
                                                            if let Some(profile) = (*profile_data).as_ref() {
                                                                if profile.sub_tier.is_some() {
                                                                    html! {
                                                                        <div class="subscriber-promo">
                                                                            <p>{"Subscribed users can get 20% off from Cold Turkey Blocker Pro with code LIGHTFRIEND"}</p>
                                                                            <a href="https://getcoldturkey.com" target="_blank" rel="noopener noreferrer">{"getcoldturkey.com"}</a>
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
                                                            if let Some(profile) = (*profile_data).as_ref() {
                                                                render_notification_settings(Some(profile))
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                    </div>
                                                },
                                                DashboardTab::Personal => html! {
                                                    <div class="personal-tab">
                                                        {
                                                            if let Some(profile) = (*profile_data).as_ref() {
                                                                html! {
                                                                    <SettingsPage
                                                                        user_profile={profile.clone()}
                                                                        on_profile_update={{
                                                                            let profile_data = profile_data.clone();
                                                                            Callback::from(move |updated_profile| {
                                                                                profile_data.set(Some(updated_profile));
                                                                            })
                                                                        }}
                                                                    />
                                                                }
                                                            } else {
                                                                html! {
                                                                    <div class="loading-profile">{"Loading profile..."}</div>
                                                                }
                                                            }
                                                        }
                                                    </div>
                                                }
                                            }
                                        }
                                    </>
                                }
                            } else {
                                html! {}
                            }
                        } else {
                            html! {}
                        }
                    }
                    <footer class="dashboard-footer">
                        <div class="development-links">
                            <p>{"Source code on "}
                                <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                            </p>
                            <div class="legal-links">
                                <a href="/terms">{"Terms & Conditions"}</a>
                                {" | "}
                                <a href="/privacy">{"Privacy Policy"}</a>
                                {" | "}
                                <a href="/updates">{"Updates"}</a>
                            </div>
                        </div>
                    </footer>
                </div>
                <style>
                    {r#"
                        .status-section {
                            display: flex;
                            align-items: center;
                            justify-content: space-between;
                            margin-bottom: 1.5rem;
                        }
                        .credits-info {
                            width: 100%;
                            margin-bottom: 1rem;
                        }
                        .credits-grid {
                            display: grid;
                            grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
                            gap: 1rem;
                            width: 100%;
                        }
                        .credit-item {
                            background: rgba(30, 144, 255, 0.05);
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            border-radius: 8px;
                            padding: 0.75rem;
                            display: flex;
                            flex-direction: column;
                            align-items: center;
                            gap: 0.25rem;
                            position: relative;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            outline: none;
                        }
                        .magic-link-item {
                            background: rgba(30, 144, 255, 0.05);
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            border-radius: 8px;
                            padding: 0.75rem;
                            display: flex;
                            flex-direction: column;
                            align-items: flex-start;
                            gap: 0.25rem;
                            position: relative;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            outline: none;
                            width: 100%;
                        }
                        .magic-link-item:hover {
                            background: rgba(30, 144, 255, 0.1);
                            border-color: rgba(30, 144, 255, 0.2);
                        }
                        .magic-link-item button:hover {
                            background: rgba(30, 144, 255, 0.3) !important;
                            transform: translateY(-1px);
                        }
                        .credit-item:hover,
                        .credit-item:focus {
                            background: rgba(30, 144, 255, 0.1);
                            border-color: rgba(30, 144, 255, 0.2);
                        }
                        .credit-tooltip {
                            position: absolute;
                            bottom: calc(100% + 10px);
                            left: 50%;
                            transform: translateX(-50%);
                            background: rgba(0, 0, 0, 0.9);
                            color: #fff;
                            padding: 1rem;
                            border-radius: 8px;
                            font-size: 0.80rem;
                            width: max-content;
                            max-width: 350px;
                            z-index: 1000;
                            opacity: 0;
                            visibility: hidden;
                            transition: all 0.3s ease;
                            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            text-align: center;
                        }
                        .credit-tooltip::after {
                            content: '';
                            position: absolute;
                            top: 100%;
                            left: 50%;
                            transform: translateX(-50%);
                            border-width: 8px;
                            border-style: solid;
                            border-color: rgba(0, 0, 0, 0.9) transparent transparent transparent;
                        }
                        .credit-item:hover .credit-tooltip,
                        .credit-item:focus .credit-tooltip {
                            opacity: 1;
                            visibility: visible;
                        }
                        .credit-label {
                            color: #999;
                            font-size: 0.8rem;
                            text-transform: uppercase;
                            letter-spacing: 0.5px;
                        }
                        .credit-value {
                            color: #7EB2FF;
                            font-size: 1.1rem;
                            font-weight: 500;
                        }
                        .reset-timer {
                            display: block;
                            color: #999;
                            font-size: 0.8rem;
                            margin-top: 0.3rem;
                        }
                        .credit-warning {
                            grid-column: 1 / -1;
                            color: #ff4444;
                            font-size: 0.9rem;
                            text-align: center;
                            padding: 0.5rem;
                            background: rgba(255, 68, 68, 0.1);
                            border-radius: 6px;
                            margin-top: 0.5rem;
                        }
                        .subscription-promo {
                            background: linear-gradient(45deg, rgba(30, 144, 255, 0.1), rgba(65, 105, 225, 0.1));
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            border-radius: 8px;
                            padding: 0.75rem 1.25rem;
                            margin-left: 1rem;
                            flex-shrink: 0;
                            grid-column: 1 / -1;
                        }
                        .promo-link {
                            color: #1E90FF;
                            text-decoration: none;
                            font-size: 0.9rem;
                            display: inline-flex;
                            align-items: center;
                            transition: all 0.3s ease;
                        }
                        .promo-link:hover {
                            color: #7EB2FF;
                            transform: translateX(5px);
                        }
                        .phone-selector {
                            margin: 1.5rem 0;
                        }
                        .dashboard-container {
                            min-height: 100vh;
                            border-radius: 12px;
                            background: #1a1a1a;
                            padding: 3rem 2rem;
                            width: 100%;
                            max-width: 800px;
                            margin: 4rem auto;
                        }
                        .panel-title {
                            font-size: 2.5rem;
                            background: linear-gradient(45deg, #fff, #7EB2FF);
                            -webkit-background-clip: text;
                            -webkit-text-fill-color: transparent;
                            margin: 0 0 1.5rem 0;
                            text-align: center;
                        }
                        .dashboard-tabs {
                            display: flex;
                            gap: 1rem;
                            margin-bottom: 2rem;
                            border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                            padding-bottom: 1rem;
                            flex-wrap: wrap;
                        }
                        .tab-button {
                            background: transparent;
                            border: none;
                            color: #999;
                            padding: 0.5rem 1rem;
                            cursor: pointer;
                            font-size: 1rem;
                            transition: all 0.3s ease;
                            position: relative;
                            white-space: nowrap;
                            flex: 1;
                            min-width: fit-content;
                        }
                        .tab-button::after {
                            content: '';
                            position: absolute;
                            bottom: -1rem;
                            left: 0;
                            width: 100%;
                            height: 2px;
                            background: transparent;
                            transition: background-color 0.3s ease;
                        }
                        .tab-button.active {
                            color: white;
                        }
                        .tab-button.active::after {
                            background: #1E90FF;
                        }
                        .tab-button:hover {
                            color: #7EB2FF;
                        }
                        .feature-status {
                            margin-top: 3rem;
                            text-align: left;
                            padding: 2rem;
                            background: rgba(30, 30, 30, 0.7);
                            border-radius: 12px;
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            backdrop-filter: blur(10px);
                        }
                        .feature-status h4 {
                            color: #7EB2FF;
                            font-size: 0.9rem;
                            margin: 1rem 0 0.5rem 0;
                        }
                        .feature-status ul {
                            list-style: none;
                            padding: 0;
                            margin: 0 0 1.5rem 0;
                        }
                        .feature-status li {
                            color: #999;
                            font-size: 0.9rem;
                            padding: 0.3rem 0;
                            padding-left: 1.5rem;
                            position: relative;
                        }
                        .feature-status li::before {
                            content: '•';
                            position: absolute;
                            left: 0.5rem;
                            color: #1E90FF;
                        }
                        .feature-suggestion {
                            margin-top: 1rem;
                            color: #999;
                            font-size: 0.9rem;
                        }
                        .feature-suggestion a {
                            color: #1E90FF;
                            text-decoration: none;
                            transition: color 0.3s ease;
                        }
                        .feature-suggestion a:hover {
                            color: #7EB2FF;
                            text-decoration: underline;
                        }
                        .subscriber-promo {
                            margin-top: 1rem;
                            padding: 1rem;
                            background: rgba(30, 144, 255, 0.05);
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            border-radius: 8px;
                            text-align: center;
                        }
                        .subscriber-promo p {
                            color: #fff;
                            margin-bottom: 0.5rem;
                        }
                        .subscriber-promo a {
                            color: #1E90FF;
                            text-decoration: none;
                        }
                        .subscriber-promo a:hover {
                            text-decoration: underline;
                        }
                        .development-links {
                            margin-top: 2rem;
                            font-size: 0.9rem;
                            color: #666;
                        }
                        .development-links p {
                            margin: 0.5rem 0;
                        }
                        .development-links a {
                            color: #007bff;
                            text-decoration: none;
                            position: relative;
                            padding: 0 2px;
                            transition: all 0.3s ease;
                        }
                        .development-links a::after {
                            content: '';
                            position: absolute;
                            width: 100%;
                            height: 1px;
                            bottom: -2px;
                            left: 0;
                            background: linear-gradient(90deg, #1E90FF, #4169E1);
                            transform: scaleX(0);
                            transform-origin: bottom right;
                            transition: transform 0.3s ease;
                        }
                        .development-links a:hover {
                            color: #7EB2FF;
                            text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
                        }
                        .development-links a:hover::after {
                            transform: scaleX(1);
                            transform-origin: bottom left;
                        }
                        .switch {
                            position: relative !important;
                            display: inline-block !important;
                            width: 60px !important;
                            height: 34px !important;
                            margin-left: 1rem !important;
                        }
                        .switch input {
                            opacity: 0 !important;
                            width: 0 !important;
                            height: 0 !important;
                        }
                        .slider {
                            position: absolute !important;
                            cursor: pointer !important;
                            top: 0 !important;
                            left: 0 !important;
                            right: 0 !important;
                            bottom: 0 !important;
                            background-color: #666 !important;
                            transition: .4s !important;
                            border-radius: 34px !important;
                            border: 1px solid rgba(255, 255, 255, 0.1) !important;
                        }
                        .slider:before {
                            position: absolute !important;
                            content: "" !important;
                            height: 26px !important;
                            width: 26px !important;
                            left: 4px !important;
                            bottom: 4px !important;
                            background-color: white !important;
                            transition: .4s !important;
                            border-radius: 50% !important;
                            box-shadow: 0 2px 5px rgba(0, 0, 0, 0.2) !important;
                        }
                        input:checked + .slider {
                            background-color: #1E90FF !important;
                        }
                        input:checked + .slider:before {
                            transform: translateX(26px) !important;
                        }
                        input:focus + .slider {
                            box-shadow: 0 0 1px rgba(30, 144, 255, 0.5) !important;
                        }
                        .slider.round {
                            border-radius: 34px !important;
                        }
                        .slider.round:before {
                            border-radius: 50% !important;
                        }
                        @media (max-width: 768px) {
                            .status-section {
                                flex-direction: column;
                                align-items: flex-start;
                            }
                            .subscription-promo {
                                margin: 1rem 0 0 0;
                                width: 100%;
                            }
                            .credit-item {
                                padding: 1rem;
                            }
                            .credit-tooltip {
                                position: fixed;
                                bottom: 20px;
                                left: 50%;
                                transform: translateX(-50%);
                                width: 90%;
                                max-width: 300px;
                                z-index: 1001;
                            }
                            .credit-tooltip::after {
                                display: none;
                            }
                            .dashboard-container {
                                padding: 2rem;
                            }
                            .panel-title {
                                font-size: 1.75rem;
                            }
                        }
                        @media (max-width: 480px) {
                            .dashboard-tabs {
                                gap: 0.5rem;
                                justify-content: center;
                            }
                            .tab-button {
                                padding: 0.5rem 0.75rem;
                                font-size: 0.9rem;
                            }
                        }
                    "#}
                </style>
            </>
        }
    }
}
