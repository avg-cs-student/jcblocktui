use anyhow::Result;
use crossterm::event::KeyEvent;

pub trait TuiGame {
    /// Respond to events (key presses, etc).
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()>;

    /// Reset the game state.
    fn reset(&mut self);
}
