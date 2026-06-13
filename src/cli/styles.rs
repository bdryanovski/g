//! Colour palette used for clap's `--help` output.

/// Build the colour palette used for `--help` output.
///
/// Applied via `#[command(styles = get_styles())]` on the top-level `Cli`
/// struct.  Clap 4 calls this function when it first constructs the command
/// object, so the styles are applied consistently to every `--help` page in
/// the hierarchy.
///
/// | Element       | Style                    |
/// |---------------|--------------------------|
/// | Section heads | bold bright-white        |
/// | Usage line    | bold bright-white        |
/// | Literals      | bold bright-cyan         |
/// | Placeholders  | cyan                     |
/// | Errors        | bold bright-red          |
/// | Valid values  | bold bright-green        |
/// | Invalid       | bold bright-yellow       |
pub(super) fn get_styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Style, Styles};
    Styles::styled()
        .header(
            Style::new()
                .fg_color(Some(AnsiColor::BrightWhite.into()))
                .bold(),
        )
        .usage(
            Style::new()
                .fg_color(Some(AnsiColor::BrightWhite.into()))
                .bold(),
        )
        .literal(
            Style::new()
                .fg_color(Some(AnsiColor::BrightCyan.into()))
                .bold(),
        )
        .placeholder(Style::new().fg_color(Some(AnsiColor::Cyan.into())))
        .error(
            Style::new()
                .fg_color(Some(AnsiColor::BrightRed.into()))
                .bold(),
        )
        .valid(
            Style::new()
                .fg_color(Some(AnsiColor::BrightGreen.into()))
                .bold(),
        )
        .invalid(
            Style::new()
                .fg_color(Some(AnsiColor::BrightYellow.into()))
                .bold(),
        )
}
