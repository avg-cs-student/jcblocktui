//! An untimed game mode where the player must attempt to place randomly
//! generated blocks onto the playing board.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use jcblocks::{
    block::{self, Point},
    canvas::PointStatus,
    game::Game,
};
use ratatui::widgets::Widget;
use ratatui::{
    layout::{Constraint, Flex, Layout},
    prelude::*,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, BorderType, Clear, Paragraph},
};

use crate::{
    block_index::{BlockIndex, DisplayPointStatus},
    config::{BLOCK_REPRESENTATION, EMPTY_BLOCK_REPRESENTATION, NUM_BLOCKS_PER_TURN},
    game::TuiGame,
    scoreboard::{LocalScoreBoard, Scoreboard},
};

/// Game state for the Blast game variant.
#[derive(Debug)]
pub struct Blast {
    pub game_over: bool,
    pub game: Game,
    pub blocks: Vec<block::Block>,
    pub selected: BlockIndex,
    pub cursor_position: Point,
    pub center: Point,
    pub board_width: i32,
    pub board_height: i32,
    pub show_conflict_popup: bool,
    pub scoreboard: LocalScoreBoard,
}

impl Blast {
    pub fn is_complete(&self) -> bool {
        self.game_over
    }

    fn render_local_scoreboard(&self, area: Rect, buf: &mut Buffer) {
        let content = self
            .scoreboard
            .all()
            .iter()
            .take(3)
            .map(|high_score| {
                format!(
                    "{:<6} {:7}",
                    &high_score.when.to_string()[..7],
                    high_score.score
                )
            })
            .collect::<Vec<String>>()
            .join("\n");

        Paragraph::new(Text::from(format!("Personal Best:\n{}", content)))
            .yellow()
            .centered()
            .render(area, buf);
    }

    // presently unused
    fn _render_global_scoreboard(&self, area: Rect, buf: &mut Buffer) {
        let content = self
            .scoreboard
            .all()
            .iter()
            .take(3)
            .map(|high_score| {
                format!(
                    "{:<6} {:7} {:>10}",
                    &high_score.when.to_string()[..7],
                    high_score.score,
                    high_score.name
                )
            })
            .collect::<Vec<String>>()
            .join("\n");

        Paragraph::new(Text::from(format!("World Best:\n{}", content)))
            .yellow()
            .centered()
            .render(area, buf);
    }

