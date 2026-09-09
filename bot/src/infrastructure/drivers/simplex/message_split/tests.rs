use super::*;

#[test]
fn test_split_lines_simple() {
    let text = "Hello\nWorld\n!";
    let result = split_lines_by_byte_limit(text, 20);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "Hello\nWorld\n!");
}

#[test]
fn test_split_lines_multiple_chunks() {
    let text = "Line 1\nLine 2\nLine 3\nLine 4";
    let result = split_lines_by_byte_limit(text, 15);

    // "Line 1\nLine 2\n" = 14 байт
    // "Line 3\nLine 4\n" = 14 байт
    assert_eq!(result.len(), 2);
}

#[test]
fn test_split_lines_single_long_line() {
    let text = "This is a very long line";
    let result = split_lines_by_byte_limit(text, 10);

    // Даже длинная строка не режется, добавляется целиком
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "This is a very long line");
}

#[test]
fn test_split_lines_cyrillic() {
    // Кириллица - 2 байта на символ в UTF-8
    let text = "Привет\nМир";
    let result = split_lines_by_byte_limit(text, 15);

    // "Привет" = 12 байт (6 символов * 2)
    // "Мир" = 6 байт (3 символа * 2)
    // В одном чанке не влезет
    assert_eq!(result.len(), 2);
}

#[test]
fn test_split_lines_emoji() {
    // Эмодзи - 4 байта на символ в UTF-8
    let text = "😀😀😀\nLine";
    let result = split_lines_by_byte_limit(text, 15);

    // "😀😀😀" = 12 байт (3 эмодзи * 4)
    assert_eq!(result.len(), 2);
}
