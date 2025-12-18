use vt100::Parser;

pub struct Emulator {
    parser: Parser,
    rows: u16,
    cols: u16,
}

impl std::fmt::Debug for Emulator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Emulator{{rows:{}, cols:{}}}", self.rows, self.cols)
    }
}

impl Emulator {
    #[must_use]
    pub fn new(rows: u16, cols: u16) -> Self {
        let parser = Parser::new(rows, cols, 0);
        Self { parser, rows, cols }
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows == self.rows && cols == self.cols {
            return;
        }
        self.rows = rows.max(1);
        self.cols = cols.max(1);
        self.parser.set_size(self.rows, self.cols);
    }

    #[must_use]
    pub fn render_lines(&self) -> Vec<String> {
        let screen = self.parser.screen().clone();
        let mut out: Vec<String> = Vec::with_capacity(self.rows as usize);
        for r in 0..self.rows {
            let mut line = String::with_capacity(self.cols as usize);
            for c in 0..self.cols {
                if let Some(cell) = screen.cell(r, c) {
                    let s = cell.contents();
                    if s.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(&s);
                    }
                } else {
                    line.push(' ');
                }
            }
            // Trim trailing spaces for nicer rendering
            let trimmed = line.trim_end().to_string();
            out.push(trimmed);
        }
        out
    }
}
