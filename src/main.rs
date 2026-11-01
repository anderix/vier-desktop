// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

mod board;
mod board_view;
mod engine;
mod game;
mod palette;
mod rng;

use std::path::PathBuf;

use eframe::egui::{self, Align, Button, Color32, Key, Layout, RichText, Vec2};

use board::Player;
use engine::Difficulty;
use game::{Game, Mode, Phase};
use palette::{DISC_RED, QUIET_GRAY, SEGMENT_SELECTED, SEGMENT_TRACK, disc_color};

const APP_ID: &str = "vier-desktop";
const RESULT_DELAY_SECONDS: f64 = 1.0;

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_app_id(APP_ID)
        .with_title("Vier")
        .with_inner_size([520.0, 640.0])
        .with_min_inner_size([320.0, 420.0]);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Vier",
        options,
        Box::new(|creation| Ok(Box::new(Vier::new(&creation.egui_ctx)))),
    )
}

#[derive(PartialEq)]
enum Screen {
    Home,
    Playing,
}

struct Vier {
    game: Game,
    screen: Screen,
    pending_difficulty: Option<Difficulty>,
}

impl Vier {
    fn new(ctx: &egui::Context) -> Vier {
        ctx.set_theme(egui::Theme::Dark);
        ctx.style_mut_of(egui::Theme::Dark, |style| {
            style.visuals.panel_fill = Color32::BLACK;
            style.spacing.button_padding = egui::vec2(14.0, 8.0);
        });
        let vier = Vier {
            game: Game::open(state_path()),
            screen: Screen::Home,
            pending_difficulty: None,
        };
        #[cfg(debug_assertions)]
        let vier = vier.with_launch_arguments();
        vier
    }

    #[cfg(debug_assertions)]
    fn with_launch_arguments(mut self) -> Vier {
        let arguments: Vec<String> = std::env::args().collect();
        if let Some(index) = arguments.iter().position(|argument| argument == "--demo")
            && let Some(columns) = arguments.get(index + 1)
        {
            let columns: Vec<usize> = columns
                .split(',')
                .filter_map(|column| column.parse().ok())
                .collect();
            self.game.load_demo(&columns);
            self.screen = Screen::Playing;
        } else if arguments.iter().any(|argument| argument == "--play") {
            self.play(0.0);
        }
        self
    }

    fn play(&mut self, now: f64) {
        if !self.game.has_game_in_progress() || self.game.mode() != self.game.memory.mode {
            self.game.new_game(now);
        }
        self.screen = Screen::Playing;
    }

    fn go_home(&mut self) {
        self.game.save();
        self.screen = Screen::Home;
    }

