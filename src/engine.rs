// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use serde::{Deserialize, Serialize};

use crate::board::{Board, LINES, Player, ROWS};
use crate::rng::SplitMix64;

pub const SEARCH_ORDER: [usize; 7] = [3, 2, 4, 1, 5, 0, 6];
const WIN_SCORE: i32 = 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    #[default]
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard];

    pub fn title(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Medium => "Medium",
            Difficulty::Hard => "Hard",
        }
    }

    fn search_depth(self) -> i32 {
        match self {
            Difficulty::Easy => 1,
            Difficulty::Medium => 2,
            Difficulty::Hard => 4,
        }
    }

    fn random_move_chance(self) -> f64 {
        match self {
            Difficulty::Easy => 0.65,
            Difficulty::Medium => 0.5,
            Difficulty::Hard => 0.0,
        }
    }

    fn blocks_immediate_wins(self) -> bool {
        self != Difficulty::Easy
    }
}

pub fn best_move(
    board: &Board,
    player: Player,
    difficulty: Difficulty,
    rng: &mut SplitMix64,
) -> usize {
    let moves: Vec<usize> = SEARCH_ORDER
        .into_iter()
        .filter(|&column| board.can_drop(column))
        .collect();
    assert!(!moves.is_empty(), "no legal move on a full board");

    if let Some(win) = immediate_win(board, player) {
        return win;
    }
    if difficulty.blocks_immediate_wins()
        && let Some(block) = immediate_win(board, player.opponent())
    {
        return block;
    }
    if rng.unit() < difficulty.random_move_chance() {
        return moves[rng.below(moves.len())];
    }

    let mut search = board.clone();
    let mut best_column = moves[0];
    let mut alpha = -i32::MAX;
    for &column in &moves {
        let score = score(
            column,
            &mut search,
            player,
            difficulty.search_depth(),
            0,
            -i32::MAX,
            -alpha,
        );
        if score > alpha {
            alpha = score;
            best_column = column;
        }
    }
    best_column
}

fn immediate_win(board: &Board, player: Player) -> Option<usize> {
    let mut search = board.clone();
    SEARCH_ORDER.into_iter().find(|&column| {
        let Some(position) = search.drop(player, column) else {
            return false;
        };
        let wins = search.completes_four(position);
        search.remove(position);
        wins
    })
}

fn score(
    column: usize,
    board: &mut Board,
    player: Player,
    depth: i32,
    ply: i32,
    alpha: i32,
    beta: i32,
) -> i32 {
    let position = board
        .drop(player, column)
        .expect("searched column has room");
    let value = if board.completes_four(position) {
        WIN_SCORE - (ply + 1)
    } else {
        -negamax(board, player.opponent(), depth - 1, ply + 1, -beta, -alpha)
    };
    board.remove(position);
    value
}

fn negamax(
    board: &mut Board,
    player: Player,
    depth: i32,
    ply: i32,
    mut alpha: i32,
    beta: i32,
) -> i32 {
    if board.is_full() {
        return 0;
    }
    if depth == 0 {
        return evaluate(board, player);
    }
    let mut best = -i32::MAX;
    for column in SEARCH_ORDER {
        if !board.can_drop(column) {
            continue;
        }
        let score = score(column, board, player, depth, ply, alpha, beta);
        best = best.max(score);
        alpha = alpha.max(score);
        if alpha >= beta {
            break;
        }
    }
    best
}

