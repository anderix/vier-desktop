// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::board::{Board, COLUMNS, Outcome, Player, Position};
use crate::engine::{self, Difficulty};
use crate::rng::SplitMix64;

pub const DROP_SECONDS: f64 = 0.3;
pub const THINKING_SECONDS: f64 = 0.4;
pub const HOVER_SECONDS: f64 = 0.25;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    VersusComputer,
    HotSeat,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::VersusComputer, Mode::HotSeat];

    pub fn title(self) -> &'static str {
        match self {
            Mode::VersusComputer => "Against the computer",
            Mode::HotSeat => "Two players",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    PlayerTurn,
    AnimatingDrop,
    ComputerThinking,
    Finished(Outcome),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    FinishDrop,
    RevealComputerColumn(usize),
    ComputerDrop(usize),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedGame {
    board: Board,
    turn: Player,
    starter: Player,
    difficulty: Difficulty,
    mode: Mode,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Memory {
    pub difficulty: Difficulty,
    pub mode: Mode,
    next_starter: Option<Player>,
    saved_game: Option<SavedGame>,
}

pub struct Game {
    pub memory: Memory,
    path: Option<PathBuf>,
    board: Board,
    turn: Player,
    phase: Phase,
    mode: Mode,
    difficulty: Difficulty,
    starter: Player,
    last_drop: Option<(Position, f64)>,
    finished_at: f64,
    selected_column: usize,
    computer_column: Option<usize>,
    pending: Option<(f64, Step)>,
    rng: SplitMix64,
}

impl Game {
    pub fn open(path: Option<PathBuf>) -> Game {
        let memory = path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut game = Game {
            memory,
            path,
            board: Board::default(),
            turn: Player::Red,
            phase: Phase::PlayerTurn,
            mode: Mode::VersusComputer,
            difficulty: Difficulty::Easy,
            starter: Player::Red,
            last_drop: None,
            finished_at: 0.0,
            selected_column: 3,
            computer_column: None,
            pending: None,
            rng: SplitMix64::from_clock(),
        };
        game.restore();
        game
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn turn(&self) -> Player {
        self.turn
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn last_drop(&self) -> Option<(Position, f64)> {
        self.last_drop
    }

    pub fn finished_at(&self) -> f64 {
        self.finished_at
    }

    pub fn has_game_in_progress(&self) -> bool {
        !self.board.is_empty() && self.board.outcome().is_none()
    }

    pub fn hover(&self) -> Option<(Player, usize)> {
        match self.phase {
            Phase::PlayerTurn => Some((self.turn, self.selected_column)),
            Phase::ComputerThinking => self.computer_column.map(|column| (Player::Yellow, column)),
            _ => None,
        }
    }

    pub fn winning_cells(&self) -> &[Position] {
        match &self.phase {
            Phase::Finished(Outcome::Win(win)) => &win.cells,
            _ => &[],
        }
    }

    pub fn result_title(&self) -> Option<&'static str> {
        let Phase::Finished(outcome) = &self.phase else {
            return None;
        };
        let Outcome::Win(win) = outcome else {
            return Some("Draw");
        };
        Some(match (self.mode, win.player) {
            (Mode::VersusComputer, Player::Red) => "You win!",
            (Mode::VersusComputer, Player::Yellow) => "Computer wins",
            (Mode::HotSeat, Player::Red) => "Red wins!",
            (Mode::HotSeat, Player::Yellow) => "Yellow wins!",
        })
    }

    pub fn new_game(&mut self, now: f64) {
        self.pending = None;
        self.mode = self.memory.mode;
        self.difficulty = self.memory.difficulty;
        self.starter = match self.mode {
            Mode::VersusComputer => {
                let starter = self.memory.next_starter.unwrap_or(Player::Red);
                self.memory.next_starter = Some(starter.opponent());
                starter
            }
            Mode::HotSeat => Player::Red,
        };
        self.board = Board::default();
        self.turn = self.starter;
        self.last_drop = None;
        self.selected_column = 3;
        self.begin_turn(now);
        self.save();
    }

    pub fn abandon_game(&mut self) {
        self.pending = None;
        self.board = Board::default();
        self.last_drop = None;
        self.phase = Phase::PlayerTurn;
        self.save();
    }

    pub fn resume(&mut self, now: f64) {
        if self.phase == Phase::ComputerThinking && self.pending.is_none() {
            self.play_computer_turn(now);
        }
    }

    pub fn select(&mut self, column: usize) {
        if self.phase != Phase::PlayerTurn || column == self.selected_column || column >= COLUMNS {
            return;
        }
        let rightward = column > self.selected_column;
        self.selected_column = self
            .open_column(column, rightward)
            .or_else(|| self.open_column(column, !rightward))
            .unwrap_or(column);
    }

    pub fn step_selection(&mut self, rightward: bool) {
        let target = if rightward {
            (self.selected_column + 1..COLUMNS).find(|&column| self.board.can_drop(column))
        } else {
            (0..self.selected_column)
                .rev()
                .find(|&column| self.board.can_drop(column))
        };
        if let Some(column) = target {
            self.select(column);
        }
    }

    pub fn drop_in_selected_column(&mut self, now: f64) {
        self.play(self.selected_column, now);
    }

    pub fn play(&mut self, column: usize, now: f64) {
        if self.phase != Phase::PlayerTurn || !self.board.can_drop(column) {
            return;
        }
        self.selected_column = column;
        self.drop(column, now);
    }

    pub fn next_deadline(&self) -> Option<f64> {
        self.pending.map(|(at, _)| at)
    }

    pub fn advance(&mut self, now: f64) {
        while let Some((at, step)) = self.pending {
            if at > now {
                return;
            }
            self.pending = None;
            match step {
                Step::FinishDrop => self.finish_drop(at),
                Step::RevealComputerColumn(column) => {
                    self.computer_column = Some(column);
                    self.pending = Some((at + HOVER_SECONDS, Step::ComputerDrop(column)));
                }
                Step::ComputerDrop(column) => {
                    self.computer_column = None;
                    self.drop(column, at);
                }
            }
        }
    }

    pub fn save(&mut self) {
        self.memory.saved_game = self.has_game_in_progress().then(|| SavedGame {
            board: self.board.clone(),
            turn: if self.phase == Phase::AnimatingDrop {
                self.turn.opponent()
            } else {
                self.turn
            },
            starter: self.starter,
            difficulty: self.difficulty,
            mode: self.mode,
        });
        self.persist();
    }

    pub fn persist(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let Ok(bytes) = serde_json::to_vec_pretty(&self.memory) else {
            return;
        };
        if let Some(parent) = path.parent() {
            _ = std::fs::create_dir_all(parent);
        }
        let staged = path.with_extension("tmp");
        if std::fs::write(&staged, bytes).is_ok() {
            _ = std::fs::rename(&staged, path);
        }
    }

    #[cfg(debug_assertions)]
    pub fn load_demo(&mut self, columns: &[usize]) {
        self.pending = None;
        self.mode = Mode::VersusComputer;
        self.board = Board::default();
        self.turn = Player::Red;
        for &column in columns {
            self.board.drop(self.turn, column);
            self.turn = self.turn.opponent();
        }
        self.phase = match self.board.outcome() {
            Some(outcome) => Phase::Finished(outcome),
            None if self.is_human(self.turn) => Phase::PlayerTurn,
            None => Phase::ComputerThinking,
        };
    }

    fn restore(&mut self) {
        let Some(saved) = self.memory.saved_game.clone() else {
            return;
        };
        if saved.board.outcome().is_some() {
            return;
        }
        self.board = saved.board;
        self.turn = saved.turn;
        self.starter = saved.starter;
        self.difficulty = saved.difficulty;
        self.mode = saved.mode;
        self.selected_column = self
            .open_column(3, true)
            .or_else(|| self.open_column(3, false))
            .unwrap_or(3);
        self.phase = if self.is_human(self.turn) {
            Phase::PlayerTurn
        } else {
            Phase::ComputerThinking
        };
    }

    fn drop(&mut self, column: usize, now: f64) {
        let Some(position) = self.board.drop(self.turn, column) else {
            return;
        };
        self.last_drop = Some((position, now));
        self.phase = Phase::AnimatingDrop;
        self.pending = Some((now + DROP_SECONDS, Step::FinishDrop));
    }

    fn finish_drop(&mut self, now: f64) {
        if let Some(outcome) = self.board.outcome() {
            self.phase = Phase::Finished(outcome);
            self.finished_at = now;
            self.save();
            return;
        }
        self.turn = self.turn.opponent();
        self.begin_turn(now);
        self.save();
    }

    fn begin_turn(&mut self, now: f64) {
        let from = self.selected_column;
        self.selected_column = self
            .open_column(from, true)
            .or_else(|| self.open_column(from, false))
            .unwrap_or(from);
        if self.is_human(self.turn) {
            self.phase = Phase::PlayerTurn;
        } else {
            self.play_computer_turn(now);
        }
    }

    fn play_computer_turn(&mut self, now: f64) {
        self.phase = Phase::ComputerThinking;
        self.computer_column = Some(self.selected_column);
        let column = engine::best_move(&self.board, self.turn, self.difficulty, &mut self.rng);
        self.pending = Some((now + THINKING_SECONDS, Step::RevealComputerColumn(column)));
    }

    fn is_human(&self, player: Player) -> bool {
        self.mode == Mode::HotSeat || player == Player::Red
    }

    fn open_column(&self, from: usize, rightward: bool) -> Option<usize> {
        if rightward {
            (from..COLUMNS).find(|&column| self.board.can_drop(column))
        } else {
            (0..=from).rev().find(|&column| self.board.can_drop(column))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::ROWS;

    const SETTLE: f64 = DROP_SECONDS + 0.15;

    fn game_in(mode: Mode, difficulty: Difficulty) -> Game {
        let mut game = Game::open(None);
        game.memory.mode = mode;
        game.memory.difficulty = difficulty;
        game.new_game(0.0);
        game
    }

    fn scratch_path(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("vier-test-{}-{name}.json", std::process::id()));
        _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn a_drop_lands_then_passes_the_turn() {
        let mut game = game_in(Mode::HotSeat, Difficulty::Easy);
        game.play(5, 0.0);
        assert_eq!(game.board().at(5, 0), Some(Player::Red));
        assert_eq!(*game.phase(), Phase::AnimatingDrop);
        game.advance(SETTLE);
        assert_eq!(*game.phase(), Phase::PlayerTurn);
        assert_eq!(game.turn(), Player::Yellow);
    }

    #[test]
    fn input_is_ignored_while_a_disc_falls() {
        let mut game = game_in(Mode::HotSeat, Difficulty::Easy);
        game.play(1, 0.0);
        game.play(2, 0.0);
        assert_eq!(game.board().at(2, 0), None);
    }

    #[test]
    fn selection_skips_full_columns_and_a_full_column_is_refused() {
        let mut game = game_in(Mode::HotSeat, Difficulty::Easy);
        let mut now = 0.0;
        for _ in 0..ROWS {
            game.play(4, now);
            now += SETTLE;
            game.advance(now);
        }
        assert!(!game.board().can_drop(4));
        game.play(4, now);
        assert_eq!(*game.phase(), Phase::PlayerTurn);

        game.select(3);
        game.select(4);
        assert_eq!(game.selected_column, 5);
        game.select(4);
        assert_eq!(game.selected_column, 3);
        game.step_selection(true);
        assert_eq!(game.selected_column, 5);
    }

    #[test]
    fn the_computer_replies_with_yellow() {
        let mut game = game_in(Mode::VersusComputer, Difficulty::Medium);
        assert_eq!(game.starter, Player::Red);
        game.play(3, 0.0);
        game.advance(SETTLE + THINKING_SECONDS + HOVER_SECONDS + SETTLE);
        let yellow = (0..COLUMNS)
            .flat_map(|column| (0..ROWS).map(move |row| (column, row)))
            .filter(|&(column, row)| game.board().at(column, row) == Some(Player::Yellow))
            .count();
        assert_eq!(yellow, 1);
        assert_eq!(game.turn(), Player::Red);
        assert_eq!(*game.phase(), Phase::PlayerTurn);
    }

    #[test]
    fn the_first_move_alternates_against_the_computer() {
        let mut game = game_in(Mode::VersusComputer, Difficulty::Easy);
        assert_eq!(game.starter, Player::Red);
        game.new_game(0.0);
        assert_eq!(game.starter, Player::Yellow);
        assert_eq!(*game.phase(), Phase::ComputerThinking);
        game.new_game(0.0);
        assert_eq!(game.starter, Player::Red);
    }

    #[test]
    fn a_saved_game_restores_position_and_turn() {
        let path = scratch_path("restore");
        let mut game = Game::open(Some(path.clone()));
        game.memory.mode = Mode::HotSeat;
        game.memory.difficulty = Difficulty::Hard;
        game.new_game(0.0);
        game.play(2, 0.0);
        game.save();

        let restored = Game::open(Some(path.clone()));
        assert_eq!(restored.board(), game.board());
        assert_eq!(restored.turn(), Player::Yellow);
        assert_eq!(restored.mode(), Mode::HotSeat);
        assert_eq!(restored.difficulty, Difficulty::Hard);
        assert_eq!(*restored.phase(), Phase::PlayerTurn);
        _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_finished_game_is_not_saved() {
        let path = scratch_path("finished");
        let mut game = Game::open(Some(path.clone()));
        game.memory.difficulty = Difficulty::Medium;
        game.new_game(0.0);
        game.load_demo(&[0, 6, 0, 6, 0, 6]);
        game.play(0, 0.0);
        game.advance(SETTLE);
        assert_eq!(game.result_title(), Some("You win!"));
        assert!(game.memory.saved_game.is_none());
        assert!(!Game::open(Some(path.clone())).has_game_in_progress());
        _ = std::fs::remove_file(path);
    }
}
