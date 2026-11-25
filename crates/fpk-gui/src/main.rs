use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const DEFAULT_VERSION: &str = "26.3";
const BG: egui::Color32 = egui::Color32::from_rgb(18, 20, 25);
const PANEL: egui::Color32 = egui::Color32::from_rgb(27, 30, 37);
const PANEL_2: egui::Color32 = egui::Color32::from_rgb(36, 40, 48);
const GREEN: egui::Color32 = egui::Color32::from_rgb(231, 171, 83);
const MUTED: egui::Color32 = egui::Color32::from_rgb(158, 164, 176);

enum TaskMessage { Status(String), Finished(Result<(), String>) }

struct FpkApp {
    version: String,
    username: String,
    status: String,
    busy: bool,
    receiver: Option<Receiver<TaskMessage>>,
    selected_page: usize,
    search: String,
    instance_name: String,
}

impl Default for FpkApp {
    fn default() -> Self {
        Self {
            version: DEFAULT_VERSION.into(),
            username: "Player".into(),
            status: "Ready to play. Install the game files first.".into(),
            busy: false,
            receiver: None,
            selected_page: 0,
            search: String::new(),
            instance_name: "Survival".into(),
        }
    }
}

impl FpkApp {
    fn start_task(&mut self, launch: bool) {
        if self.busy { return; }
        let version = self.version.trim().to_owned();
        let username = self.username.trim().to_owned();
        if version.is_empty() { self.status = "Version cannot be empty.".into(); return; }
        if launch && username.is_empty() { self.status = "Username cannot be empty.".into(); return; }

        let (tx, rx): (Sender<TaskMessage>, Receiver<TaskMessage>) = mpsc::channel();
        self.receiver = Some(rx);
        self.busy = true;
        self.status = if launch { "Starting Minecraft...".into() } else { "Preparing downloads...".into() };

        thread::spawn(move || {
            let result = if launch {
                let _ = tx.send(TaskMessage::Status("Launching with temporary offline profile...".into()));
                fpk_core::launch_game(&version, &username).map_err(|e| format!("{e:#}"))
            } else {
                let _ = tx.send(TaskMessage::Status("Downloading Minecraft client...".into()));
                match tokio::runtime::Runtime::new() {
                    Ok(runtime) => runtime.block_on(async {
                        fpk_core::install_client(&version).await?;
                        let _ = tx.send(TaskMessage::Status("Downloading assets and libraries (up to 16 at once)...".into()));
                        fpk_core::install_runtime_files(&version).await
                    }).map_err(|e| format!("{e:#}")),
                    Err(e) => Err(e.to_string()),
                }
            };
            let _ = tx.send(TaskMessage::Finished(result));
        });
    }

