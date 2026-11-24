use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDateTime, NaiveTime, TimeZone, Utc};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use shared::{Event, NewEvent};

/// API origin. In the browser we use the page's own origin so the site works
/// from any host (localhost, Tailscale IP, or a real domain) with no rebuild.
/// The env var is only a fallback for non-browser builds (e.g. Android shell).
fn api_base() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(win) = web_sys::window() {
            if let Ok(origin) = win.location().origin() {
                if !origin.is_empty() {
                    return origin;
                }
            }
        }
    }
    match option_env!("EVENT_API_BASE_URL") {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => "http://localhost:8080".to_string(),
    }
}

const CITY_CENTER: [f64; 2] = [110.5076, -7.3307];
const WIB: i32 = 7 * 3600; // Asia/Jakarta, UTC+7, no DST

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct Bounds {
    west: f64,
    south: f64,
    east: f64,
    north: f64,
}

fn fmt_wib(dt: &DateTime<Utc>) -> String {
    let offset = FixedOffset::east_opt(WIB).unwrap();
    let local = dt.with_timezone(&offset);
    // Date-only events are stored at 00:00 (start) / 23:59 (end) — show just
    // the date, not a made-up time.
    if local.time() == NaiveTime::from_hms_opt(0, 0, 0).unwrap()
        || local.time() == NaiveTime::from_hms_opt(23, 59, 0).unwrap()
    {
        local.format("%a, %d %b %Y").to_string()
    } else {
        local.format("%a, %d %b %Y %H:%M").to_string()
    }
}

fn parse_local_datetime(value: &str) -> Option<DateTime<Utc>> {
    // <input type="datetime-local"> values are interpreted as city local time (WIB).
    let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M").ok()?;
    let offset = FixedOffset::east_opt(WIB).unwrap();
    offset
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
}

fn build_query(
    category: Option<String>,
    free_only: bool,
    preset: &str,
    bounds: Option<Bounds>,
) -> Vec<(String, String)> {
    let mut params: Vec<(String, String)> = Vec::new();
    if let Some(c) = category {
        params.push(("category".into(), c));
    }
    if free_only {
        params.push(("free_only".into(), "true".into()));
    }
    if let Some(b) = bounds {
        params.push(("west".into(), b.west.to_string()));
        params.push(("south".into(), b.south.to_string()));
        params.push(("east".into(), b.east.to_string()));
        params.push(("north".into(), b.north.to_string()));
    }

    let now = Utc::now();
    let wib = FixedOffset::east_opt(WIB).unwrap();
    let now_wib = now.with_timezone(&wib);
    let (from, to) = match preset {
        "today" => {
            let d = now_wib.date_naive();
            let start = wib.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap()).unwrap();
            let end = wib.from_local_datetime(&d.and_hms_opt(23, 59, 59).unwrap()).unwrap();
            (Some(start.with_timezone(&Utc)), Some(end.with_timezone(&Utc)))
        }
        "weekend" => {
            let weekday = now_wib.weekday().num_days_from_monday();
            let until_sat = (5 + 7 - weekday) % 7;
            let sat = now_wib.date_naive() + Duration::days(until_sat as i64);
            let sun = sat + Duration::days(1);
            let start = wib.from_local_datetime(&sat.and_hms_opt(0, 0, 0).unwrap()).unwrap();
            let end = wib.from_local_datetime(&sun.and_hms_opt(23, 59, 59).unwrap()).unwrap();
            (Some(start.with_timezone(&Utc)), Some(end.with_timezone(&Utc)))
        }
        _ => (None, None),
    };
    if let Some(f) = from {
        params.push(("from".into(), f.to_rfc3339()));
    }
    if let Some(t) = to {
        params.push(("to".into(), t.to_rfc3339()));
    }
    params
}

