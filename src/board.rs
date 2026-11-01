// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

pub const COLUMNS: usize = 7;
pub const ROWS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Player {
    Red,
    Yellow,
}

impl Player {
    pub fn opponent(self) -> Player {
        match self {
            Player::Red => Player::Yellow,
            Player::Yellow => Player::Red,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub column: usize,
    pub row: usize,
}

impl Position {
    fn index(self) -> usize {
        self.row * COLUMNS + self.column
    }
}

pub type Line = [Position; 4];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Win {
    pub player: Player,
    pub cells: Line,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Win(Win),
    Draw,
}

pub static LINES: LazyLock<Vec<Line>> = LazyLock::new(|| {
    let directions: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
    let mut lines = Vec::new();
    for column in 0..COLUMNS as isize {
        for row in 0..ROWS as isize {
            for (dc, dr) in directions {
                let end_column = column + dc * 3;
                let end_row = row + dr * 3;
                if !(0..COLUMNS as isize).contains(&end_column)
                    || !(0..ROWS as isize).contains(&end_row)
                {
                    continue;
                }
                lines.push(std::array::from_fn(|step| Position {
                    column: (column + dc * step as isize) as usize,
                    row: (row + dr * step as isize) as usize,
                }));
            }
        }
    }
    lines
});

static LINES_THROUGH_CELL: LazyLock<Vec<Vec<Line>>> = LazyLock::new(|| {
    let mut table = vec![Vec::new(); COLUMNS * ROWS];
    for line in LINES.iter() {
        for position in line {
            table[position.index()].push(*line);
        }
    }
    table
});

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    cells: [[Option<Player>; COLUMNS]; ROWS],
}

impl Board {
    pub fn get(&self, position: Position) -> Option<Player> {
        self.cells[position.row][position.column]
    }

    pub fn at(&self, column: usize, row: usize) -> Option<Player> {
        self.cells[row][column]
    }

    pub fn can_drop(&self, column: usize) -> bool {
        column < COLUMNS && self.cells[ROWS - 1][column].is_none()
    }

    pub fn is_full(&self) -> bool {
        !(0..COLUMNS).any(|column| self.can_drop(column))
    }

    pub fn is_empty(&self) -> bool {
        self.cells.iter().flatten().all(Option::is_none)
    }

    pub fn drop(&mut self, player: Player, column: usize) -> Option<Position> {
        if !self.can_drop(column) {
            return None;
        }
        let row = (0..ROWS).find(|&row| self.cells[row][column].is_none())?;
        self.cells[row][column] = Some(player);
        Some(Position { column, row })
    }

    pub fn remove(&mut self, position: Position) {
        self.cells[position.row][position.column] = None;
    }

    pub fn winner(&self) -> Option<Win> {
        LINES.iter().find_map(|line| {
            let player = self.get(line[0])?;
            line.iter()
                .all(|&position| self.get(position) == Some(player))
                .then_some(Win {
                    player,
                    cells: *line,
                })
        })
    }

    pub fn completes_four(&self, position: Position) -> bool {
        let Some(player) = self.get(position) else {
            return false;
        };
        LINES_THROUGH_CELL[position.index()]
            .iter()
            .any(|line| line.iter().all(|&cell| self.get(cell) == Some(player)))
    }

    pub fn outcome(&self) -> Option<Outcome> {
        if let Some(win) = self.winner() {
            Some(Outcome::Win(win))
        } else if self.is_full() {
            Some(Outcome::Draw)
        } else {
            None
        }
    }
}

#[cfg(test)]
pub fn played(columns: &[usize]) -> Board {
    let mut board = Board::default();
    let mut turn = Player::Red;
    for &column in columns {
        board.drop(turn, column).expect("column has room");
        turn = turn.opponent();
    }
    board
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_69_winning_lines() {
        assert_eq!(LINES.len(), 69);
    }

    #[test]
    fn discs_stack_from_the_bottom() {
        let mut board = Board::default();
        assert_eq!(
            board.drop(Player::Red, 3),
            Some(Position { column: 3, row: 0 })
        );
        assert_eq!(
            board.drop(Player::Yellow, 3),
            Some(Position { column: 3, row: 1 })
        );
    }

    #[test]
    fn a_full_column_refuses_a_disc() {
        let mut board = played(&[0, 0, 0, 0, 0, 0]);
        assert!(!board.can_drop(0));
        assert_eq!(board.drop(Player::Red, 0), None);
    }

    #[test]
    fn four_across_wins() {
        let board = played(&[0, 0, 1, 1, 2, 2, 3]);
        assert_eq!(board.winner().map(|win| win.player), Some(Player::Red));
        assert!(board.completes_four(Position { column: 3, row: 0 }));
    }

    #[test]
    fn four_on_a_diagonal_wins() {
        let board = played(&[0, 1, 1, 2, 2, 3, 2, 3, 3, 6, 3]);
        assert_eq!(board.winner().map(|win| win.player), Some(Player::Red));
    }

    #[test]
    fn a_full_board_with_no_four_is_a_draw() {
        let mut board = Board::default();
        let pattern = [Player::Red, Player::Red, Player::Yellow, Player::Yellow];
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                board
                    .drop(pattern[(column + 2 * (row % 2)) % 4], column)
                    .unwrap();
            }
        }
        assert_eq!(board.outcome(), Some(Outcome::Draw));
    }
}