    fn poll_task(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(message) = rx.try_recv() {
                match message {
                    TaskMessage::Status(s) => self.status = s,
                    TaskMessage::Finished(Ok(())) => { self.status = "Task completed successfully.".into(); finished = true; }
                    TaskMessage::Finished(Err(e)) => { self.status = format!("Error: {e}"); finished = true; }
                }
            }
        }
        if finished { self.busy = false; self.receiver = None; }
    }

    fn accent_button(ui: &mut egui::Ui, label: &str, size: egui::Vec2) -> egui::Response {
        ui.add_sized(size, egui::Button::new(egui::RichText::new(label).strong().color(egui::Color32::from_rgb(5, 25, 19)))
            .fill(GREEN).stroke(egui::Stroke::new(1.0, GREEN)))
    }

    fn draw_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar").exact_width(184.0).resizable(false).show(ctx, |ui| {
            ui.add_space(22.0);
            ui.horizontal(|ui| {
                ui.add_space(17.0);
                ui.label(egui::RichText::new("F").size(27.0).strong().color(GREEN));
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("FPK").size(20.0).strong().color(egui::Color32::WHITE));
                    ui.label(egui::RichText::new("MINECRAFT").size(9.0).strong().color(MUTED));
                });
            });
            ui.add_space(30.0);
            for (i, (icon, label)) in [("", "Library"), ("", "Discover"), ("", "Instances"), ("", "Downloads"), ("", "Settings")].iter().enumerate() {
                let page = if i == 4 { 4 } else { i };
                let selected = self.selected_page == page;
                let fill = if selected { egui::Color32::from_rgb(54, 43, 30) } else { BG };
                let color = if selected { GREEN } else { MUTED };
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let response = ui.add_sized([158.0, 43.0], egui::Button::new(
                        egui::RichText::new(format!("{}", label)).size(14.0).color(color)
                    ).fill(fill).stroke(egui::Stroke::NONE));
                    if response.clicked() { self.selected_page = page; }
                });
                ui.add_space(5.0);
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    ui.label(egui::RichText::new("FPK  |  0.1.0").size(11.0).color(MUTED));
                });
                ui.add_space(14.0);
            });
        });
    }

    fn draw_header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").exact_height(66.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(match self.selected_page { 0 => "Your library", 1 => "Discover", 2 => "Instances", 3 => "Downloads", _ => "Preferences" }).size(21.0).strong().color(egui::Color32::WHITE));
                    ui.label(egui::RichText::new("Minecraft Java Edition  |  Offline profile").size(11.0).color(MUTED));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(18.0);
                    if Self::accent_button(ui, "Create instance", [154.0, 38.0].into()).clicked() {
                        self.selected_page = 4;
                        self.status = "Set the instance name and version, then use Install / Repair.".into();
                    }
                    ui.add_sized([230.0, 38.0], egui::TextEdit::singleline(&mut self.search).hint_text("Search your instances"));
                    ui.add_space(10.0);
                });
            });
        });
    }

    fn draw_hero(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(available, 202.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 16.0, egui::Color32::from_rgb(48, 43, 37));
        painter.rect_filled(egui::Rect::from_min_max(rect.left_top(), egui::pos2(rect.right(), rect.top() + 120.0)), 16.0, egui::Color32::from_rgb(75, 62, 47));
        let horizon = rect.top() + 135.0;
        painter.circle_filled(egui::pos2(rect.right() - 112.0, rect.top() + 62.0), 23.0, egui::Color32::from_rgb(220, 178, 108));
        let peaks = vec![
            egui::pos2(rect.left() + available * 0.48, horizon),
            egui::pos2(rect.left() + available * 0.65, rect.top() + 38.0),
            egui::pos2(rect.left() + available * 0.82, horizon),
            egui::pos2(rect.left() + available * 0.94, rect.top() + 76.0),
            rect.right_bottom(),
            egui::pos2(rect.left(), rect.bottom()),
        ];
        painter.add(egui::Shape::convex_polygon(peaks, egui::Color32::from_rgb(35, 34, 33), egui::Stroke::NONE));
        painter.rect_filled(rect, 16.0, egui::Color32::from_rgba_unmultiplied(10, 12, 16, 48));
        let content = rect.shrink(24.0);
        ui.allocate_ui_at_rect(content, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("YOUR ACTIVE WORLD  /  JAVA EDITION").size(10.0).strong().color(GREEN));
                ui.add_space(7.0);
                ui.label(egui::RichText::new(&self.instance_name).size(32.0).strong().color(egui::Color32::WHITE));
                ui.label(egui::RichText::new(format!("Vanilla Minecraft  {}", self.version)).size(13.0).color(egui::Color32::from_rgb(225, 222, 216)));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if Self::accent_button(ui, if self.busy { "Launching…" } else { "Play" }, [116.0, 40.0].into()).clicked() { self.start_task(true); }
                    if ui.add_sized([118.0, 40.0], egui::Button::new("Configure").fill(PANEL_2)).clicked() { self.selected_page = 4; }
                });
            });
        });
    }

    fn draw_home(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        self.draw_hero(ui);
        ui.add_space(25.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("INSTALLED INSTANCES").size(12.0).strong().color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("1 profile").size(12.0).color(MUTED));
            });
        });
        ui.add_space(10.0);
        if self.search.is_empty() || self.instance_name.to_lowercase().contains(&self.search.to_lowercase()) {
            egui::Frame::group(ui.style()).fill(PANEL).stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(53, 57, 65))).show(ui, |ui| {
                ui.set_min_height(106.0);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(110.0, 78.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 8.0, egui::Color32::from_rgb(67, 56, 43));
                    ui.painter().circle_filled(egui::pos2(rect.right() - 25.0, rect.top() + 22.0), 12.0, egui::Color32::from_rgb(220, 178, 108));
                    ui.painter().add(egui::Shape::convex_polygon(vec![egui::pos2(rect.left(), rect.bottom()), egui::pos2(rect.left()+45.0, rect.top()+25.0), egui::pos2(rect.right(), rect.bottom())], egui::Color32::from_rgb(35, 34, 33), egui::Stroke::NONE));
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&self.instance_name).size(17.0).strong().color(egui::Color32::WHITE));
                        ui.label(egui::RichText::new(format!("Minecraft {}  |  Vanilla", self.version)).size(12.0).color(MUTED));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Ready to launch").size(11.0).color(GREEN));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if Self::accent_button(ui, "▶  Play", [96.0, 38.0].into()).clicked() { self.start_task(true); }
                    });
                });
            });
        } else {
            ui.label(egui::RichText::new("No instance matches your search.").color(MUTED));
        }
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            if ui.add_sized([170.0, 42.0], egui::Button::new("Add instance").fill(PANEL_2)).clicked() { self.selected_page = 4; }
            if ui.add_sized([170.0, 42.0], egui::Button::new("Install / repair").fill(PANEL_2)).clicked() { self.start_task(false); }
        });
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui) {
        ui.add_space(24.0);
        ui.heading("Game settings");
        ui.label(egui::RichText::new("Configure your Minecraft installation").color(MUTED));
        ui.add_space(20.0);
        egui::Frame::group(ui.style()).fill(PANEL).show(ui, |ui| {
            ui.set_max_width(520.0);
            ui.label("Instance name");
            ui.text_edit_singleline(&mut self.instance_name);
            ui.add_space(12.0);
            ui.label("Minecraft version");
            ui.text_edit_singleline(&mut self.version);
            ui.add_space(12.0);
            ui.label("Offline username (temporary)");
            ui.text_edit_singleline(&mut self.username);
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.busy, egui::Button::new("Install / Repair").min_size([150.0, 40.0].into())).clicked() { self.start_task(false); }
                if Self::accent_button(ui, "▶  Play", [110.0, 40.0].into()).clicked() { self.start_task(true); }
            });
        });
    }
}

