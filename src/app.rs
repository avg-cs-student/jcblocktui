use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::event::{KeyCode, KeyEvent};
use jcblocks::{block::Point, game::Game};
use ratatui::{DefaultTerminal, Frame};

use crate::{blast::Blast, game::TuiGame, scoreboard::LocalScoreBoard};

use super::block_index::*;
use super::config::*;

#[derive(Debug)]
pub struct App {
    exit: bool,
    game: Blast,
}

impl App {
    pub fn new() -> Result<Self> {
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

        let exe_path = std::env::current_exe()?;
        let exe_dir = match exe_path.parent() {
            Some(dir) => dir,
            None => bail!("Cannot determine executable directory"),
        };

        // Create database path relative to executable
        let db_path = exe_dir.join("app.db");
        let scoreboard = LocalScoreBoard::new(5, db_path)?;
        let blast = Blast {
            game_over: false,
            game,
            blocks,
            selected: BlockIndex::default(),
            cursor_position: center.clone(),
            center,
            board_width,
            board_height,
            show_conflict_popup: false,
            scoreboard,
        };

        Ok(Self {
            exit: false,
            game: blast,
        })
    }

    fn reset(&mut self) {
        self.game.reset();
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
        frame.render_widget(&self.game, frame.area());
    }

    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)?;
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            // new game
            KeyCode::Enter => {
                if self.game.is_complete() {
                    self.reset();
                }
            }

            // quit
            KeyCode::Char('q') => {
                return {
                    self.exit();
                    Ok(())
                };
            }

            _ => self.game.handle_key_event(key_event)?,
        }

        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}