async fn fetch_events(params: Vec<(String, String)>) -> Result<Vec<Event>, String> {
    let resp = reqwest::Client::new()
        .get(format!("{}/api/events", api_base()))
        .query(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("API returned {}", resp.status()));
    }
    resp.json::<Vec<Event>>().await.map_err(|e| e.to_string())
}

async fn eval_script(script: String) {
    let _ = document::eval(&script).await;
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Data + UI state
    let mut events = use_signal(Vec::<Event>::new);
    let mut selected_id = use_signal(|| None::<String>);
    let mut loading = use_signal(|| true);
    let mut api_error = use_signal(|| None::<String>);
    let mut category = use_signal(|| None::<String>);
    let mut free_only = use_signal(|| false);
    let mut preset = use_signal(|| "all".to_string());
    let mut bounds = use_signal(|| None::<Bounds>);
    let mut map_ready = use_signal(|| false);
    let mut show_submit = use_signal(|| false);
    let mut show_admin = use_signal(|| false);
    let mut picked = use_signal(|| None::<(f64, f64)>);
    let mut pick_mode = use_signal(|| false);
    let mut move_token = use_signal(|| 0u64);
    let mut toast = use_signal(|| None::<String>);

    // Initialise the map after mount. Retry until map.js is available.
    use_effect(move || {
        spawn(async move {
            let init = format!(
                r#"(function(){{ const go = () => {{ if (window.EventMap) window.EventMap.init("map", [{},{}], 12); else setTimeout(go, 60); }}; go(); }})();"#,
                CITY_CENTER[0], CITY_CENTER[1]
            );
            eval_script(init).await;
        });
    });

    // Listen for browser events coming from map.js and route them into signals.
    use_effect(move || {
        spawn(async move {
            let mut eval = document::eval(
                r#"
                window.addEventListener("map-ready", () => dioxus.send({kind:"ready"}));
                window.addEventListener("event-selected", (e) => dioxus.send({kind:"select", id:String(e.detail)}));
                window.addEventListener("map-moved", (e) => dioxus.send({kind:"moved", bounds:e.detail}));
                window.addEventListener("map-clicked", (e) => dioxus.send({kind:"clicked", point:e.detail}));
                "#,
            );
            loop {
                let msg: serde_json::Value = match eval.recv().await {
                    Ok(v) => v,
                    Err(_) => break,
                };
                match msg.get("kind").and_then(|v| v.as_str()).unwrap_or("") {
                    "ready" => map_ready.set(true),
                    "select" => {
                        if let Some(id) = msg.get("id").and_then(|v| v.as_str()) {
                            selected_id.set(Some(id.to_string()));
                        }
                    }
                    "clicked" => {
                        if *pick_mode.peek() {
                            if let (Some(lat), Some(lng)) = (
                                msg.get("point").and_then(|p| p.get("lat")).and_then(|v| v.as_f64()),
                                msg.get("point").and_then(|p| p.get("lng")).and_then(|v| v.as_f64()),
                            ) {
                                picked.set(Some((lat, lng)));
                            }
                        }
                    }
                    "moved" => {
                        if let (Some(w), Some(s), Some(e), Some(n)) = (
                            msg.get("bounds").and_then(|b| b.get("west")).and_then(|v| v.as_f64()),
                            msg.get("bounds").and_then(|b| b.get("south")).and_then(|v| v.as_f64()),
                            msg.get("bounds").and_then(|b| b.get("east")).and_then(|v| v.as_f64()),
                            msg.get("bounds").and_then(|b| b.get("north")).and_then(|v| v.as_f64()),
                        ) {
                            let token = {
                                let t = *move_token.peek() + 1;
                                move_token.set(t);
                                t
                            };
                            spawn(async move {
                                #[cfg(target_arch = "wasm32")]
                                gloo_timers::future::TimeoutFuture::new(300).await;
                                // Native targets are not a shipping platform for this app;
                                // skip the debounce delay there.
                                if *move_token.peek() == token {
                                    bounds.set(Some(Bounds { west: w, south: s, east: e, north: n }));
                                }
                            });
                        }
                    }
                    _ => {}
                }
            }
        });
    });

    // Refetch when filters or the viewport change.
    use_effect(move || {
        let cat = category();
        let free = free_only();
        let p = preset();
        let b = bounds();
        spawn(async move {
            loading.set(true);
            let params = build_query(cat, free, &p, b);
            match fetch_events(params).await {
                Ok(mut list) => {
                    api_error.set(None);
                    // Maintenance notices always sort last, then by start time.
                    list.sort_by(|a, b| {
                        (a.category == "maintenance")
                            .cmp(&(b.category == "maintenance"))
                            .then(a.starts_at.cmp(&b.starts_at))
                    });
                    events.set(list);
                }
                Err(e) => api_error.set(Some(format!("Cannot load events: {e}"))),
            }
            loading.set(false);
        });
    });

    // Push the loaded events into MapLibre once the map is ready.
    use_effect(move || {
        let ready = map_ready();
        let list = events();
        if ready {
            spawn(async move {
                match serde_json::to_string(&list) {
                    Ok(json) => {
                        let script = format!("window.EventMap.setEvents({json});");
                        eval_script(script).await;
                    }
                    Err(_) => {}
                }
            });
        }
    });

    let selected_event = {
        let id = selected_id();
        events().into_iter().find(|e| Some(e.id.to_string()) == id)
    };

    let on_pick = move |id: String| {
        let coords = events
            .peek()
            .iter()
            .find(|e| e.id.to_string() == id)
            .map(|e| (e.lat, e.lng));
        selected_id.set(Some(id));
        if let Some((lat, lng)) = coords {
            spawn(async move {
                eval_script(format!("window.EventMap.flyTo({lat},{lng},14);")).await;
            });
        }
    };
    let open_submit = move |_| {
        picked.set(None);
        pick_mode.set(true);
        show_submit.set(true);
        spawn(async move { eval_script("window.EventMap.setPickMode(true);".into()).await; });
    };
    let close_submit = move |_| {
        show_submit.set(false);
        pick_mode.set(false);
        spawn(async move { eval_script("window.EventMap.setPickMode(false);".into()).await; });
    };

    rsx! {
        div { id: "map" }

        div { class: "topbar",
            div { class: "row",
                div { class: "brand", "Salatiga Event Map", small { "Discover what's on in the city" } }
                div { class: "spacer" }
                button { class: "icon-btn", title: "Near me", onclick: move |_| {
                    spawn(async move {
                        eval_script(
                            r#"navigator.geolocation.getCurrentPosition(
                                (p) => window.EventMap.map.flyTo([p.coords.longitude, p.coords.latitude], 14),
                                () => dioxus.send("Location unavailable"),
                                { enableHighAccuracy: true, timeout: 10000 }
                            );"#.to_string(),
                        ).await;
                    });
                }, "◎" }
                button { class: "icon-btn", title: "Submit an event", onclick: open_submit, "+" }
                button { class: "icon-btn", title: "Admin", onclick: move |_| show_admin.set(true), "⚙" }
            }
            div { class: "chips",
                button { class: chip_class(preset() == "all"), onclick: move |_| preset.set("all".into()), "All" }
                button { class: chip_class(preset() == "today"), onclick: move |_| preset.set("today".into()), "Today" }
                button { class: chip_class(preset() == "weekend"), onclick: move |_| preset.set("weekend".into()), "This weekend" }
                for c in ["music", "food", "arts", "community"] {
                    button {
                        class: chip_class(category().as_deref() == Some(c)),
                        onclick: move |_| {
                            let next = if category().as_deref() == Some(c) { None } else { Some(c.to_string()) };
                            category.set(next);
                        },
                        "{c}"
                    }
                }
                button { class: chip_class(free_only()), onclick: move |_| free_only.set(!free_only()), "Free" }
            }
        }

        if loading() && events().is_empty() {
            div { class: "state-box", "Loading events…" }
        }
        if !loading() && events().is_empty() && api_error().is_none() {
            div { class: "state-box", "No events here. Try zooming out." }
        }
        if let Some(err) = api_error() {
            div { class: "state-box", "{err}" }
        }
        if let Some(msg) = toast() {
            div { class: "state-box", "{msg}" }
        }

        EventList { events: events(), on_pick }

        if let Some(ev) = selected_event {
            DetailCard { event: ev, onclose: move |_| selected_id.set(None) }
        }

        if show_submit() {
            SubmitPanel {
                picked: picked(),
                onclose: close_submit,
                submitted: move |msg: String| {
                    show_submit.set(false);
                    pick_mode.set(false);
                    toast.set(Some(msg));
                    spawn(async move {
                        eval_script("window.EventMap.setPickMode(false);".into()).await;
                    });
                },
            }
        }

        if show_admin() {
            AdminPanel { onclose: move |_| show_admin.set(false) }
        }
    }
}

