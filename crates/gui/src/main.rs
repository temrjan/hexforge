//! `hexforge` — minimalist egui GUI for the vanity Ethereum address forge.
//!
//! The search runs on a background thread; progress and matches stream back over
//! a channel so the UI never blocks. Secret material (mnemonic, private key) is
//! hidden until the user reveals it, and nothing is written to disk.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, TryRecvError};
use eframe::egui;
use hexforge_core::{
    search, FoundWallet, MatchMode, Progress, SearchConfig, SearchError, Target,
    DEFAULT_DERIVATION_PATH,
};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([560.0, 440.0]),
        ..Default::default()
    };
    eframe::run_native(
        "hexforge",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(HexforgeApp::default()))
        }),
    )
}

/// Messages streamed from the worker thread to the UI.
enum Msg {
    Progress(Progress),
    Found(FoundWallet),
    Done(Result<(), SearchError>),
}

/// Handle to the background search worker.
struct Worker {
    stop: Arc<AtomicBool>,
    rx: Receiver<Msg>,
    handle: Option<JoinHandle<()>>,
}

#[derive(PartialEq, Eq)]
enum RunState {
    Idle,
    Running,
    Done,
}

struct HexforgeApp {
    word: String,
    mode: MatchMode,
    state: RunState,
    found: Vec<FoundWallet>,
    progress: Option<Progress>,
    error: Option<String>,
    reveal: HashSet<usize>,
    worker: Option<Worker>,
}

impl Default for HexforgeApp {
    fn default() -> Self {
        Self {
            word: String::new(),
            mode: MatchMode::Suffix,
            state: RunState::Idle,
            found: Vec::new(),
            progress: None,
            error: None,
            reveal: HashSet::new(),
            worker: None,
        }
    }
}

impl HexforgeApp {
    fn start(&mut self) {
        let target = match Target::new(&self.word, self.mode) {
            Ok(target) => target,
            Err(err) => {
                self.error = Some(err.to_string());
                return;
            }
        };
        let config = SearchConfig {
            targets: vec![target],
            threads: 0,
            derivation_path: DEFAULT_DERIVATION_PATH.to_string(),
        };

        // Keep previously found wallets — new matches accumulate below them.
        self.progress = None;
        self.error = None;

        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = crossbeam_channel::unbounded::<Msg>();
        let worker_stop = Arc::clone(&stop);
        let tx_progress = tx.clone();
        let tx_found = tx.clone();

        let handle = std::thread::spawn(move || {
            let result = search(
                &config,
                &worker_stop,
                move |progress| {
                    let _ = tx_progress.send(Msg::Progress(progress));
                },
                move |wallet| {
                    let _ = tx_found.send(Msg::Found(wallet.clone()));
                },
            );
            let _ = tx.send(Msg::Done(result.map(|_| ())));
        });

        self.worker = Some(Worker {
            stop,
            rx,
            handle: Some(handle),
        });
        self.state = RunState::Running;
    }

