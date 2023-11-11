use std::io::{BufRead, Write};
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about = "Formats the given input according to specified options", long_about = None)]
struct Cli {
    #[arg(short, long, default_value_t = 0, help = "Break lines that exceed the specified number of characters; specify 0 for no line limit")]
    max_line_length: usize,
    #[arg(short, long, default_value_t = false, help = "Break words when breaking lines via --max-line-length")]
    break_words: bool,
    #[arg(short, long, default_value_t = false, help = "Keep trailing whitespaces")]
    keep_trailing_whitespaces: bool,
    #[arg(short, long, default_value_t = false, help = "Preserve list indentation when breaking lines via --max-line-length (does not fix existing list indentation)")]
    preserve_list_indentation: bool,
    #[arg(short, long, default_value_t = false, help = "Join lines that would otherwise be shorter than the maximum specified via --max-line-length")]
    rewrap: bool,
}

struct LineState<'a> {
    current_char: char,
    output_line: &'a mut String,
    has_last_word_end: bool,
    has_word: bool,
    last_word_end: usize,
    list_indentation: String,
    has_list_indentation: bool,
    list_padding_end: bool,
    is_at_word_boundary: bool,
}

fn write_line(output: &mut dyn Write, line: &String, args: &Cli) {
    if args.keep_trailing_whitespaces {
        write!(output, "{}\n", line).unwrap();
    } else {
        write!(output, "{}\n", line.trim_end()).unwrap();
    }
}

fn is_list_start(c: char) -> bool {
    return c == '*' || c == '-';
}

fn handle_overflow(output: &mut dyn Write, state: &mut LineState, args: &Cli) -> bool {
    // skip if there is no overflow
    if args.max_line_length == 0 || state.output_line.len() < args.max_line_length {
        return false;
    }

    // deal with overflow
    if args.break_words || state.is_at_word_boundary {
        // print the output line we have so far and write further characters into a new/clear output line
        write_line(output, &state.output_line, &args);
        state.output_line.clear();
    } else if state.has_last_word_end {
        // print the output line we have so far but only until the last whitespace; keep further characters
        // the output line for the next line
        let output_line_until_last_whitespace: String = state.output_line.drain(..state.last_word_end + 1).collect();
        write_line(output, &output_line_until_last_whitespace, &args);
    }
    state.has_last_word_end = false;

    // repeat list indentation on the next line if present
    if state.has_list_indentation {
        state.output_line.insert_str(0, state.list_indentation.as_str());
    }

    // continue with next character if the overflow happened at a word-boundary (no need to repeat the whitespace)
    if state.is_at_word_boundary {
        state.has_last_word_end = false;
        return true;
    }

    return false;
}

fn handle_list(state: &mut LineState, args: &Cli) -> bool {
    let list_found = args.preserve_list_indentation && !state.has_word && !state.has_list_indentation && is_list_start(state.current_char);
    if list_found {
        state.has_list_indentation = true;
        state.list_indentation = state.output_line.clone();
        state.list_indentation.push(' ');
        state.list_padding_end = false;
    }
    return list_found;
}

fn handle_word_boundary(state: &mut LineState, _args: &Cli) {
    if state.is_at_word_boundary {
        state.last_word_end = state.output_line.len();
        state.has_last_word_end = true;
    } else {
        state.has_word = true;
    }
}

fn add_list_indentation(state: &mut LineState, list_found: bool, _args: &Cli) {
    if  state.has_list_indentation && !list_found && !state.list_padding_end {
        if state.is_at_word_boundary {
            state.list_indentation.push(state.current_char);
        } else {
            state.list_padding_end = true;
        }
    }
}

fn is_new_paragraph_c(c: char) -> bool {
    return c.is_control() || is_list_start(c);
}

fn is_new_paragraph(s: &String) -> bool {
    for c in s.chars() {
        if c.is_whitespace() {
            continue;
        }
        return is_new_paragraph_c(c);
    }
    return true;
}

fn handle_next_line(output: &mut dyn Write, input_line: String, output_line_: &mut String, args: &Cli) {
    let mut state = LineState{
        current_char: '\0',
        output_line: output_line_,
        has_last_word_end: false,
        has_word: false,
        last_word_end: 0,
        list_indentation: String::new(),
        has_list_indentation: false,
        list_padding_end: true,
        is_at_word_boundary: false,
    };

    // flush previous line in rewrapping mode if the current line is a new paragraph/list-item
    if args.rewrap && !state.output_line.is_empty() && is_new_paragraph(&input_line) {
        write_line(output, &state.output_line, &args);
        state.output_line.clear();
    }

    // insert a whitespace on underflow when rewrapping and trim input
    let mut input_iter = input_line.chars();
    if args.rewrap && !state.output_line.is_empty() {
        state.output_line.push(' ');
        input_iter = input_line.trim_start().chars();
    }

    for c in input_iter {
        state.current_char = c;
        state.is_at_word_boundary = c.is_whitespace();

        // handle the case when the current line is full
        if handle_overflow(output, &mut state, args) {
            continue;
        }

        // take note of lists and word boundaries
        let list_found = handle_list(&mut state, &args);
        handle_word_boundary(&mut state, &args);

        // add the current character to current line
        state.output_line.push(c);

        // add the current character to list indentation
        add_list_indentation(&mut state, list_found, &args);
    }

    // flush current output line
    if !args.rewrap {
        write_line(output, &state.output_line, &args);
        state.output_line.clear();
    }
}

