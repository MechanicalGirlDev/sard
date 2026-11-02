/// Helper for building text content with formatting.
pub struct TextBuilder {
    lines: Vec<String>,
}

impl TextBuilder {
    /// Create a new text builder.
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Add a line of text.
    pub fn line(mut self, text: impl Into<String>) -> Self {
        self.lines.push(text.into());
        self
    }

    /// Add an empty line.
    pub fn blank(mut self) -> Self {
        self.lines.push(String::new());
        self
    }

    /// Add a separator line.
    pub fn separator(mut self, char: char, width: usize) -> Self {
        self.lines.push(char.to_string().repeat(width));
        self
    }

    /// Build the final text string.
    pub fn build(self) -> String {
        self.lines.join("\n")
    }
}

impl Default for TextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
