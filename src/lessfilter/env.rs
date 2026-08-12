pub mod line_column {
    use std::env;
    use std::sync::OnceLock;

    /// Line/column highlight position, populated once from the
    /// HIGHLIGHT_LINE / HIGHLIGHT_COLUMN environment variables.
    pub static LINE_COLUMN: OnceLock<(Option<isize>, Option<isize>)> = OnceLock::new();

    /// Populate from environment variables:
    /// HIGHLIGHT_LINE and HIGHLIGHT_COLUMN
    pub fn init_from_env() {
        let line = env::var("HIGHLIGHT_LINE")
            .ok()
            .and_then(|v| v.parse::<isize>().ok());

        let column = env::var("HIGHLIGHT_COLUMN")
            .ok()
            .and_then(|v| v.parse::<isize>().ok());

        LINE_COLUMN
            .set((line, column))
            .expect("line/column initialized more than once");
    }

    // /// Parse line/column string like "10:3" or "10,3"
    // fn parse_line_column(s: &str) -> Option<(usize, usize)> {
    //     if let Some((l, c)) = s.split_once(':') {
    //         Some((l.parse().ok()?, c.parse().ok()?))
    //     } else if let Some((l, c)) = s.split_once(',') {
    //         Some((l.parse().ok()?, c.parse().ok()?))
    //     } else {
    //         None
    //     }
    // }
}