fn read_lines(output: &mut dyn Write, input: &mut dyn BufRead, args: &Cli) {
    // read input line-by-line and echo a formatted version of the input
    let mut output_line = String::new();
    for line in input.lines() {
        handle_next_line(output, line.unwrap(), &mut output_line, &args);
    }

    // print the last output line
    if args.rewrap {
        write_line(output, &output_line, &args);
    }
}

pub fn run(output: &mut dyn Write, input: &mut dyn BufRead) {
    let args = Cli::parse();
    read_lines(output, input, &args);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Cursor, Seek, SeekFrom};

    fn test_read_lines(expected: &[u8], input_data: &[u8], args: &Cli) {
        let mut input = Cursor::new(Vec::new());
        let mut output = Cursor::new(Vec::new());

        // write some test data
        input.write_all(&input_data).unwrap();
        input.seek(SeekFrom::Start(0)).unwrap();

        // read the test data
        read_lines(&mut output, &mut input, &args);

        // check the output
        let mut result = Vec::new();
        output.seek(SeekFrom::Start(0)).unwrap();
        output.read_to_end(&mut result).unwrap();
        assert_eq!(String::from_utf8(expected.to_vec()).unwrap(), String::from_utf8(result).unwrap());
    }

    #[test]
    fn test_simple_one_liner() {
        test_read_lines(b"foo\n", b"foo\n", &Cli{ max_line_length: 0, break_words: true, keep_trailing_whitespaces: true, preserve_list_indentation: false, rewrap: false });
    }

    #[test]
    fn test_line_wrapping_with_word_breaks() {
        test_read_lines(b"foo bar ba\nz\n", b"foo bar baz\n", &Cli{ max_line_length: 10, break_words: true, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false });
        test_read_lines(b"foo bar ba\nz\n", b"foo bar baz\n", &Cli{ max_line_length: 10, break_words: true, keep_trailing_whitespaces: true, preserve_list_indentation: false, rewrap: false });
        test_read_lines(b"fo\no\nba\nr\nba\nz\n", b"foo bar baz\n", &Cli{ max_line_length: 2, break_words: true, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false });
        test_read_lines(b"fo\no \nba\nr \nba\nz\n", b"foo bar baz\n", &Cli{ max_line_length: 2, break_words: true, keep_trailing_whitespaces: true, preserve_list_indentation: false, rewrap: false });
        test_read_lines(b"fooba\nr\nbaz\n", b"foobar\nbaz\n", &Cli{ max_line_length: 5, break_words: true, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false });
    }

    #[test]
    fn test_line_wrapping_without_work_breaks() {
        test_read_lines(b"foo bar\nbaz\n", b"foo bar baz\n", &Cli{ max_line_length: 10, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false });
        test_read_lines(b"foo bar \nbaz\n", b"foo bar baz\n", &Cli{ max_line_length: 10, break_words: false, keep_trailing_whitespaces: true, preserve_list_indentation: false, rewrap: false });
        test_read_lines(b"foo\nbar\nbaz\n", b"foo bar baz\n", &Cli{ max_line_length: 2, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false });
        test_read_lines(b"foobar\nbaz\nt1 t2\n", b"foobar\nbaz t1 t2\n", &Cli{ max_line_length: 5, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false });
    }

    #[test]
    fn test_list_handling_without_preserving_indentation() {
        test_read_lines(b"A list\nfollows:\n* foo bar baz\n* test1 test2\ntest3 test4\n", b"A list follows:\n* foo bar baz\n* test1 test2 test3 test4\n", &Cli{ max_line_length: 14, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false });
        test_read_lines(b"A list\nfollows:\n* foo bar baz\n* test1 test2\ntest3 test4\n", b"A list follows:\n* foo bar baz\n* test1 test2 test3 test4\n", &Cli{ max_line_length: 13, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false });
    }

    #[test]
    fn test_list_handling_with_preserving_indentation() {
        test_read_lines(b"A list\nfollows:\n* foo bar baz\n* test1 test2\n  test3 test4\n", b"A list follows:\n* foo bar baz\n* test1 test2 test3 test4\n", &Cli{ max_line_length: 13, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: false });
        test_read_lines(b"A list\nfollows:\n* foo bar baz\n  * test1\n    test2\n    test3\n    test4\n", b"A list follows:\n* foo bar baz\n  * test1 test2 test3 test4\n", &Cli{ max_line_length: 13, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: false });
        test_read_lines(b"A list follows:\n* foo bar baz\n  * test1 test2\n    test3 test4\n", b"A list follows:\n* foo bar baz\n  * test1 test2 test3 test4\n", &Cli{ max_line_length: 15, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: false });
    }

    #[test]
    fn test_rewrapping() {
        test_read_lines(b"A list follows:\n* foo bar baz\n  * test1 test2\n    test3 test4\n", b"A list follows:\n* foo bar baz\n  * test1 test2 test3 test4\n", &Cli{ max_line_length: 15, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: true });
        test_read_lines(b"A list follows:\n* foo bar baz\n  * test1 test2\n    test3 test4\n", b"A list\nfollows:\n* foo\n  bar baz\n  * test1 test2 test3 test4\n", &Cli{ max_line_length: 15, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: true });
        test_read_lines(b"A list follows:\n* foo bar baz\n  * test1 test2 test3 test4\n", b"A list\nfollows:\n* foo\n  bar baz\n  * test1 test2 test3 test4\n", &Cli{ max_line_length: 0, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: true });
    }
}
