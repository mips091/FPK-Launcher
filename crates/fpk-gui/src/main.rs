use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const DEFAULT_VERSION: &str = "26.3";

enum TaskMessage {
    Status(String),
    Finished(Result<(), String>),
}

struct FpkApp {
    version: String,
    username: String,
    status: String,
    busy: bool,
    receiver: Option<Receiver<TaskMessage>>,
}

impl Default for FpkApp {
    fn default() -> Self {
        Self {
            version: DEFAULT_VERSION.to_owned(),
            username: "Player".to_owned(),
            status: "Ready. Install Minecraft before pressing Play.".to_owned(),
            busy: false,
            receiver: None,
        }
    }
}

impl FpkApp {
    fn start_task(&mut self, launch: bool) {
        if self.busy { return; }
        let version = self.version.trim().to_owned();
        let username = self.username.trim().to_owned();
        if version.is_empty() {
            self.status = "Version cannot be empty.".to_owned();
            return;
        }
        if launch && username.is_empty() {
            self.status = "Username cannot be empty.".to_owned();
            return;
        }

        let (tx, rx): (Sender<TaskMessage>, Receiver<TaskMessage>) = mpsc::channel();
        self.receiver = Some(rx);
        self.busy = true;
        self.status = if launch { "Starting Minecraft...".into() } else { "Downloading Minecraft files...".into() };

        thread::spawn(move || {
            let result = if launch {
                let _ = tx.send(TaskMessage::Status("Launching with a temporary offline profile...".into()));
                fpk_core::launch_game(&version, &username).map_err(|e| format!("{e:#}"))
            } else {
                let _ = tx.send(TaskMessage::Status("Downloading client JAR...".into()));
                let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string());
                match runtime {
                    Ok(runtime) => runtime.block_on(async {
                        fpk_core::install_client(&version).await?;
                        let _ = tx.send(TaskMessage::Status("Installing assets, libraries, and Linux natives...".into()));
                        fpk_core::install_runtime_files(&version).await
                    }).map_err(|e| format!("{e:#}")),
                    Err(e) => Err(e),
                }
            };
            let _ = tx.send(TaskMessage::Finished(result));
        });
    }

    fn poll_task(&mut self) {
        let mut finished = false;
        if let Some(receiver) = &self.receiver {
            while let Ok(message) = receiver.try_recv() {
                match message {
                    TaskMessage::Status(status) => self.status = status,
                    TaskMessage::Finished(Ok(())) => {
                        self.status = "Task completed.".into();
                        finished = true;
                    }
                    TaskMessage::Finished(Err(error)) => {
                        self.status = format!("Error: {error}");
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.busy = false;
            self.receiver = None;
        }
    }
}

impl eframe::App for FpkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_task();
        ctx.request_repaint_after(std::time::Duration::from_millis(150));

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("FPK Launcher");
                ui.label(egui::RichText::new("MINECRAFT JAVA").small().color(egui::Color32::from_rgb(120, 170, 255)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Rust edition");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("YOUR NEXT WORLD STARTS HERE").small().color(egui::Color32::from_rgb(145, 160, 185)));
                ui.add_space(8.0);
                ui.heading(egui::RichText::new("Minecraft Java").size(32.0));
                ui.label("A lightweight launcher powered by Rust");
            });

            ui.add_space(28.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.heading("Game configuration");
                ui.add_space(12.0);
                ui.label("Minecraft version");
                ui.text_edit_singleline(&mut self.version);
                ui.add_space(8.0);
                ui.label("Offline username (temporary)");
                ui.text_edit_singleline(&mut self.username);
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let install = ui.add_enabled(!self.busy, egui::Button::new("Install / Repair").min_size(egui::vec2(150.0, 42.0)));
                    if install.clicked() { self.start_task(false); }
                    let play = ui.add_enabled(!self.busy, egui::Button::new(egui::RichText::new("▶  PLAY").strong()).min_size(egui::vec2(130.0, 42.0)));
                    if play.clicked() { self.start_task(true); }
                });
            });

            ui.add_space(18.0);
            ui.heading("Launcher status");
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.busy { ui.spinner(); } else { ui.label("●"); }
                    ui.label(&self.status);
                });
            });

            ui.add_space(16.0);
            ui.label(egui::RichText::new("Note: Microsoft login is not implemented yet. Play currently uses an offline profile and cannot authenticate online multiplayer. Install a compatible Java runtime first.").small().color(egui::Color32::GRAY));
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 560.0])
            .with_min_inner_size([620.0, 460.0)
            .with_title("FPK Launcher"),
        ..Default::default()
    };
    eframe::run_native("FPK Launcher", options, Box::new(|_cc| Ok(Box::new(FpkApp::default()))))
}
