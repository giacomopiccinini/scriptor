use crate::configs::settings::{SettingsField, SettingsState};
use crate::configs::theme::ThemeConfig;
use crate::tui::ui::components::overlay_window::OverlayWindow;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph, Widget};

pub struct SettingsScreen;

impl SettingsScreen {
    /// Render the settings screen inside an overlay window.
    /// The overlay sits on top of the main UI, clearing only its own area.
    pub fn render(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        // Build footer hints - navigation on left, commands on right
        let footer_hints_left = Self::build_navigation_hints(theme);
        let footer_hints_right = Self::build_command_hints(theme);

        // Render the overlay window and get the inner content area
        let content_area = OverlayWindow::render(
            footer_hints_left,
            footer_hints_right,
            Some(50),
            Some(70),
            area,
            buf,
            theme,
        );

        // Render the settings content inside the overlay
        Self::render_content(settings_state, content_area, buf, theme);
    }

    /// Build the navigation hints for the bottom left
    fn build_navigation_hints(theme: &ThemeConfig) -> Line<'static> {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("↑/↓", Style::default().fg(theme.highlight)),
            Span::styled(" switch  ", Style::default().fg(theme.foreground)),
            Span::styled("←/→", Style::default().fg(theme.highlight)),
            Span::styled(" adjust ", Style::default().fg(theme.foreground)),
        ])
    }

    /// Build the command hints for the bottom right
    fn build_command_hints(theme: &ThemeConfig) -> Line<'static> {
        Line::from(vec![
            Span::styled("[s]", Style::default().fg(theme.highlight)),
            Span::styled("ave ", Style::default().fg(theme.foreground)),
            Span::styled("[d]", Style::default().fg(theme.highlight)),
            Span::styled("iscard ", Style::default().fg(theme.foreground)),
            Span::styled("[S]", Style::default().fg(theme.highlight)),
            Span::styled("ave as default ", Style::default().fg(theme.foreground)),
        ])
    }

    /// Render the settings content (header + fields) inside the given area
    fn render_content(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        // Calculate layout: header + 4 fields
        let layout = Layout::vertical([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Input Device field
            Constraint::Length(3), // VAD Threshold field
            Constraint::Length(3), // Min Fragmentum Duration field
            Constraint::Length(3), // Max Fragmentum Duration field
            Constraint::Length(3), // Pause Threshold field
            Constraint::Length(3), // STT Model field
            Constraint::Length(3), // VAD Model field
            Constraint::Min(1),    // Spacing
        ]);
        let [
            header_area,
            device_area,
            vad_threshold_area,
            min_fragmentum_duration_area,
            max_fragmentum_duration_area,
            pause_threshold_area,
            stt_model_area,
            vad_model_area,
            _,
        ] = layout.areas(area);

        // Render header
        Self::render_header(header_area, buf, theme);

        // Render device selector
        Self::render_device_field(
            settings_state,
            device_area,
            buf,
            theme,
            settings_state.active_field == SettingsField::InputDevice,
        );

        // Render VAD threshold slider
        Self::render_vad_threshold_field(
            settings_state,
            vad_threshold_area,
            buf,
            theme,
            settings_state.active_field == SettingsField::VadThreshold,
        );

        // Render min fragmentum duration slider
        Self::render_min_fragmentum_duration_field(
            settings_state,
            min_fragmentum_duration_area,
            buf,
            theme,
            settings_state.active_field == SettingsField::MinFragmentumDurationSeconds,
        );

        // Render max fragmentum duration slider
        Self::render_max_fragmentum_duration_field(
            settings_state,
            max_fragmentum_duration_area,
            buf,
            theme,
            settings_state.active_field == SettingsField::MaxFragmentumDurationSeconds,
        );

        // Render pause threshold slider
        Self::render_pause_threshold_field(
            settings_state,
            pause_threshold_area,
            buf,
            theme,
            settings_state.active_field == SettingsField::PauseThresholdInChunks,
        );

        // Render STT model selector
        Self::render_stt_model_field(
            settings_state,
            stt_model_area,
            buf,
            theme,
            settings_state.active_field == SettingsField::STTModel,
        );

        // Render VAD model selector
        Self::render_vad_model_field(
            settings_state,
            vad_model_area,
            buf,
            theme,
            settings_state.active_field == SettingsField::VADModel,
        );
    }

    fn render_header(area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        let block = Block::default().padding(Padding::new(0, 0, 1, 0));

        let header_text = Line::from(vec![Span::styled(
            "S E T T I N G S",
            Style::default().fg(theme.highlight),
        )])
        .centered();

        let paragraph = Paragraph::new(header_text)
            .block(block)
            .style(Style::default().bg(theme.background));

        paragraph.render(area, buf);
    }

    fn render_device_field(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
        is_active: bool,
    ) {
        let block = Block::default().padding(Padding::new(2, 2, 1, 0));

        let device_name = settings_state.selected_device_display();

        // When active, swap colors (bg = highlight, fg = background)
        let (base_style, text_style, accent_style) = if is_active {
            (
                Style::default().bg(theme.highlight),
                Style::default().fg(theme.background),
                Style::default().fg(theme.background),
            )
        } else {
            (
                Style::default().bg(theme.background),
                Style::default().fg(theme.foreground),
                Style::default().fg(theme.highlight),
            )
        };

        // Build the selector line: < Device Name >
        let selector_line = Line::from(vec![
            Span::styled("Input Device: ", text_style),
            Span::styled("< ", text_style),
            Span::styled(device_name, accent_style),
            Span::styled(" >", text_style),
        ]);

        let lines = vec![selector_line];

        let paragraph = Paragraph::new(lines).block(block).style(base_style);

        paragraph.render(area, buf);
    }

    fn render_vad_threshold_field(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
        is_active: bool,
    ) {
        let block = Block::default().padding(Padding::new(2, 2, 1, 0));

        // Build the gauge visualization
        let gauge_width = 20;
        let threshold = settings_state.vad_threshold;
        let filled = ((threshold * gauge_width as f32).round() as usize).min(gauge_width);
        let empty = gauge_width - filled;

        // Create gauge string: [████.......]
        let gauge_filled: String = "█".repeat(filled.saturating_sub(1));
        let gauge_pointer = if filled > 0 { "█" } else { "" };
        let gauge_empty: String = ".".repeat(empty);

        // When active, swap colors (bg = highlight, fg = background)
        let (base_style, text_style, accent_style) = if is_active {
            (
                Style::default().bg(theme.highlight),
                Style::default().fg(theme.background),
                Style::default().fg(theme.background),
            )
        } else {
            (
                Style::default().bg(theme.background),
                Style::default().fg(theme.foreground),
                Style::default().fg(theme.highlight),
            )
        };

        let gauge_line = Line::from(vec![
            Span::styled("VAD Threshold: ", text_style),
            Span::styled("[", text_style),
            Span::styled(gauge_filled, accent_style),
            Span::styled(gauge_pointer, accent_style),
            Span::styled(gauge_empty, text_style),
            Span::styled("] ", text_style),
            Span::styled(format!("{:.2}", threshold), accent_style),
        ]);

        let lines = vec![gauge_line];

        let paragraph = Paragraph::new(lines).block(block).style(base_style);

        paragraph.render(area, buf);
    }

    fn render_min_fragmentum_duration_field(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
        is_active: bool,
    ) {
        let block = Block::default().padding(Padding::new(2, 2, 1, 0));

        // Build the gauge visualization
        let gauge_width = 20;
        let duration = settings_state.min_fragmentum_duration_seconds;
        let filled = ((duration / settings_state.max_fragmentum_duration_seconds
            * gauge_width as f32)
            .round() as usize)
            .min(gauge_width);
        let empty = gauge_width - filled;

        // Create gauge string: [████.......]
        let gauge_filled: String = "█".repeat(filled.saturating_sub(1));
        let gauge_pointer = if filled > 0 { "█" } else { "" };
        let gauge_empty: String = ".".repeat(empty);

        // When active, swap colors (bg = highlight, fg = background)
        let (base_style, text_style, accent_style) = if is_active {
            (
                Style::default().bg(theme.highlight),
                Style::default().fg(theme.background),
                Style::default().fg(theme.background),
            )
        } else {
            (
                Style::default().bg(theme.background),
                Style::default().fg(theme.foreground),
                Style::default().fg(theme.highlight),
            )
        };

        let gauge_line = Line::from(vec![
            Span::styled("Min Fragmentum Duration (s): ", text_style),
            Span::styled("[", text_style),
            Span::styled(gauge_filled, accent_style),
            Span::styled(gauge_pointer, accent_style),
            Span::styled(gauge_empty, text_style),
            Span::styled("] ", text_style),
            Span::styled(format!("{:.2}", duration), accent_style),
        ]);

        let lines = vec![gauge_line];

        let paragraph = Paragraph::new(lines).block(block).style(base_style);

        paragraph.render(area, buf);
    }

    fn render_max_fragmentum_duration_field(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
        is_active: bool,
    ) {
        let block = Block::default().padding(Padding::new(2, 2, 1, 0));

        // Build the gauge visualization
        let gauge_width = 20;
        let duration = settings_state.max_fragmentum_duration_seconds;
        let filled = ((duration / 60.0 * gauge_width as f32).round() as usize).min(gauge_width);
        let empty = gauge_width - filled;

        // Create gauge string: [████.......]
        let gauge_filled: String = "█".repeat(filled.saturating_sub(1));
        let gauge_pointer = if filled > 0 { "█" } else { "" };
        let gauge_empty: String = ".".repeat(empty);

        // When active, swap colors (bg = highlight, fg = background)
        let (base_style, text_style, accent_style) = if is_active {
            (
                Style::default().bg(theme.highlight),
                Style::default().fg(theme.background),
                Style::default().fg(theme.background),
            )
        } else {
            (
                Style::default().bg(theme.background),
                Style::default().fg(theme.foreground),
                Style::default().fg(theme.highlight),
            )
        };

        let gauge_line = Line::from(vec![
            Span::styled("Max Fragmentum Duration (s): ", text_style),
            Span::styled("[", text_style),
            Span::styled(gauge_filled, accent_style),
            Span::styled(gauge_pointer, accent_style),
            Span::styled(gauge_empty, text_style),
            Span::styled("] ", text_style),
            Span::styled(format!("{:.2}", duration), accent_style),
        ]);

        let lines = vec![gauge_line];

        let paragraph = Paragraph::new(lines).block(block).style(base_style);

        paragraph.render(area, buf);
    }

    fn render_pause_threshold_field(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
        is_active: bool,
    ) {
        let block = Block::default().padding(Padding::new(2, 2, 1, 0));

        // Convert quantitative number (# chunks) to qualitative to improve UX
        let pause_duration_text = match settings_state.pause_threshold_in_chunks {
            16_u32 => "Short".to_string(),
            24_u32 => "Medium".to_string(),
            32_u32 => "Long".to_string(),
            _ => "Error".to_string(),
        };

        // When active, swap colors (bg = highlight, fg = background)
        let (base_style, text_style, accent_style) = if is_active {
            (
                Style::default().bg(theme.highlight),
                Style::default().fg(theme.background),
                Style::default().fg(theme.background),
            )
        } else {
            (
                Style::default().bg(theme.background),
                Style::default().fg(theme.foreground),
                Style::default().fg(theme.highlight),
            )
        };

        // Build the selector line: < Device Name >
        let selector_line = Line::from(vec![
            Span::styled("Pause Duration: ", text_style),
            Span::styled("< ", text_style),
            Span::styled(pause_duration_text, accent_style),
            Span::styled(" >", text_style),
        ]);

        let lines = vec![selector_line];

        let paragraph = Paragraph::new(lines).block(block).style(base_style);

        paragraph.render(area, buf);
        // let block = Block::default().padding(Padding::new(2, 2, 1, 0));

        // // Build the gauge visualization
        // let gauge_width = 20;
        // let duration = settings_state.pause_threshold_in_chunks;
        // let filled = ((duration * gauge_width as u32) as usize).min(gauge_width);
        // let empty = gauge_width - filled;

        // // Create gauge string: [========>--------]
        // let gauge_filled: String = "=".repeat(filled.saturating_sub(1));
        // let gauge_pointer = if filled > 0 { ">" } else { "" };
        // let gauge_empty: String = "-".repeat(empty);

        // // When active, swap colors (bg = highlight, fg = background)
        // let (base_style, text_style, accent_style) = if is_active {
        //     (
        //         Style::default().bg(theme.highlight),
        //         Style::default().fg(theme.background),
        //         Style::default().fg(theme.background),
        //     )
        // } else {
        //     (
        //         Style::default().bg(theme.background),
        //         Style::default().fg(theme.foreground),
        //         Style::default().fg(theme.highlight),
        //     )
        // };

        // let gauge_line = Line::from(vec![
        //     Span::styled("Pause Duration: ", text_style),
        //     Span::styled("[", text_style),
        //     Span::styled(gauge_filled, accent_style),
        //     Span::styled(gauge_pointer, accent_style),
        //     Span::styled(gauge_empty, text_style),
        //     Span::styled("] ", text_style),
        //     Span::styled(format!("{:.2}", duration), accent_style),
        // ]);

        // let lines = vec![gauge_line];

        // let paragraph = Paragraph::new(lines).block(block).style(base_style);

        // paragraph.render(area, buf);
    }

    fn render_stt_model_field(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
        is_active: bool,
    ) {
        let block = Block::default().padding(Padding::new(2, 2, 1, 0));

        let model_name = settings_state.selected_stt_model_display();

        // When active, swap colors (bg = highlight, fg = background)
        let (base_style, text_style, accent_style) = if is_active {
            (
                Style::default().bg(theme.highlight),
                Style::default().fg(theme.background),
                Style::default().fg(theme.background),
            )
        } else {
            (
                Style::default().bg(theme.background),
                Style::default().fg(theme.foreground),
                Style::default().fg(theme.highlight),
            )
        };

        // Build the selector line: < Model Name >
        let selector_line = Line::from(vec![
            Span::styled("STT Model:    ", text_style),
            Span::styled("< ", text_style),
            Span::styled(model_name, accent_style),
            Span::styled(" >", text_style),
        ]);

        let lines = vec![selector_line];

        let paragraph = Paragraph::new(lines).block(block).style(base_style);

        paragraph.render(area, buf);
    }

    fn render_vad_model_field(
        settings_state: &SettingsState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
        is_active: bool,
    ) {
        let block = Block::default().padding(Padding::new(2, 2, 1, 0));

        let model_name = settings_state.selected_vad_model_display();

        // When active, swap colors (bg = highlight, fg = background)
        let (base_style, text_style, accent_style) = if is_active {
            (
                Style::default().bg(theme.highlight),
                Style::default().fg(theme.background),
                Style::default().fg(theme.background),
            )
        } else {
            (
                Style::default().bg(theme.background),
                Style::default().fg(theme.foreground),
                Style::default().fg(theme.highlight),
            )
        };

        // Build the selector line: < Model Name >
        let selector_line = Line::from(vec![
            Span::styled("VAD Model:    ", text_style),
            Span::styled("< ", text_style),
            Span::styled(model_name, accent_style),
            Span::styled(" >", text_style),
        ]);

        let lines = vec![selector_line];

        let paragraph = Paragraph::new(lines).block(block).style(base_style);

        paragraph.render(area, buf);
    }
}