    fn home(&mut self, ui: &mut egui::Ui, now: f64) {
        if ui.input(|input| input.key_pressed(Key::Enter)) && self.pending_difficulty.is_none() {
            self.play(now);
        }
        let width = ui.available_width().min(360.0);
        ui.add_space((ui.available_height() - 330.0).max(0.0) / 2.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(width);
            title(ui);
            ui.add_space(32.0);
            let play = Button::new(RichText::new("Play").size(20.0).color(Color32::WHITE))
                .fill(DISC_RED)
                .corner_radius(10.0)
                .min_size(Vec2::new(width, 48.0));
            if ui.add(play).clicked() {
                self.play(now);
            }
            ui.add_space(28.0);
            let mut mode = self.game.memory.mode;
            segmented(ui, width, &mut mode, &Mode::ALL, |mode| mode.title());
            if mode != self.game.memory.mode {
                self.game.memory.mode = mode;
                self.game.persist();
            }
            if mode == Mode::VersusComputer {
                ui.add_space(14.0);
                let mut difficulty = self.game.memory.difficulty;
                segmented(ui, width, &mut difficulty, &Difficulty::ALL, |difficulty| {
                    difficulty.title()
                });
                self.choose_difficulty(difficulty);
            }
        });
        self.confirm_difficulty(ui);
    }

    fn choose_difficulty(&mut self, choice: Difficulty) {
        if choice == self.game.memory.difficulty {
            return;
        }
        if self.game.has_game_in_progress() && self.game.mode() == Mode::VersusComputer {
            self.pending_difficulty = Some(choice);
        } else {
            self.game.memory.difficulty = choice;
            self.game.persist();
        }
    }

    fn confirm_difficulty(&mut self, ui: &mut egui::Ui) {
        let Some(choice) = self.pending_difficulty else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("confirm-difficulty")).show(ui.ctx(), |ui| {
            ui.set_width(280.0);
            ui.heading("Start a new game?");
            ui.add_space(6.0);
            ui.label(format!(
                "Switching to {} ends the game in progress.",
                choice.title()
            ));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("New game").clicked() {
                    self.game.memory.difficulty = choice;
                    self.game.abandon_game();
                    self.pending_difficulty = None;
                }
                if ui.button("Cancel").clicked() {
                    self.pending_difficulty = None;
                }
            });
        });
        if modal.should_close() {
            self.pending_difficulty = None;
        }
    }

    fn playing(&mut self, ui: &mut egui::Ui, now: f64) {
        self.game.resume(now);
        self.handle_keys(ui, now);

        ui.horizontal(|ui| {
            if ui.button("‹ Home").clicked() {
                self.go_home();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                turn_indicator(ui, &self.game)
            });
        });
        ui.add_space(8.0);

        let showing_result = matches!(self.game.phase(), Phase::Finished(_))
            && now - self.game.finished_at() >= RESULT_DELAY_SECONDS;
        let result_height = if showing_result { 110.0 } else { 0.0 };
        let board_height = (ui.available_height() - result_height).max(0.0);
        let board_area = ui.allocate_ui(Vec2::new(ui.available_width(), board_height), |ui| {
            board_view::show(ui, &self.game, now)
        });
        let input = board_area.inner;
        if let Some(column) = input.hovered_column {
            self.game.select(column);
        }
        if let Some(column) = input.clicked_column {
            self.game.play(column, now);
        }

        if showing_result && let Some(title) = self.game.result_title() {
            self.result_panel(ui, title, now);
        }

        if input.animating || matches!(self.game.phase(), Phase::Finished(_)) && !showing_result {
            ui.ctx().request_repaint();
        }
    }

    fn handle_keys(&mut self, ui: &egui::Ui, now: f64) {
        ui.input(|input| {
            if input.key_pressed(Key::ArrowLeft) {
                self.game.step_selection(false);
            }
            if input.key_pressed(Key::ArrowRight) {
                self.game.step_selection(true);
            }
            for (column, key) in [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
            ]
            .into_iter()
            .enumerate()
            {
                if input.key_pressed(key) {
                    self.game.select(column);
                }
            }
            if [Key::ArrowDown, Key::Space, Key::Enter]
                .into_iter()
                .any(|key| input.key_pressed(key))
            {
                if matches!(self.game.phase(), Phase::Finished(_)) {
                    self.game.new_game(now);
                } else {
                    self.game.drop_in_selected_column(now);
                }
            }
            if input.key_pressed(Key::Escape) {
                self.go_home();
            }
        });
    }

    fn result_panel(&mut self, ui: &mut egui::Ui, title: &str, now: f64) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(title).size(28.0).color(Color32::WHITE));
            ui.add_space(14.0);
            let width = ui.available_width().min(360.0);
            let button_width = (width - 16.0) / 2.0;
            ui.allocate_ui_with_layout(
                Vec2::new(width, 48.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    if ui
                        .add(capsule("Play again", DISC_RED, button_width))
                        .clicked()
                    {
                        self.game.new_game(now);
                    }
                    if ui.add(capsule("Home", QUIET_GRAY, button_width)).clicked() {
                        self.go_home();
                    }
                },
            );
        });
    }
}

impl eframe::App for Vier {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = ui.input(|input| input.time);
        self.game.advance(now);
        if ui.input(|input| input.viewport().close_requested()) {
            self.game.save();
        }
        egui::CentralPanel::default().show(ui, |ui| match self.screen {
            Screen::Home => self.home(ui, now),
            Screen::Playing => self.playing(ui, now),
        });
        if let Some(deadline) = self.game.next_deadline() {
            ui.ctx()
                .request_repaint_after_secs((deadline - now).max(0.0) as f32);
        }
    }
}

fn title(ui: &mut egui::Ui) {
    let size = 36.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(280.0, 80.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        "Vier",
        egui::FontId::proportional(64.0),
        Color32::WHITE,
    );
    board_view::paint_disc(
        &painter,
        center - Vec2::new(105.0, 0.0),
        size,
        Player::Red,
        1.0,
    );
    board_view::paint_disc(
        &painter,
        center + Vec2::new(105.0, 0.0),
        size,
        Player::Yellow,
        1.0,
    );
}

fn segmented<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    width: f32,
    value: &mut T,
    choices: &[T],
    label: impl Fn(T) -> &'static str,
) {
    let inset = 3.0;
    let each = (width - inset * 2.0) / choices.len() as f32;
    egui::Frame::new()
        .fill(SEGMENT_TRACK)
        .corner_radius(9.0)
        .inner_margin(inset)
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(width - inset * 2.0, 30.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for &choice in choices {
                        let selected = *value == choice;
                        let button =
                            Button::new(RichText::new(label(choice)).color(Color32::WHITE))
                                .fill(if selected {
                                    SEGMENT_SELECTED
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(7.0)
                                .min_size(Vec2::new(each, 30.0));
                        if ui.add(button).clicked() {
                            *value = choice;
                        }
                    }
                },
            );
        });
}

fn capsule(text: &str, fill: Color32, width: f32) -> Button<'_> {
    Button::new(RichText::new(text).size(17.0).color(Color32::WHITE))
        .fill(fill)
        .corner_radius(24.0)
        .min_size(Vec2::new(width, 48.0))
}

fn turn_indicator(ui: &mut egui::Ui, game: &Game) {
    match game.phase() {
        Phase::ComputerThinking => {
            ui.label(RichText::new("…").size(20.0));
        }
        Phase::Finished(_) => {}
        Phase::PlayerTurn | Phase::AnimatingDrop => {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 8.0, disc_color(game.turn()));
        }
    }
}

fn state_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(base.join(APP_ID).join("state.json"))
}
