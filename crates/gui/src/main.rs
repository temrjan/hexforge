//! `hexforge` — minimalist egui GUI for the vanity Ethereum address forge.
//!
//! The search runs on a background thread; progress and matches stream back over
//! a channel so the UI never blocks. Secret material (mnemonic, private key) is
//! hidden until the user reveals it, and nothing is written to disk.

mod theme;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};
use eframe::egui::{self, FontFamily, RichText};
use hexforge_core::{
    search, FoundWallet, MatchMode, Progress, SearchConfig, SearchError, Target,
    DEFAULT_DERIVATION_PATH,
};

use theme::color;

/// How long the "Скопировано ✓" confirmation stays on a copy button.
const COPIED_FLASH: Duration = Duration::from_millis(1200);

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // app_id must match StartupWMClass in the .desktop so the window
            // groups under the launcher icon (Wayland/X11).
            .with_app_id("hexforge")
            .with_inner_size([560.0, 460.0])
            .with_min_inner_size([480.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "hexforge",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
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
    copied: Option<(usize, Instant)>,
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
            copied: None,
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

    fn copied_flash_active(&self) -> bool {
        self.copied
            .is_some_and(|(_, at)| at.elapsed() < COPIED_FLASH)
    }

    fn ui_brand(ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            theme::paint_hexmark(ui, 26.0);
            ui.add_space(2.0);
            ui.heading(RichText::new("hexforge").color(color::TXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("vanity eth")
                        .size(11.0)
                        .family(FontFamily::Monospace)
                        .color(color::TXT_FAINT),
                );
            });
        });
    }

    fn ui_controls(&mut self, ui: &mut egui::Ui) {
        let validation = validation_message(&self.word);
        // Empty input shows no error but must not enable the search.
        let can_start = !self.word.trim().is_empty() && validation.is_none();
        let running = self.state == RunState::Running;

        ui.horizontal(|ui| {
            ui.label(RichText::new("Слово:").color(color::TXT_MUTED));
            ui.add_enabled(
                !running,
                egui::TextEdit::singleline(&mut self.word)
                    .hint_text("deadbeef")
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
        });
        if let Some(message) = &validation {
            ui.label(RichText::new(message).size(11.0).color(color::DANGER));
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new("Где:").color(color::TXT_MUTED));
            ui.radio_value(&mut self.mode, MatchMode::Prefix, "в начале");
            ui.radio_value(&mut self.mode, MatchMode::Anywhere, "везде");
            ui.radio_value(&mut self.mode, MatchMode::Suffix, "в конце");
        });

        ui.horizontal(|ui| {
            let primary = if can_start && !running {
                egui::Button::new(RichText::new("Искать").color(color::ON_ACCENT))
                    .fill(color::ACCENT)
            } else {
                egui::Button::new(RichText::new("Искать").color(color::TXT_FAINT))
            };
            if ui.add_enabled(can_start && !running, primary).clicked() {
                self.start();
            }
            if ui.add_enabled(running, egui::Button::new("Стоп")).clicked() {
                self.stop();
            }
            let has_results = !self.found.is_empty();
            if ui
                .add_enabled(has_results && !running, egui::Button::new("Очистить"))
                .clicked()
            {
                self.found.clear();
                self.reveal.clear();
                self.copied = None;
            }
        });
    }

    fn ui_status(&self, ui: &mut egui::Ui) {
        match self.state {
            RunState::Idle => {
                ui.horizontal(|ui| {
                    status_dot(ui, color::TXT_FAINT);
                    ui.label(
                        RichText::new("Введите hex-слово и нажмите «Искать».")
                            .color(color::TXT_MUTED),
                    );
                });
            }
            RunState::Running => {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().color(color::ACCENT).size(14.0));
                    match &self.progress {
                        Some(progress) => ui.label(
                            RichText::new(format_progress(progress))
                                .family(FontFamily::Monospace)
                                .color(color::TXT),
                        ),
                        None => ui
                            .label(RichText::new("Идёт поиск… (разогрев)").color(color::TXT_MUTED)),
                    };
                });
            }
            RunState::Done => {
                ui.horizontal(|ui| {
                    if let Some(error) = &self.error {
                        status_dot(ui, color::DANGER);
                        ui.label(RichText::new(format!("Ошибка: {error}")).color(color::DANGER));
                    } else {
                        status_dot(ui, color::SUCCESS);
                        ui.label(
                            RichText::new(format!("Готово · найдено {}", self.found.len()))
                                .color(color::TXT_MUTED),
                        );
                    }
                });
            }
        }
    }

    fn ui_results(&mut self, ui: &mut egui::Ui) {
        if self.found.is_empty() {
            if self.state == RunState::Running {
                ui.label(RichText::new("Пока ничего не найдено — ищем…").color(color::TXT_MUTED));
            }
            return;
        }

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Найдено: ")
                    .family(theme::strong_family())
                    .color(color::TXT),
            );
            ui.label(
                RichText::new(self.found.len().to_string())
                    .family(theme::strong_family())
                    .color(color::ACCENT),
            );
        });
        ui.add_space(2.0);

        let mut toggle: Option<usize> = None;
        let mut copied_now: Option<usize> = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (index, wallet) in self.found.iter().enumerate() {
                    let revealed = self.reveal.contains(&index);
                    let is_copied = self
                        .copied
                        .is_some_and(|(ci, at)| ci == index && at.elapsed() < COPIED_FLASH);
                    egui::Frame::default()
                        .fill(color::CARD_BG)
                        .stroke(egui::Stroke::new(1.0, color::BORDER))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(12))
                        .shadow(theme::card_shadow())
                        .show(ui, |ui| {
                            ui.label(theme::address_layout(&wallet.address, &wallet.target_word));
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                let copy_label = if is_copied {
                                    RichText::new("Скопировано ✓")
                                        .size(12.0)
                                        .color(color::SUCCESS)
                                } else {
                                    RichText::new("Копировать адрес").size(12.0)
                                };
                                if ui.button(copy_label).clicked() {
                                    ui.ctx().copy_text(wallet.address.clone());
                                    copied_now = Some(index);
                                }
                                let reveal_label = if revealed {
                                    "Скрыть"
                                } else {
                                    "Показать секрет"
                                };
                                if ui.button(RichText::new(reveal_label).size(12.0)).clicked() {
                                    toggle = Some(index);
                                }
                            });
                            if revealed {
                                // NOTE: rendering a secret hands its text to egui's
                                // galley cache (not zeroized). Inherent to showing a
                                // secret in any GUI; reveal is an explicit action.
                                ui.add_space(2.0);
                                secret_block(ui, wallet);
                            }
                        });
                }
            });

        if let Some(index) = toggle {
            if !self.reveal.remove(&index) {
                self.reveal.insert(index);
            }
        }
        if let Some(index) = copied_now {
            self.copied = Some((index, Instant::now()));
        }
    }
}

