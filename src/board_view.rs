// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::f32::consts::TAU;

use eframe::egui::{
    self, Color32, Mesh, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2, pos2, vec2,
};

use crate::board::{COLUMNS, Player, Position, ROWS};
use crate::game::Game;
use crate::palette::{BOARD_BLUE, disc_color};

const FIELD_ROWS: usize = ROWS + 1;
const DISC_RATIO: f32 = 0.84;
const HOLE_RATIO: f32 = 0.8;
const CORNER_RATIO: f32 = 0.2;
const FALL_SECONDS: f32 = 0.25;
const HOVER_SLIDE_SECONDS: f32 = 0.15;
const PULSE_PERIOD: f32 = 1.0;
const HOLE_SEGMENTS: usize = 48;

pub struct BoardInput {
    pub hovered_column: Option<usize>,
    pub clicked_column: Option<usize>,
    pub animating: bool,
}

pub fn show(ui: &mut egui::Ui, game: &Game, now: f64) -> BoardInput {
    let available = ui.available_rect_before_wrap();
    let cell = (available.width() / COLUMNS as f32)
        .min(available.height() / FIELD_ROWS as f32)
        .floor();
    let field = Rect::from_center_size(
        available.center(),
        vec2(cell * COLUMNS as f32, cell * FIELD_ROWS as f32),
    );
    let response = ui.allocate_rect(field, Sense::click());
    let painter = ui.painter_at(field);
    let geometry = Geometry {
        origin: field.min,
        cell,
    };

    let column_at = |pos: Pos2| {
        ((pos.x - field.min.x) / cell)
            .floor()
            .clamp(0.0, COLUMNS as f32 - 1.0) as usize
    };
    let pointer_moved = ui.input(|input| input.pointer.delta() != Vec2::ZERO);
    let hovered_column = response
        .hover_pos()
        .filter(|_| pointer_moved)
        .map(column_at);
    let clicked_column = response
        .clicked()
        .then(|| response.interact_pointer_pos().map(column_at))
        .flatten();

    let mut animating = false;

    if let Some((player, column)) = game.hover() {
        let x = ui.ctx().animate_value_with_time(
            egui::Id::new("hover-column"),
            column as f32,
            HOVER_SLIDE_SECONDS,
        );
        animating |= x != column as f32;
        paint_disc(
            &painter,
            geometry.center(x, 0.0),
            cell * DISC_RATIO,
            player,
            1.0,
        );
    }

    let winning = game.winning_cells();
    let pulse = if winning.is_empty() {
        1.0
    } else {
        animating = true;
        let phase = (now as f32 % PULSE_PERIOD) / PULSE_PERIOD;
        1.0 + 0.12 * (0.5 - 0.5 * (phase * TAU).cos())
    };

    for column in 0..COLUMNS {
        for row in 0..ROWS {
            let Some(player) = game.board().at(column, row) else {
                continue;
            };
            let position = Position { column, row };
            let field_row = (ROWS - row) as f32;
            let mut y = field_row;
            if let Some((dropped, at)) = game.last_drop()
                && dropped == position
            {
                let progress = (((now - at) as f32) / FALL_SECONDS).clamp(0.0, 1.0);
                if progress < 1.0 {
                    animating = true;
                }
                y = field_row * progress * progress;
            }
            let is_winning = winning.contains(&position);
            let size = cell * DISC_RATIO * if is_winning { pulse } else { 1.0 };
            let opacity = if !winning.is_empty() && !is_winning {
                0.4
            } else {
                1.0
            };
            let center = geometry.center(column as f32, y);
            paint_disc(&painter, center, size, player, opacity);
            if is_winning {
                let ring = (size * 0.12).max(1.5);
                painter.circle_stroke(
                    center,
                    size / 2.0 - ring / 2.0,
                    Stroke::new(ring, Color32::WHITE),
                );
            }
        }
    }

    paint_frame(&painter, geometry, ui.ctx().pixels_per_point());

    BoardInput {
        hovered_column,
        clicked_column,
        animating,
    }
}

pub fn paint_disc(painter: &Painter, center: Pos2, size: f32, player: Player, opacity: f32) {
    painter.circle_filled(
        center,
        size / 2.0,
        disc_color(player).gamma_multiply(opacity),
    );
    if player == Player::Yellow {
        let ring = (size * 0.08).max(1.0);
        let radius = size / 2.0 - size * 0.2 - ring / 2.0;
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(ring, Color32::from_black_alpha(77).gamma_multiply(opacity)),
        );
    }
}

#[derive(Clone, Copy)]
struct Geometry {
    origin: Pos2,
    cell: f32,
}

impl Geometry {
    fn center(self, column: f32, field_row: f32) -> Pos2 {
        self.origin + vec2((column + 0.5) * self.cell, (field_row + 0.5) * self.cell)
    }
}

fn paint_frame(painter: &Painter, geometry: Geometry, pixels_per_point: f32) {
    let cell = geometry.cell;
    let frame = Rect::from_min_size(
        geometry.origin + vec2(0.0, cell),
        vec2(cell * COLUMNS as f32, cell * ROWS as f32),
    );
    let corner = cell * CORNER_RATIO;
    let feather = 1.0 / pixels_per_point;
    let hole = cell * HOLE_RATIO / 2.0;
    let clear = Color32::TRANSPARENT;
    let mut mesh = Mesh::default();

    for column in 0..COLUMNS {
        for row in 0..ROWS {
            let center = frame.min + vec2((column as f32 + 0.5) * cell, (row as f32 + 0.5) * cell);
            let base = mesh.vertices.len() as u32;
            for segment in 0..HOLE_SEGMENTS {
                let angle = segment as f32 / HOLE_SEGMENTS as f32 * TAU;
                let direction = vec2(angle.cos(), angle.sin());
                mesh.colored_vertex(center + direction * (hole - feather), clear);
                mesh.colored_vertex(center + direction * hole, BOARD_BLUE);
                mesh.colored_vertex(
                    edge_point(center, direction, cell, frame, corner),
                    BOARD_BLUE,
                );
            }
            let count = HOLE_SEGMENTS as u32;
            for segment in 0..count {
                let here = base + segment * 3;
                let next = base + ((segment + 1) % count) * 3;
                for ring in 0..2 {
                    mesh.add_triangle(here + ring, next + ring, here + ring + 1);
                    mesh.add_triangle(next + ring, next + ring + 1, here + ring + 1);
                }
            }
        }
    }
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(
        frame,
        corner,
        Stroke::new(feather, BOARD_BLUE),
        StrokeKind::Inside,
    );
}

fn edge_point(center: Pos2, direction: Vec2, cell: f32, frame: Rect, corner: f32) -> Pos2 {
    let half = cell / 2.0;
    let reach = half / direction.x.abs().max(direction.y.abs());
    let point = center + direction * reach;
    let rounded = |value: f32, low: f32, high: f32| {
        if value <= low + corner {
            Some(low + corner)
        } else if value >= high - corner {
            Some(high - corner)
        } else {
            None
        }
    };
    let (Some(x), Some(y)) = (
        rounded(point.x, frame.min.x, frame.max.x),
        rounded(point.y, frame.min.y, frame.max.y),
    ) else {
        return point;
    };
    let corner_center = pos2(x, y);
    let offset = center - corner_center;
    let along = offset.dot(direction);
    let reach = -along
        + (along * along - offset.length_sq() + corner * corner)
            .max(0.0)
            .sqrt();
    center + direction * reach
}
