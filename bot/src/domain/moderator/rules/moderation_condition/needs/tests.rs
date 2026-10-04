use super::*;

#[test]
fn test_a_zero_window_or_count_needs_nothing() {
    assert_eq!(Needs::author_messages(0), Needs::default());
    assert_eq!(Needs::author_lines(0, 40), Needs::default());
    assert_eq!(Needs::history(0), Needs::default());
}

#[test]
fn test_merge_keeps_the_longest_window_of_each_counter_apart() {
    let needs = Needs::author_messages(7)
        .merge(Needs::author_messages(30))
        .merge(Needs::author_characters(5));
    assert_eq!(needs.author_messages, Some(30));
    // The counters are separate logs: neither window answers for the other.
    assert_eq!(needs.author_characters, Some(5));
    assert_eq!(needs.group_messages, None);
}

#[test]
fn test_merge_takes_the_widest_wrap_width_with_zero_widest_of_all() {
    let wide = Needs::author_lines(7, 40).merge(Needs::author_lines(30, 80));
    assert_eq!(
        wide.author_lines,
        Some(LineWindow {
            time_window_minutes: 30,
            chars_per_line: 80
        })
    );
    let unwrapped = wide.merge(Needs::author_lines(5, 0));
    assert_eq!(unwrapped.author_lines.map(|l| l.chars_per_line), Some(0));
}

#[test]
fn test_merge_keeps_the_largest_history() {
    assert_eq!(Needs::history(2).merge(Needs::history(5)).history, Some(5));
}
