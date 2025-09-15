use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use jcblocks::{
    block::{self, Point},
    canvas::PointStatus,
    game::Game,
};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, BorderType, Clear, Paragraph, Widget},
};

use super::block_index::*;
use super::config::*;

#[derive(Debug)]
pub struct App {
    exit: bool,
    game_over: bool,
    game: Game,
    blocks: Vec<block::Block>,
    selected: BlockIndex,
    cursor_position: Point,
    center: Point,
    board_width: i32,
    board_height: i32,
}

impl App {
    pub fn new() -> Self {
        let game = Game::default();

        // block coordinates include negative numbers, so having these as i32 just reduces the
        // number of casts we have to do later.
        let board_height = game.canvas.rows as i32;
        let board_width = game.canvas.columns as i32;

        // the player always has one selected block and zero or more additional blocks.
        let blocks = game
            .generate_blocks(NUM_BLOCKS_PER_TURN)
            .expect("Should be able to generate blocks for an empty canvas.");

        // noting the center position is useful as it gives a place to initially place blocks where
        // they are ~guaranteed to fit without wrap
        let center = Point {
            x: board_width / 2 - 1,
            y: board_height / 2 - 1,
        };

        Self {
            exit: false,
            game_over: false,
            game,
            blocks,
            selected: BlockIndex::default(),
            cursor_position: center.clone(),
            center,
            board_width,
            board_height,
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

    /// Run the application's main loop.
    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.exit = false;
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        if self.game_over {
            frame.render_widget(Clear, frame.area());
        }
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
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

        match key_event.code {
            // quit
            KeyCode::Char('q') => self.exit(),

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
                    if self.blocks.len() < 1 {
                        match self.game.generate_blocks(NUM_BLOCKS_PER_TURN) {
                            Some(blocks) => self.blocks = blocks,
                            None => unreachable!("There is always a combination that will work."),
                        }
                    }

                    // check if the game can make progress.
                    let mut can_fit_at_least_one = false;
                    for block in self.blocks.iter() {
                        if self.game.canvas.can_fit(&block).is_some() {
                            can_fit_at_least_one = true;
                            break;
                        }
                    }
                    self.game_over = !can_fit_at_least_one;
                    self.cursor_position = self.center.clone();
                }
            }

            // cursor left
            KeyCode::Char('h') | KeyCode::Left => {
                if self.game_over {
                    return;
                }

                let maybe_new_cursor_position = Point {
                    x: self.cursor_position.x - 1,
                    y: self.cursor_position.y,
                };
                if is_selected_block_within_boundary(&maybe_new_cursor_position) {
                    self.cursor_position = maybe_new_cursor_position;
                }
            }

            // cursor down
            KeyCode::Char('j') | KeyCode::Down => {
                if self.game_over {
                    return;
                }

                let maybe_new_cursor_position = Point {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y - 1,
                };
                if is_selected_block_within_boundary(&maybe_new_cursor_position) {
                    self.cursor_position = maybe_new_cursor_position;
                }
            }

            // cursor up
            KeyCode::Char('k') | KeyCode::Up => {
                if self.game_over {
                    return;
                }

                let maybe_new_cursor_position = Point {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y + 1,
                };
                if is_selected_block_within_boundary(&maybe_new_cursor_position) {
                    self.cursor_position = maybe_new_cursor_position;
                }
            }

            // cursor right
            KeyCode::Char('l') | KeyCode::Right => {
                if self.game_over {
                    return;
                }

                let maybe_new_cursor_position = Point {
                    x: self.cursor_position.x + 1,
                    y: self.cursor_position.y,
                };
                if is_selected_block_within_boundary(&maybe_new_cursor_position) {
                    self.cursor_position = maybe_new_cursor_position;
                }
            }

            // cycle block selection
            KeyCode::Char('n') => {
                if self.game_over {
                    return;
                }

                self.selected.cycle();
                self.cursor_position = self.center.clone();
            }

            KeyCode::Enter => {
                if self.game_over {
                    self.reset();
                }
            }
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Outermost layout
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

        // Main content
        let areas = Layout::vertical([
            // spacing or game over text
            Constraint::Percentage(10),
            // game board
            Constraint::Length(self.game.canvas.rows as u16),
            // spacer
            Constraint::Percentage(0),
            // next blocks
            Constraint::Percentage(15),
            // nominally empty, useful for dumping debug info
            Constraint::Percentage(40),
        ])
        .vertical_margin(5)
        .flex(ratatui::layout::Flex::Center)
        .split(area);

        // Gameboard layout
        let [game_container] =
            Layout::horizontal([Constraint::Length((self.board_width * 2) as u16)])
                .flex(ratatui::layout::Flex::Center)
                .areas(areas[1]);

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
                    DisplayPointStatus::Hovered { has_conflict: true } => {
                        Text::from(CONFLICT_REPRESENTATION).red()
                    }
                };

                // FIXME: game over screen isnt my favorite.
                Paragraph::new(|| -> Text<'_> {
                    if self.game_over { repr.gray() } else { repr }
                }())
                .centered()
                .render(*col, buf);
            }
        }

        // remaining blocks view
        let block_areas = Layout::horizontal([
            Constraint::Percentage(23), // spacing
            Constraint::Percentage(18),
            Constraint::Percentage(18),
            Constraint::Percentage(18),
            Constraint::Percentage(23), // spacing
        ])
        .flex(ratatui::layout::Flex::Center)
        .split(areas[4]);

        // account for spacing
        let offset = 1;
        for (i, b) in self.blocks.iter().enumerate() {
            let mut view = Text::from(format!("{}", b));

            // add a border to the selected block
            view = if i == self.selected.current() {
                view.magenta()
            } else {
                view.rapid_blink().black()
            };
            Paragraph::new(view)
                .centered()
                .render(block_areas[i + offset], buf);
        }

        // Game Over
        if self.game_over {
            Clear.render(areas[0], buf);
            Clear.render(areas[2], buf);
            Clear.render(areas[3], buf);
            Clear.render(areas[4], buf);

            let game_over_str = Text::from(format!("{}", "GAME OVER")).red();
            Paragraph::new(game_over_str)
                .centered()
                .render(areas[0], buf);

            let help_txt = Text::from(format!("{}", "Press ENTER to play again.")).blue();
            Paragraph::new(help_txt).centered().render(areas[4], buf);
        }
    }
}