fn chip_class(active: bool) -> String {
    if active { "chip active".to_string() } else { "chip".to_string() }
}

#[component]
fn EventList(events: Vec<Event>, on_pick: EventHandler<String>) -> Element {
    rsx! {
        div { class: "sidebar",
            h2 { "Events ({events.len()})" }
            div { class: "list",
                for ev in events.iter() {
                    EventRow { event: ev.clone(), on_pick }
                }
            }
        }
    }
}

#[component]
fn EventRow(event: Event, on_pick: EventHandler<String>) -> Element {
    let event_id = event.id.to_string();
    rsx! {
        button {
            key: "{event.id}",
            class: "list-item",
            onclick: move |_| on_pick.call(event_id.clone()),
            div { class: "t", "{event.title}" }
            div { class: "m",
                span { class: "badge {event.category}", "{event.category}" }
                " · {fmt_wib(&event.starts_at)} · {event.venue_name}"
            }
        }
    }
}

#[component]
fn DetailCard(event: Event, onclose: EventHandler<()>) -> Element {
    let id = event.id.to_string();
    let share_base = option_env!("PUBLIC_WEB_URL").filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(api_base);
    let share_url = format!("{share_base}/#event-{id}");
    rsx! {
        div { class: "card",
            div { style: "display:flex; justify-content:space-between; align-items:flex-start;",
                span { class: "badge {event.category}", "{event.category}" }
                button { class: "close", title: "Close", onclick: move |_| onclose.call(()), "×" }
            }
            if let Some(url) = &event.image_url {
                img { src: "{url}", alt: "{event.title}" }
            }
            h1 { "{event.title}" }
            div { class: "meta", "🕒 {fmt_wib(&event.starts_at)}" }
            if let Some(end) = event.ends_at {
                div { class: "meta", "Until {fmt_wib(&end)}" }
            }
            div { class: "meta", "📍 {event.venue_name} — {event.address}" }
            div { class: "meta", "🎟 {event.price_text}" }
            div { class: "meta", "👤 {event.organizer} · {event.contact}" }
            div { class: "desc", "{event.description}" }
            div { class: "tags",
                for tag in event.tags.iter() {
                    span { class: "tag", "#{tag}" }
                }
            }
            div { class: "actions",
                a {
                    class: "btn primary",
                    href: "https://www.google.com/maps/dir/?api=1&destination={event.lat},{event.lng}",
                    target: "_blank",
                    "Directions"
                }
                if let Some(url) = &event.ticket_url {
                    a { class: "btn", href: "{url}", target: "_blank", "Tickets" }
                }
                button {
                    class: "btn",
                    onclick: move |_| {
                        let url = share_url.clone();
                        spawn(async move {
                            eval_script(format!("navigator.clipboard.writeText('{url}');")).await;
                        });
                    },
                    "Share"
                }
            }
        }
    }
}

