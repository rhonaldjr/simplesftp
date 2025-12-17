#[cfg(target_os = "linux")]
use gtk;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

pub struct TrayManager {
    _tray_icon: TrayIcon,
    menu_event_receiver: tray_icon::menu::MenuEventReceiver,
    show_item_id: MenuId,
    exit_item_id: MenuId,
}

impl TrayManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize GTK on Linux (required for tray-icon crate)
        #[cfg(target_os = "linux")]
        {
            if gtk::is_initialized() {
                // Already initialized, skip
            } else if let Err(e) = gtk::init() {
                return Err(format!("Failed to initialize GTK: {}", e).into());
            }
        }

        // Create tray menu
        println!("Creating tray menu...");
        let tray_menu = Menu::new();

        let show_item = MenuItem::new("Show Window", true, None);
        let exit_item = MenuItem::new("Exit", true, None);

        let show_item_id = show_item.id().clone();
        let exit_item_id = exit_item.id().clone();

        tray_menu.append(&show_item)?;
        tray_menu.append(&exit_item)?;

        // Create tray icon
        println!("Building tray icon...");
        let icon = Self::generate_icon()?;
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("SimpleSFTP")
            .with_icon(icon)
            .build()?;
        println!("Tray icon built successfully.");

        let menu_event_receiver = MenuEvent::receiver().clone();

        Ok(Self {
            _tray_icon: tray_icon,
            menu_event_receiver,
            show_item_id,
            exit_item_id,
        })
    }

    pub fn update(&self) {
        #[cfg(target_os = "linux")]
        {
            while gtk::events_pending() {
                gtk::main_iteration();
            }
        }
    }

    fn generate_icon() -> Result<Icon, Box<dyn std::error::Error>> {
        let width = 32;
        let height = 32;
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);

        for _ in 0..width * height {
            // Blue icon
            rgba.push(0); // R
            rgba.push(100); // G
            rgba.push(255); // B
            rgba.push(255); // A
        }

        Icon::from_rgba(rgba, width, height)
            .map_err(|e| format!("Failed to create icon: {}", e).into())
    }

    /// Check for tray menu events and return the action
    pub fn poll_events(&self) -> Option<TrayAction> {
        if let Ok(event) = self.menu_event_receiver.try_recv() {
            if event.id == self.show_item_id {
                return Some(TrayAction::Show);
            } else if event.id == self.exit_item_id {
                return Some(TrayAction::Exit);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    Exit,
}