impl eframe::App for HexforgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_messages();

        egui::Panel::top("controls").show_inside(ui, |ui| {
            ui.add_space(4.0);
            Self::ui_brand(ui);
            ui.add_space(6.0);
            self.ui_controls(ui);
            ui.add_space(2.0);
            ui.separator();
            self.ui_status(ui);
            ui.add_space(2.0);
        });
        egui::Panel::bottom("footer").show_inside(ui, |ui| {
            ui.separator();
            ui.add_space(2.0);
            ui.label(
                RichText::new("🔒 Offline · ключи не сохраняются на диск · выпиши seed на бумагу")
                    .size(11.0)
                    .color(color::TXT_FAINT),
            );
            ui.add_space(2.0);
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.ui_results(ui);
        });

        if self.state == RunState::Running || self.copied_flash_active() {
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

/// Paints a small filled status dot.
fn status_dot(ui: &mut egui::Ui, fill: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, fill);
}

/// Renders the revealed secret block (mnemonic + private key) in an inset frame.
fn secret_block(ui: &mut egui::Ui, wallet: &FoundWallet) {
    egui::Frame::default()
        .fill(color::INSET_BG)
        .stroke(egui::Stroke::new(1.0, color::BORDER_SOFT))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                RichText::new("SEED-ФРАЗА")
                    .size(11.0)
                    .color(color::TXT_MUTED),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(wallet.mnemonic.as_str())
                        .family(FontFamily::Monospace)
                        .color(color::MONO_TXT),
                )
                .selectable(true),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("ПРИВАТНЫЙ КЛЮЧ")
                    .size(11.0)
                    .color(color::TXT_MUTED),
            );
            let private_key = wallet.private_key_hex();
            ui.add(
                egui::Label::new(
                    RichText::new(private_key.as_str())
                        .family(FontFamily::Monospace)
                        .color(color::TXT),
                )
                .selectable(true),
            );
        });
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
