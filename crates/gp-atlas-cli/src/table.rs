//! Minimal fixed-width table printing for the terminal.

pub struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

const MAX_CELL: usize = 48;

fn width(s: &str) -> usize {
    s.chars().count()
}

fn clip(s: &str) -> String {
    let one_line = s.replace(['\n', '\r'], " ");
    if width(&one_line) <= MAX_CELL {
        one_line
    } else {
        let mut t: String = one_line.chars().take(MAX_CELL - 1).collect();
        t.push('…');
        t
    }
}

impl Table {
    pub fn new(header: &[&str]) -> Self {
        Self {
            header: header.iter().map(|s| (*s).to_owned()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells.iter().map(|c| clip(c)).collect());
    }

    pub fn print(&self) {
        let n = self.header.len();
        let mut w: Vec<usize> = self.header.iter().map(|h| width(h)).collect();
        for r in &self.rows {
            for (i, c) in r.iter().enumerate().take(n) {
                w[i] = w[i].max(width(c));
            }
        }
        let line = |cells: &[String]| {
            let parts: Vec<String> = (0..n)
                .map(|i| {
                    let c = cells.get(i).map(String::as_str).unwrap_or("");
                    format!("{c}{}", " ".repeat(w[i].saturating_sub(width(c))))
                })
                .collect();
            crate::out!("{}", parts.join("  ").trim_end());
        };
        line(&self.header);
        crate::out!(
            "{}",
            w.iter()
                .map(|x| "─".repeat(*x))
                .collect::<Vec<_>>()
                .join("  ")
        );
        for r in &self.rows {
            line(r);
        }
    }
}
