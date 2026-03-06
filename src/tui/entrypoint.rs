use crate::tui::app::App;
use anyhow::Result;

/// Application entry point
///
/// Initializes the terminal, creates the application instance, runs the main loop,
/// and properly restores the terminal on exit.
pub async fn run_tui() -> Result<()> {
    // Set the terminal up
    let mut terminal = ratatui::init();

    // Set up the app
    let app = App::new().await?;

    // Create and run the app
    let app_result = app.run(&mut terminal).await;

    // Restore terminal to original state
    ratatui::restore();

    app_result
}