impl eframe::App for FpkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_task();
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.window_fill = PANEL;
        style.visuals.panel_fill = BG;
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(14, 16, 20);
        style.visuals.widgets.inactive.bg_fill = PANEL_2;
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(57, 51, 43);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(125, 86, 39);
        ctx.set_style(style);

        self.draw_sidebar(ctx);
        self.draw_header(ctx);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(28, 10))).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.selected_page {
                    0 => self.draw_home(ui),
                    4 => self.draw_settings(ui),
                    _ => {
                        ui.add_space(30.0);
                        ui.heading(match self.selected_page { 1 => "Discover", 2 => "Instances", 3 => "Downloads", _ => "Library" });
                        ui.label(egui::RichText::new("This section is being built. Your installed game is available in Library.").color(MUTED));
                        if ui.button("Back to Library").clicked() { self.selected_page = 0; }
                    }
                }
                ui.add_space(18.0);
                egui::Frame::group(ui.style()).fill(PANEL).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if self.busy { ui.spinner(); } else { ui.label(egui::RichText::new("●").color(GREEN)); }
                        ui.label(egui::RichText::new(&self.status).color(if self.status.starts_with("Error:") { egui::Color32::LIGHT_RED } else { MUTED }));
                    });
                });
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Microsoft account authentication is not implemented yet. Offline mode cannot join authenticated online servers.").size(10.0).color(MUTED));
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1120.0, 720.0]).with_min_inner_size([850.0, 600.0]).with_title("FPK Launcher"),
        ..Default::default()
    };
    eframe::run_native("FPK Launcher", options, Box::new(|_cc| Ok(Box::new(FpkApp::default()))))
}