    fn stop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Relaxed);
        }
        // State stays Running until the worker's Done message arrives.
    }

    /// Drains worker messages into UI state without holding the worker borrow.
    fn drain_messages(&mut self) {
        let mut messages = Vec::new();
        let mut disconnected = false;
        if let Some(worker) = &self.worker {
            loop {
                match worker.rx.try_recv() {
                    Ok(msg) => messages.push(msg),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        for msg in messages {
            match msg {
                Msg::Progress(progress) => self.progress = Some(progress),
                Msg::Found(wallet) => self.found.push(wallet),
                Msg::Done(result) => {
                    if let Err(err) = result {
                        self.error = Some(err.to_string());
                    }
                    self.state = RunState::Done;
                    self.join_worker();
                }
            }
        }
        // Worker thread vanished without a Done (e.g. it panicked): leave Running
        // instead of spinning forever on an empty, disconnected channel.
        if disconnected && self.state == RunState::Running {
            if self.error.is_none() {
                self.error = Some("поиск неожиданно прервался".to_string());
            }
            self.state = RunState::Done;
            self.join_worker();
        }
    }

    fn join_worker(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            if let Some(handle) = worker.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn ui_controls(&mut self, ui: &mut egui::Ui) {
        let validation = validation_message(&self.word);
        // Empty input shows no error but must not enable the search.
        let can_start = !self.word.trim().is_empty() && validation.is_none();
        let running = self.state == RunState::Running;

        ui.horizontal(|ui| {
            ui.label("Слово:");
            ui.add_enabled(
                !running,
                egui::TextEdit::singleline(&mut self.word).hint_text("deadbeef"),
            );
        });
        if let Some(message) = &validation {
            ui.colored_label(egui::Color32::from_rgb(220, 90, 90), message);
        }

        ui.horizontal(|ui| {
            ui.label("Где:");
            ui.radio_value(&mut self.mode, MatchMode::Prefix, "в начале");
            ui.radio_value(&mut self.mode, MatchMode::Anywhere, "везде");
            ui.radio_value(&mut self.mode, MatchMode::Suffix, "в конце");
        });

        let has_results = !self.found.is_empty();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(can_start && !running, egui::Button::new("Искать"))
                .clicked()
            {
                self.start();
            }
            if ui.add_enabled(running, egui::Button::new("Стоп")).clicked() {
                self.stop();
            }
            if ui
                .add_enabled(has_results && !running, egui::Button::new("Очистить"))
                .clicked()
            {
                self.found.clear();
                self.reveal.clear();
            }
        });
    }

    fn ui_status(&self, ui: &mut egui::Ui) {
        match self.state {
            RunState::Idle => {
                ui.label("Введите hex-слово и нажмите «Искать».");
            }
            RunState::Running => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    match &self.progress {
                        Some(progress) => ui.label(format_progress(progress)),
                        None => ui.label("Идёт поиск… (разогрев)"),
                    }
                });
            }
            RunState::Done => {
                if let Some(error) = &self.error {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 90, 90),
                        format!("Ошибка: {error}"),
                    );
                } else {
                    ui.label(format!("Готово · найдено {}", self.found.len()));
                }
            }
        }
    }

    fn ui_results(&mut self, ui: &mut egui::Ui) {
        if self.found.is_empty() {
            if self.state == RunState::Running {
                ui.label("Пока ничего не найдено — ищем…");
            }
            return;
        }
        ui.label(format!("Найдено: {}", self.found.len()));
        let mut toggle: Option<usize> = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (index, wallet) in self.found.iter().enumerate() {
                    let revealed = self.reveal.contains(&index);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.monospace(wallet.address.as_str());
                            if ui.button("Копировать адрес").clicked() {
                                ui.ctx().copy_text(wallet.address.clone());
                            }
                            let label = if revealed {
                                "Скрыть"
                            } else {
                                "Показать секрет"
                            };
                            if ui.button(label).clicked() {
                                toggle = Some(index);
                            }
                        });
                        if revealed {
                            // NOTE: rendering a secret hands its text to egui's galley
                            // cache (font layout), which is not zeroized. This is
                            // inherent to displaying secrets in any GUI; reveal is an
                            // explicit, transient user action.
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(wallet.mnemonic.as_str()).monospace(),
                                )
                                .selectable(true),
                            );
                            let private_key = wallet.private_key_hex();
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(private_key.as_str()).monospace(),
                                )
                                .selectable(true),
                            );
                        }
                    });
                }
            });
        if let Some(index) = toggle {
            if !self.reveal.remove(&index) {
                self.reveal.insert(index);
            }
        }
    }
}

impl eframe::App for HexforgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_messages();

        // Fixed controls on top, fixed note at the bottom, scrolling results in
        // the middle — the list scrolls no matter how many wallets accumulate.
        egui::Panel::top("controls").show_inside(ui, |ui| {
            ui.heading("hexforge");
            ui.add_space(4.0);
            self.ui_controls(ui);
            ui.separator();
            self.ui_status(ui);
        });
        egui::Panel::bottom("footer").show_inside(ui, |ui| {
            ui.separator();
            ui.small("🔒 Offline · ключи не сохраняются на диск · выпиши seed на бумагу");
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.ui_results(ui);
        });

        if self.state == RunState::Running {
            ui.ctx().request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Relaxed);
        }
        self.join_worker();
    }
}

/// Generated keys per second, guarding against a zero elapsed time.
fn rate_per_sec(attempts: u64, elapsed: Duration) -> u64 {
    // checked_div returns None when elapsed is < 1s; fall back to the raw count.
    attempts.checked_div(elapsed.as_secs()).unwrap_or(attempts)
}

/// One-line progress summary for the status row.
fn format_progress(progress: &Progress) -> String {
    format!(
        "{} ключей · {}/с · найдено {}/{} · {}с",
        progress.attempts,
        rate_per_sec(progress.attempts, progress.elapsed),
        progress.found,
        progress.total,
        progress.elapsed.as_secs(),
    )
}

/// Inline validation message for the current word, or `None` if it is valid.
fn validation_message(word: &str) -> Option<String> {
    if word.trim().is_empty() {
        return None; // empty input shows no error (Start is just disabled)
    }
    match hexforge_core::validate_target(word) {
        Ok(_) => None,
        Err(err) => Some(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{rate_per_sec, validation_message};
    use std::time::Duration;

    #[test]
    fn rate_with_zero_elapsed_returns_attempts() {
        assert_eq!(rate_per_sec(100, Duration::ZERO), 100);
    }

    #[test]
    fn rate_divides_by_seconds() {
        assert_eq!(rate_per_sec(1000, Duration::from_secs(4)), 250);
    }

    #[test]
    fn validation_is_none_for_valid_and_for_empty() {
        assert!(validation_message("deadbeef").is_none());
        assert!(validation_message("   ").is_none());
    }

    #[test]
    fn validation_reports_non_hex() {
        assert!(validation_message("zzz").is_some());
    }
}
