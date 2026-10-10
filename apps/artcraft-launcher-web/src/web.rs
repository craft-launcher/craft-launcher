//! The browser backend and entry point.

use std::cell::RefCell;
use std::rc::Rc;

use artcraft_launcher_core::protocol::{Backend, Capabilities, Command, Event, LatestRelease, Source};
use artcraft_launcher_core::release::{self, Release};
use artcraft_launcher_core::{Arch, Os, Platform, SUITE, catalog, version};
use artcraft_launcher_ui_egui::{Host, LauncherUi, Settings};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

const SETTINGS_KEY: &str = "settings";

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Warn).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
        let Some(canvas) = document.get_element_by_id("launcher_canvas").and_then(|c| c.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("no canvas");
            return;
        };
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    let ctx = cc.egui_ctx.clone();
                    let settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)).unwrap_or_default();
                    let backend = WebBackend::new(ctx.clone());
                    Ok(Box::new(WebApp { ui: LauncherUi::new(&ctx, Box::new(backend), settings) }))
                }),
            )
            .await;
        if let Some(loading) = document.get_element_by_id("launcher_loading") {
            loading.remove();
        }
        if let Err(e) = result {
            log::error!("couldn't start: {e:?}");
        }
    });
}

struct NoHost;
impl Host for NoHost {}

struct WebApp {
    ui: LauncherUi,
}

impl eframe::App for WebApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui.logic(ctx, &mut NoHost);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui.ui(ui, &mut NoHost);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.ui.settings);
    }
}

struct WebBackend {
    events: Rc<RefCell<Vec<Event>>>,
    ctx: egui::Context,
    platform: Option<Platform>,
    checking: Rc<RefCell<bool>>,
    prereleases: bool,
}

impl WebBackend {
    fn new(ctx: egui::Context) -> Self {
        Self { events: Rc::default(), ctx, platform: visitor_platform(), checking: Rc::default(), prereleases: false }
    }

    fn check(&self) {
        if self.checking.replace(true) {
            return;
        }
        let events = self.events.clone();
        let ctx = self.ctx.clone();
        let checking = self.checking.clone();
        let pre = self.prereleases;
        wasm_bindgen_futures::spawn_local(async move {
            events.borrow_mut().push(Event::CheckStarted);
            let mut limited = None;
            for app in &SUITE {
                let latest = latest(app.repo, pre).await;
                if let Err(Some(reset)) = &latest {
                    limited = Some(*reset);
                }
                let latest = latest.unwrap_or_else(|e| {
                    LatestRelease::Failed(if e.is_some() { "GitHub's hourly limit was reached".into() } else { "couldn't reach GitHub".into() })
                });
                events.borrow_mut().push(Event::Latest { app: app.id.to_string(), latest });
                ctx.request_repaint();
            }
            if let Ok(LatestRelease::Found(r)) = latest(catalog::LAUNCHER_REPO, false).await
                && let (Some(l), Some(c)) = (r.version.as_ref(), version::parse(env!("CARGO_PKG_VERSION")))
                && version::is_newer(l, &c)
            {
                events.borrow_mut().push(Event::LauncherUpdate(r));
            }
            let at = (js_sys::Date::now() / 1000.0) as i64;
            events.borrow_mut().push(Event::CheckFinished { at, source: Source::GitHubApi, rate_limited_until: limited });
            *checking.borrow_mut() = false;
            ctx.request_repaint();
        });
    }
}

/// The latest release; `Err(Some(reset))` when rate-limited, `Err(None)` on other failures.
async fn latest(repo: &str, prereleases: bool) -> Result<LatestRelease, Option<i64>> {
    let url = if prereleases {
        format!("https://api.github.com/repos/{repo}/releases?per_page=30")
    } else {
        format!("https://api.github.com/repos/{repo}/releases/latest")
    };
    let window = web_sys::window().ok_or(None)?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::Cors);
    let request = web_sys::Request::new_with_str_and_init(&url, &init).map_err(|_| None)?;
    let _ = request.headers().set("Accept", "application/vnd.github+json");
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_request(&request)).await.map_err(|_| None)?.dyn_into().map_err(|_| None)?;
    match resp.status() {
        200 => {}
        404 => return Ok(LatestRelease::None),
        403 | 429 => {
            let reset = resp.headers().get("x-ratelimit-reset").ok().flatten().and_then(|r| r.parse().ok());
            return Err(Some(reset.unwrap_or(0)));
        }
        _ => return Err(None),
    }
    let text = JsFuture::from(resp.text().map_err(|_| None)?).await.map_err(|_| None)?.as_string().ok_or(None)?;
    let found: Option<Release> = if prereleases {
        release::parse_github_release_list(&text, repo, true).map_err(|_| None)?
    } else {
        Some(release::parse_github_release(&text, repo).map_err(|_| None)?)
    };
    Ok(found.map_or(LatestRelease::None, LatestRelease::Found))
}

/// The visitor's OS and CPU, as far as the browser says.
fn visitor_platform() -> Option<Platform> {
    let nav = web_sys::window()?.navigator();
    let ua = nav.user_agent().ok()?.to_lowercase();
    let os = if ua.contains("windows") {
        Os::Windows
    } else if ua.contains("mac os") || ua.contains("macintosh") {
        Os::MacOs
    } else if ua.contains("freebsd") {
        Os::FreeBsd
    } else if ua.contains("linux") && !ua.contains("android") {
        Os::Linux
    } else {
        return None;
    };
    let arch = if ua.contains("aarch64") || ua.contains("arm64") || ua.contains("armv8") { Arch::Arm64 } else { Arch::X64 };
    Some(Platform { os, arch })
}

impl Backend for WebBackend {
    fn send(&mut self, command: Command) {
        match command {
            Command::CheckForUpdates { .. } => self.check(),
            Command::Configure(o) => {
                let changed = o.prereleases != self.prereleases;
                self.prereleases = o.prereleases;
                if changed {
                    self.check();
                }
            }
            _ => {}
        }
    }

    fn poll(&mut self) -> Vec<Event> {
        std::mem::take(&mut *self.events.borrow_mut())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities { manage_apps: false, reveal_label: "", self_update: false }
    }

    fn platform(&self) -> Option<Platform> {
        self.platform
    }

    fn now(&self) -> i64 {
        (js_sys::Date::now() / 1000.0) as i64
    }
}