    fn render_game_board(&self, area: Rect, buf: &mut Buffer) {
        // Gameboard layout
        let [game_container] =
            Layout::horizontal([Constraint::Length((self.board_width * 2) as u16)])
                .flex(ratatui::layout::Flex::Center)
                .areas(area);

        let game_rows = Layout::vertical(vec![Constraint::default(); self.board_height as usize])
            .flex(ratatui::layout::Flex::Center)
            .split(game_container);

        // Get the current state of each coordinate within the playing area.
        let mut display_coords: Vec<DisplayPointStatus> = self
            .game
            .canvas
            .contents()
            .iter()
            .map(|p| {
                if let PointStatus::Occupied = p {
                    return DisplayPointStatus::Occupied;
                }
                DisplayPointStatus::Unoccupied
            })
            .collect();

        // Overlay the currently selected block, taking into account the user's cursor position.
        let mut has_conflicts = false;
        for p in self.blocks[self.selected.current()].coordinates() {
            let index = ((p.y + self.cursor_position.y) * self.board_width
                + (p.x + self.cursor_position.x)) as usize;

            display_coords[index] = match display_coords[index] {
                DisplayPointStatus::Occupied => {
                    has_conflicts = true;
                    DisplayPointStatus::Hovered { has_conflict: true }
                }
                DisplayPointStatus::Unoccupied => DisplayPointStatus::Hovered {
                    has_conflict: false,
                },
                _ => panic!("Unreachable."),
            }
        }

        // If there are no conflicts, show any lines that would be blasted if the block were placed.
        if !has_conflicts {
            'row_loop: for row in 0..self.board_height {
                for column in 0..self.board_width {
                    let index = (row * self.board_width + column) as usize;
                    if let DisplayPointStatus::Unoccupied
                    | DisplayPointStatus::Hovered { has_conflict: true } = display_coords[index]
                    {
                        continue 'row_loop;
                    }
                }

                for column in 0..self.board_width {
                    let index = (row * self.board_width + column) as usize;
                    display_coords[index] = DisplayPointStatus::Blast;
                }
            }

            'column_loop: for column in 0..self.board_width {
                for row in 0..self.board_height {
                    let index = (row * self.board_width + column) as usize;
                    if let DisplayPointStatus::Unoccupied
                    | DisplayPointStatus::Hovered { has_conflict: true } = display_coords[index]
                    {
                        continue 'column_loop;
                    }
                }

                for row in 0..self.board_height {
                    let index = (row * self.board_width + column) as usize;
                    display_coords[index] = DisplayPointStatus::Blast;
                }
            }
        }

        // Render the game board.
        for (i, row) in game_rows.iter().rev().enumerate() {
            let game_cols =
                Layout::horizontal(vec![Constraint::default(); self.board_width as usize])
                    .vertical_margin(0)
                    .split(*row);

            for (j, col) in game_cols.iter().enumerate() {
                let repr = match display_coords[i * self.board_width as usize + j] {
                    DisplayPointStatus::Blast => Text::from(BLOCK_REPRESENTATION).yellow(),
                    DisplayPointStatus::Occupied => Text::from(BLOCK_REPRESENTATION).blue(),
                    DisplayPointStatus::Unoccupied => {
                        Text::from(EMPTY_BLOCK_REPRESENTATION).dark_gray()
                    }
                    DisplayPointStatus::Hovered {
                        has_conflict: false,
                    } => Text::from(BLOCK_REPRESENTATION).magenta(),
                    DisplayPointStatus::Hovered { has_conflict: true } => Text::from("◎").red(),
                };

                // FIXME: game over screen isnt my favorite.
                Paragraph::new({
                    if self.game_over { repr.gray() } else { repr }
                })
                .centered()
                .render(*col, buf);
            }
        }
    }

    fn render_block_selector(&self, area: Rect, buf: &mut Buffer) {
        // remaining blocks view
        let block_areas = Layout::horizontal([
            Constraint::Percentage(23), // spacing
            Constraint::Percentage(18),
            Constraint::Percentage(18),
            Constraint::Percentage(18),
            Constraint::Percentage(23), // spacing
        ])
        .flex(Flex::Center)
        .split(area);

        // account for spacing
        let offset = 1;
        for (i, b) in self.blocks.iter().enumerate() {
            let mut view = Text::from(format!("{}", b));

            // add a border to the selected block
            view = if i == self.selected.current() {
                view.magenta()
            } else {
                view.black()
            };

            if i == self.selected.current() {
                Paragraph::new(view)
                    .style(Style::default().add_modifier(Modifier::SLOW_BLINK))
                    .centered()
                    .render(block_areas[i + offset], buf);
            } else {
                Paragraph::new(view)
                    .centered()
                    .render(block_areas[i + offset], buf);
            }
        }
    }
}

impl TuiGame for Blast {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        // moving a block could result in part of it escaping the playing board, this helper is for
        // checking that condition
        let is_selected_block_within_boundary = |cursor: &Point| {
            for p in self.blocks[self.selected.current()].coordinates() {
                if p.x + cursor.x >= self.board_width || p.x + cursor.x < 0 {
                    return false;
                }
                if p.y + cursor.y >= self.board_height || p.y + cursor.y < 0 {
                    return false;
                }
            }

            true
        };