fn evaluate(board: &Board, player: Player) -> i32 {
    let mut score = 3
        * (0..ROWS)
            .filter(|&row| board.at(3, row) == Some(player))
            .count() as i32;
    for line in LINES.iter() {
        let mut own = 0;
        let mut opponent = 0;
        for &position in line {
            match board.get(position) {
                Some(cell) if cell == player => own += 1,
                Some(_) => opponent += 1,
                None => {}
            }
        }
        score += match (own, opponent) {
            (3, 0) => 5,
            (2, 0) => 2,
            (0, 3) => -4,
            _ => 0,
        };
    }
    score
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::board::{COLUMNS, played};

    fn picture(rows: &str) -> Board {
        let mut board = Board::default();
        let lines: Vec<&str> = rows
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(lines.len(), ROWS);
        for line in lines.iter().rev() {
            for (column, cell) in line.chars().enumerate() {
                match cell {
                    'R' => _ = board.drop(Player::Red, column),
                    'Y' => _ = board.drop(Player::Yellow, column),
                    _ => {}
                }
            }
        }
        board
    }

    fn pick(board: &Board, player: Player, difficulty: Difficulty, seed: u64) -> usize {
        best_move(board, player, difficulty, &mut SplitMix64::new(seed))
    }

    #[test]
    fn every_level_takes_an_immediate_win() {
        let cases = [
            (
                "horizontal",
                picture(".......\n.......\n.......\n.......\n..RR...\n.YYY.R."),
                Player::Yellow,
                vec![0, 4],
            ),
            (
                "vertical",
                picture(".......\n.......\n.......\nR......\nR...Y..\nR..YY.."),
                Player::Red,
                vec![0],
            ),
            (
                "diagonal",
                picture(".......\n.......\n.......\n..YR...\n.YRY...\nYRYR..."),
                Player::Yellow,
                vec![3],
            ),
        ];
        for difficulty in Difficulty::ALL {
            for (name, board, player, columns) in &cases {
                for seed in 0..40 {
                    let column = pick(board, *player, difficulty, seed);
                    assert!(
                        columns.contains(&column),
                        "{name} win missed at {difficulty:?} with seed {seed}"
                    );
                }
            }
        }
    }

    #[test]
    fn medium_and_hard_block_an_immediate_win() {
        let cases = [
            (
                "horizontal",
                picture(".......\n.......\n.......\n.......\n...Y...\nYRRR.Y."),
                4,
            ),
            (
                "vertical",
                picture(".......\n.......\n.......\n..R....\n..R.Y..\n..R.YY."),
                2,
            ),
            (
                "diagonal",
                picture(".......\n.......\n.......\n..RY...\n.RYR...\nRYRY..."),
                3,
            ),
        ];
        for difficulty in [Difficulty::Medium, Difficulty::Hard] {
            for (name, board, column) in &cases {
                assert_eq!(
                    pick(board, Player::Yellow, difficulty, 1),
                    *column,
                    "{name} block missed at {difficulty:?}"
                );
            }
        }
    }

    #[test]
    fn an_own_win_beats_a_block() {
        let board = picture(".......\n.......\n.......\n......Y\n......Y\n.RRR..Y");
        for difficulty in Difficulty::ALL {
            assert_eq!(pick(&board, Player::Yellow, difficulty, 1), 6);
        }
    }

    #[test]
    fn plays_the_only_open_column() {
        let board = picture("RYRY.YR\nRYRY.YR\nYRYRRRY\nYRYRYRY\nRYRYRYR\nRYRYRYR");
        for difficulty in Difficulty::ALL {
            assert_eq!(pick(&board, Player::Red, difficulty, 1), 4);
        }
    }

    #[test]
    fn never_returns_an_illegal_column() {
        let mut chooser = SplitMix64::new(7);
        for difficulty in Difficulty::ALL {
            let games = if difficulty == Difficulty::Hard {
                3
            } else {
                20
            };
            for game in 0..games {
                let mut board = Board::default();
                let mut player = Player::Red;
                let mut placed = 0;
                while board.outcome().is_none() {
                    let column = pick(&board, player, difficulty, game * 100 + placed);
                    assert!(board.can_drop(column), "illegal column {column}");
                    let legal: Vec<usize> = (0..COLUMNS).filter(|&c| board.can_drop(c)).collect();
                    let chosen = if chooser.below(2) == 0 {
                        column
                    } else {
                        legal[chooser.below(legal.len())]
                    };
                    board.drop(player, chosen);
                    player = player.opponent();
                    placed += 1;
                }
            }
        }
    }

    #[test]
    fn hard_search_is_fast() {
        for board in [Board::default(), played(&[3, 3, 2, 4, 4, 2, 5, 1, 1, 5])] {
            assert!(board.outcome().is_none());
            let started = Instant::now();
            pick(&board, Player::Red, Difficulty::Hard, 1);
            assert!(
                started.elapsed() < Duration::from_millis(100),
                "took {:?}",
                started.elapsed()
            );
        }
    }
}