#[component]
fn SubmitPanel(
    picked: Option<(f64, f64)>,
    onclose: EventHandler<()>,
    submitted: EventHandler<String>,
) -> Element {
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut category = use_signal(|| "community".to_string());
    let mut starts = use_signal(String::new);
    let mut ends = use_signal(String::new);
    let mut venue = use_signal(String::new);
    let mut address = use_signal(String::new);
    let mut price = use_signal(|| "Free".to_string());
    let mut is_free = use_signal(|| true);
    let mut organizer = use_signal(String::new);
    let mut contact = use_signal(String::new);
    let mut ticket = use_signal(String::new);
    let mut tags = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut sending = use_signal(|| false);

    let (lat, lng) = picked.unwrap_or((CITY_CENTER[1], CITY_CENTER[0]));

    let submit = move |_| {
        let starts_at = match parse_local_datetime(&starts()) {
            Some(dt) => dt,
            None => {
                error.set(Some("Please enter a valid start date/time.".into()));
                return;
            }
        };
        let ends_at = if ends().trim().is_empty() {
            None
        } else {
            parse_local_datetime(&ends())
        };
        let payload = NewEvent {
            title: title(),
            description: description(),
            category: category(),
            starts_at,
            ends_at,
            venue_name: venue(),
            address: address(),
            lat,
            lng,
            price_text: price(),
            is_free: is_free(),
            organizer: organizer(),
            contact: contact(),
            ticket_url: if ticket().trim().is_empty() { None } else { Some(ticket()) },
            tags: tags()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .take(10)
                .collect(),
        };
        spawn(async move {
            sending.set(true);
            let result = reqwest::Client::new()
                .post(format!("{}/api/events", api_base()))
                .json(&payload)
                .send()
                .await;
            sending.set(false);
            match result {
                Ok(resp) if resp.status().is_success() => {
                    submitted.call("Submitted! It will appear on the map after approval.".into());
                }
                Ok(resp) => error.set(Some(format!("Server rejected the event ({})", resp.status()))),
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    rsx! {
        div { class: "panel", onclick: move |_| onclose.call(()),
            div { class: "panel-inner", onclick: move |e| e.stop_propagation(),
                div { style: "display:flex; justify-content:space-between; align-items:center;",
                    h2 { "Submit an event" }
                    button { class: "close", onclick: move |_| onclose.call(()), "×" }
                }
                p { class: "hint", "Tap the map to choose the location (lat {lat:.5}, lng {lng:.5})." }
                div { class: "form-grid",
                    div { class: "field full",
                        label { "Title" }
                        input { r#type: "text", value: "{title}", oninput: move |e| title.set(e.value()) }
                    }
                    div { class: "field full",
                        label { "Description" }
                        textarea { value: "{description}", oninput: move |e| description.set(e.value()) }
                    }
                    div { class: "field",
                        label { "Category" }
                        select { value: "{category}", onchange: move |e| category.set(e.value()),
                            option { value: "music", "music" }
                            option { value: "food", "food" }
                            option { value: "arts", "arts" }
                            option { value: "community", "community" }
                        }
                    }
                    div { class: "field",
                        label { "Price" }
                        input { r#type: "text", value: "{price}", oninput: move |e| price.set(e.value()) }
                    }
                    div { class: "field",
                        label { "Starts (WIB)" }
                        input { r#type: "datetime-local", value: "{starts}", oninput: move |e| starts.set(e.value()) }
                    }
                    div { class: "field",
                        label { "Ends (WIB)" }
                        input { r#type: "datetime-local", value: "{ends}", oninput: move |e| ends.set(e.value()) }
                    }
                    div { class: "field full",
                        label { "Venue" }
                        input { r#type: "text", value: "{venue}", oninput: move |e| venue.set(e.value()) }
                    }
                    div { class: "field full",
                        label { "Address" }
                        input { r#type: "text", value: "{address}", oninput: move |e| address.set(e.value()) }
                    }
                    div { class: "field",
                        label { "Organizer" }
                        input { r#type: "text", value: "{organizer}", oninput: move |e| organizer.set(e.value()) }
                    }
                    div { class: "field",
                        label { "Contact" }
                        input { r#type: "text", value: "{contact}", oninput: move |e| contact.set(e.value()) }
                    }
                    div { class: "field full",
                        label { "Ticket URL (https://)" }
                        input { r#type: "url", value: "{ticket}", oninput: move |e| ticket.set(e.value()) }
                    }
                    div { class: "field full",
                        label { "Tags (comma separated)" }
                        input { r#type: "text", value: "{tags}", oninput: move |e| tags.set(e.value()) }
                    }
                    div { class: "field",
                        label { "Free entry" }
                        input { r#type: "checkbox", checked: is_free(), onchange: move |e| is_free.set(e.checked()) }
                    }
                }
                if let Some(err) = error() {
                    div { class: "error", "{err}" }
                }
                div { class: "actions",
                    button { class: "btn primary", disabled: sending(), onclick: submit,
                        if sending() { "Sending…" } else { "Submit for approval" }
                    }
                    button { class: "btn", onclick: move |_| onclose.call(()), "Cancel" }
                }
            }
        }
    }
}

#[component]
fn AdminPanel(onclose: EventHandler<()>) -> Element {
    let mut token = use_signal(String::new);
    let mut list = use_signal(Vec::<Event>::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let load = move |_| {
        let t = token();
        spawn(async move {
            busy.set(true);
            let resp = reqwest::Client::new()
                .get(format!("{}/api/admin/events", api_base()))
                .query(&[("status", "pending")])
                .bearer_auth(&t)
                .send()
                .await;
            busy.set(false);
            match resp {
                Ok(r) if r.status().is_success() => {
                    match r.json::<Vec<Event>>().await {
                        Ok(items) => { list.set(items); error.set(None); }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                }
                Ok(r) => error.set(Some(format!("Unauthorized or server error ({})", r.status()))),
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    let decide = move |(id, action): (String, String)| {
        let t = token();
        spawn(async move {
            let url = format!("{}/api/admin/events/{id}/{action}", api_base());
            let resp = reqwest::Client::new().post(url).bearer_auth(&t).send().await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    list.set(list().into_iter().filter(|e| e.id.to_string() != id).collect());
                }
                Ok(r) => error.set(Some(format!("Action failed ({})", r.status()))),
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    rsx! {
        div { class: "panel", onclick: move |_| onclose.call(()),
            div { class: "panel-inner", onclick: move |e| e.stop_propagation(),
                div { style: "display:flex; justify-content:space-between; align-items:center;",
                    h2 { "Admin — pending events" }
                    button { class: "close", onclick: move |_| onclose.call(()), "×" }
                }
                p { class: "hint", "Token is kept in memory only and never stored." }
                div { class: "form-grid",
                    div { class: "field full",
                        label { "Admin token" }
                        input { r#type: "password", value: "{token}", oninput: move |e| token.set(e.value()) }
                    }
                }
                div { class: "actions",
                    button { class: "btn primary", onclick: load, disabled: busy(), "Load pending" }
                }
                if let Some(err) = error() {
                    div { class: "error", "{err}" }
                }
                for ev in list().iter() {
                    AdminRow { event: ev.clone(), decide }
                }
            }
        }
    }
}

#[component]
fn AdminRow(event: Event, decide: EventHandler<(String, String)>) -> Element {
    let approve_id = event.id.to_string();
    let reject_id = event.id.to_string();
    rsx! {
        div { class: "admin-row",
            div { class: "t", "{event.title}" }
            div { class: "m", "{fmt_wib(&event.starts_at)} · {event.venue_name}" }
            div { class: "actions",
                button {
                    class: "btn primary",
                    onclick: move |_| decide.call((approve_id.clone(), "approve".to_string())),
                    "Approve"
                }
                button {
                    class: "btn",
                    onclick: move |_| decide.call((reject_id.clone(), "reject".to_string())),
                    "Reject"
                }
            }
        }
    }
}
