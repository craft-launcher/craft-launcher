//! The tray (menu bar) icon: show the window, check for updates, quit.

use std::sync::mpsc::{Receiver, Sender, channel};

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub enum TrayAction {
    Toggle,
    Show,
    Check,
    Open(&'static str),
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    show: MenuId,
    check: MenuId,
    quit: MenuId,
    rx: Receiver<TrayAction>,
    tooltip: String,
}

fn load_icon() -> Result<tray_icon::Icon, String> {
    // The ArtCraft "A" mark (an ArtCraft trademark, not MIT/Apache: docs/brand/LICENSE-brand.txt).
    // macOS tints the black mark to the menu bar's colours as a template image; elsewhere the
    // blue mark is shown as supplied.
    let bytes: &[u8] = if cfg!(target_os = "macos") {
        include_bytes!("../../../docs/brand/artcraft-mark-black-44.png")
    } else {
        include_bytes!("../../../docs/brand/artcraft-mark-64.png")
    };
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = img.dimensions();
    tray_icon::Icon::from_rgba(img.into_raw(), w, h).map_err(|e| e.to_string())
}

impl Tray {
    /// Creates the tray icon. `wake` repaints the UI so it sees the action, even when hidden.
    pub fn new(wake: impl Fn() + Send + Sync + Clone + 'static) -> Result<Self, String> {
        let show = MenuItem::new("Show ArtCraft Launcher", true, None);
        let check = MenuItem::new("Check for Updates", true, None);
        use artcraft_launcher_core::catalog::{DISCORD_URL, GITHUB_URL, WEBSITE_URL};
        let website = MenuItem::new("Visit getartcraft.com", true, None);
        let github = MenuItem::new("ArtCraft on GitHub", true, None);
        let discord = MenuItem::new("Join the ArtCraft Discord", true, None);
        let quit = MenuItem::new("Quit ArtCraft Launcher", true, None);
        let menu = Menu::new();
        menu.append_items(&[&show, &check, &PredefinedMenuItem::separator(), &website, &github, &discord, &PredefinedMenuItem::separator(), &quit])
            .map_err(|e| e.to_string())?;
        let links = [(website.id().clone(), WEBSITE_URL), (github.id().clone(), GITHUB_URL), (discord.id().clone(), DISCORD_URL)];
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(load_icon()?)
            .with_icon_as_template(cfg!(target_os = "macos"))
            .with_menu_on_left_click(cfg!(target_os = "macos"))
            .with_tooltip("ArtCraft Launcher")
            .build()
            .map_err(|e| e.to_string())?;
        let (tx, rx): (Sender<TrayAction>, Receiver<TrayAction>) = channel();
        let (show_id, check_id, quit_id) = (show.id().clone(), check.id().clone(), quit.id().clone());
        {
            let tx = tx.clone();
            let wake = wake.clone();
            let ids = (show_id.clone(), check_id.clone(), quit_id.clone());
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let action = if e.id == ids.0 {
                    TrayAction::Show
                } else if e.id == ids.1 {
                    TrayAction::Check
                } else if e.id == ids.2 {
                    TrayAction::Quit
                } else if let Some((_, url)) = links.iter().find(|(id, _)| *id == e.id) {
                    TrayAction::Open(url)
                } else {
                    return;
                };
                let _ = tx.send(action);
                wake();
            }));
        }
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                let _ = tx.send(TrayAction::Toggle);
                wake();
            }
        }));
        Ok(Self { icon, show: show_id, check: check_id, quit: quit_id, rx, tooltip: String::new() })
    }

    pub fn actions(&self) -> Vec<TrayAction> {
        self.rx.try_iter().collect()
    }

    pub fn set_updates(&mut self, count: usize) {
        let tip = match count {
            0 => "ArtCraft Launcher".to_string(),
            1 => "ArtCraft Launcher · 1 update available".to_string(),
            n => format!("ArtCraft Launcher · {n} updates available"),
        };
        if tip != self.tooltip {
            let _ = self.icon.set_tooltip(Some(&tip));
            self.tooltip = tip;
        }
    }

    #[allow(dead_code)]
    pub fn ids(&self) -> (&MenuId, &MenuId, &MenuId) {
        (&self.show, &self.check, &self.quit)
    }
}
