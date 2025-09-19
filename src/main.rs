use anyhow::Result;
use jcblocktui::app::App;

fn main() -> Result<()> {
    let terminal = ratatui::init();
    let result = App::new()?.run(terminal);
    ratatui::restore();
    result
}
