pub mod line_column {
    use std::cell::RefCell;
    use std::env;

    thread_local! {
        static LINE_COLUMN: RefCell<(Option<isize>, Option<isize>)> =
        const { RefCell::new((None, None)) };
    }

    /// Populate from environment variables:
    /// LINE_NUMBER and COLUMN_NUMBER
    pub fn init_from_env() {
        let line = env::var("LINE_NUMBER")
            .ok()
            .and_then(|v| v.parse::<isize>().ok());

        let column = env::var("COLUMN_NUMBER")
            .ok()
            .and_then(|v| v.parse::<isize>().ok());

        LINE_COLUMN.with(|lc| {
            *lc.borrow_mut() = (line, column);
        });
    }

    /// Get the current (line, column)
    pub fn get() -> (Option<isize>, Option<isize>) {
        LINE_COLUMN.with(|lc| *lc.borrow())
    }

    // /// Set the current (line, column)
    // pub fn set(
    //     line: Option<isize>,
    //     column: Option<isize>,
    // ) {
    //     LINE_COLUMN.with(|lc| {
    //         *lc.borrow_mut() = (line, column);
    //     });
    // }

    // /// Clear both values
    // pub fn clear() {
    //     LINE_COLUMN.with(|lc| {
    //         *lc.borrow_mut() = (None, None);
    //     });
    // }

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
