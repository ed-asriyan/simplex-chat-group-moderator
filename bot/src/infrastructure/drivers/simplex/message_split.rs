/// Split `text` into chunks that each stay within `byte_limit` (UTF-8 bytes).
///
/// The text is only ever cut on line boundaries (`'\n'`), never inside a line,
/// so a single line longer than `byte_limit` is emitted whole in its own chunk.
pub fn split_lines_by_byte_limit(text: &str, byte_limit: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut current_size = 0usize;

    for line in text.split('\n') {
        let line_size_with_newline = line.len() + 1;

        if current_size + line_size_with_newline > byte_limit && !current.is_empty() {
            chunks.push(current.join("\n"));
            current.clear();
            current_size = 0;
        }

        current.push(line);
        current_size += line_size_with_newline;
    }

    if !current.is_empty() {
        chunks.push(current.join("\n"));
    }

    chunks
}

#[cfg(test)]
mod tests;