        self.show_conflict_popup = false;
        match key_event.code {
            // place block
            KeyCode::Char(' ') => {
                let Point { y: row, x: column } = self.cursor_position;

                // attempt to place the block
                if self
                    .game
                    .maybe_place_block(&self.blocks[self.selected.current()], row, column)
                    .is_ok()
                {
                    // yes, remove is highly inefficient, but this vector is always very tiny,
                    // so bite me.
                    self.blocks.remove(self.selected.place());
                    if self.blocks.is_empty() {
                        match self.game.generate_blocks(NUM_BLOCKS_PER_TURN) {
                            Some(blocks) => self.blocks = blocks,
                            None => unreachable!("There is always a combination that will work."),
                        }
                    }

                    // check if the game can make progress.
                    let mut can_fit_at_least_one = false;
                    for block in self.blocks.iter() {
                        if self.game.canvas.can_fit(block).is_some() {
                            can_fit_at_least_one = true;
                            break;
                        }
                    }
                    self.game_over = !can_fit_at_least_one;
                    if self.game_over {
                        self.scoreboard.add(env!("USER"), self.game.score as i64)?;
                    }
                    self.cursor_position = self.center.clone();
                } else {
                    self.show_conflict_popup = true;
                }

                Ok(())
            }

            // cursor left
            KeyCode::Char('h') | KeyCode::Left => {
                if self.game_over {
                    return Ok(());
                }

                let maybe_new_cursor_position = Point {
                    x: self.cursor_position.x - 1,
                    y: self.cursor_position.y,
                };
                if is_selected_block_within_boundary(&maybe_new_cursor_position) {
                    self.cursor_position = maybe_new_cursor_position;
                }

                Ok(())
            }

            // cursor down
            KeyCode::Char('j') | KeyCode::Down => {
                if self.game_over {
                    return Ok(());
                }

                let maybe_new_cursor_position = Point {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y - 1,
                };
                if is_selected_block_within_boundary(&maybe_new_cursor_position) {
                    self.cursor_position = maybe_new_cursor_position;
                }

                Ok(())
            }

            // cursor up
            KeyCode::Char('k') | KeyCode::Up => {
                if self.game_over {
                    return Ok(());
                }

                let maybe_new_cursor_position = Point {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y + 1,
                };
                if is_selected_block_within_boundary(&maybe_new_cursor_position) {
                    self.cursor_position = maybe_new_cursor_position;
                }

                Ok(())
            }

            // cursor right
            KeyCode::Char('l') | KeyCode::Right => {
                if self.game_over {
                    return Ok(());
                }

                let maybe_new_cursor_position = Point {
                    x: self.cursor_position.x + 1,
                    y: self.cursor_position.y,
                };
                if is_selected_block_within_boundary(&maybe_new_cursor_position) {
                    self.cursor_position = maybe_new_cursor_position;
                }

                Ok(())
            }

            // cycle block selection
            KeyCode::Char('n') => {
                if self.game_over {
                    return Ok(());
                }

                self.selected.cycle();
                self.cursor_position = self.center.clone();

                Ok(())
            }

            _ => Ok(()),
        }
    }

    fn reset(&mut self) {
        self.game.reset();
        self.game_over = false;
        self.blocks = self
            .game
            .generate_blocks(NUM_BLOCKS_PER_TURN)
            .expect("Should be able to generate blocks for an empty canvas.");
        self.selected = BlockIndex::default();
    }
}

impl Widget for &Blast {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let debug_area_constraint = Constraint::Percentage(56);
        let block_selector_constraint = Constraint::Percentage(24);
        let scoreboard_constraint = Constraint::Percentage(10);
        let vspace_constraint = Constraint::Percentage(10);
        let game_board_constraint = Constraint::Min(self.board_height as u16);

        // split the screen in horizontal slices
        let top_to_bot_view_areas = Layout::vertical([
            scoreboard_constraint,
            vspace_constraint,
            game_board_constraint,
            vspace_constraint,
            block_selector_constraint,
            debug_area_constraint,
        ])
        .vertical_margin(5)
        .flex(Flex::Center)
        .split(area);

        let [local_scoreboard_area, _, _global_scoreboard_area] = Layout::horizontal([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .horizontal_margin(5)
        .areas(top_to_bot_view_areas[0]);

        self.render_local_scoreboard(local_scoreboard_area, buf);
        // todo
        // self.render_global_scoreboard(global_scoreboard_area, buf);
        self.render_game_board(top_to_bot_view_areas[2], buf);
        self.render_block_selector(top_to_bot_view_areas[4], buf);

        // Warn the user when attempting invalid block placement
        if self.show_conflict_popup {
            Clear.render(top_to_bot_view_areas[1], buf);
            let conflict_inner = Text::from("It doesn't fit!").red();
            let conflict_outer = Paragraph::new(conflict_inner).centered();
            let popup_area = create_popup_area(area, 60, 80);
            conflict_outer.render(popup_area, buf);
        }

        // Game Over - clear everything except the game board.
        if self.game_over {
            Clear.render(top_to_bot_view_areas[0], buf);
            Clear.render(top_to_bot_view_areas[3], buf);
            Clear.render(top_to_bot_view_areas[4], buf);
            Clear.render(top_to_bot_view_areas[5], buf);

            let game_over_str = Text::from("GAME OVER".to_string()).red();
            Paragraph::new(game_over_str)
                .centered()
                .render(top_to_bot_view_areas[1], buf);

            let help_txt = Text::from("Press ENTER to play again.".to_string()).blue();
            Paragraph::new(help_txt)
                .centered()
                .render(top_to_bot_view_areas[5], buf);
        }

        let title = Line::from(" Block TUI ".bold());
        let score = Line::from(format!(" Current Score: {} ", self.game.score).bold());
        let instructions = Line::from(vec![
            " Quit ".into(),
            "<q> ".blue().bold(),
            " Movement ".into(),
            "<h,j,k,l> ".blue().bold(),
            " Cycle Block Selection ".into(),
            "<n> ".blue().bold(),
            " Place Block ".into(),
            "<Space> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .title(title.left_aligned())
            .title(score.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK)
            .border_type(BorderType::Rounded);
        Paragraph::default().block(block).render(area, buf);
    }
}

fn create_popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
