use std::io::{BufRead, BufReader, Write};
use std::fs::File;
use clap::Parser;
use itertools::{Itertools,EitherOrBoth::*};
use regex::Regex;

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
    #[arg(short, long, help = "Matches each line against the specified regex and substitutes matches with the specified --replacement")]
    substitute_regex: Vec<String>,
    #[arg(long, help = "Replacement for --substitute-regex, see https://docs.rs/regex/latest/regex/struct.Regex.html#replacement-string-syntax")]
    replacement: Vec<String>,
    #[arg(help = "Specifies files to read the input from (instead of stdin)")]
    input_files: Vec<String>,
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
    c == '*' || c == '-'
}

fn flush_output_line(output: &mut dyn Write, state: &mut LineState, args: &Cli) {
    write_line(output, &state.output_line, &args);
    state.output_line.clear();
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

    false
}

fn handle_list(state: &mut LineState, args: &Cli) -> bool {
    let list_found = args.preserve_list_indentation && !state.has_word && !state.has_list_indentation && is_list_start(state.current_char);
    if list_found {
        state.has_list_indentation = true;
        state.list_indentation = state.output_line.clone();
        state.list_indentation.push(' ');
        state.list_padding_end = false;
    }
    list_found
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
    c.is_control() || is_list_start(c)
}

fn is_new_paragraph(s: &String) -> bool {
    for c in s.chars() {
        if !c.is_whitespace() {
            return is_new_paragraph_c(c);
        }
    }
    true
}

fn handle_next_line<'a>(output: &mut dyn Write, mut input_line: &'a mut String, output_line_: &mut String, args: &Cli, substitute_regex: &Vec<Regex>) {
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
        flush_output_line(output, &mut state, &args);
    }

    // apply substitute_regex
    let substituted_line: &mut String = &mut input_line;
    for pair in substitute_regex.iter().zip_longest(&args.replacement) {
        match pair {
            Both(regex, replacement) => { *substituted_line = String::from(regex.replace(&substituted_line, replacement)); },
            Left(regex) => { *substituted_line = String::from(regex.replace(&substituted_line, "")); },
            Right(_) => {},
        };
    }

    // insert a whitespace on underflow when rewrapping and trim input
    let mut input_iter = substituted_line.chars();
    if args.rewrap && !state.output_line.is_empty() {
        state.output_line.push(' ');
        input_iter = substituted_line.trim_start().chars();
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
        flush_output_line(output, &mut state, &args);
    }
}

fn read_lines<R: BufRead>(output: &mut dyn Write, input: R, output_line: &mut String, args: &Cli, substitute_regex: &Vec<Regex>) {
    for line in input.lines() {
        handle_next_line(output, &mut line.unwrap(), output_line, &args, &substitute_regex);
    }
}

fn read_lines_from_input_or_files(output: &mut dyn Write, input: &mut dyn BufRead, args: &Cli) -> i32 {
    // parse regex for substitution
    let mut substitute_regex = Vec::new();
    for regex in &args.substitute_regex {
        match Regex::new(&regex) {
            Ok(regex) => {
                substitute_regex.push(regex);
            }
            Err(error) => {
                eprintln!("Unable parse specified regex \"{}\": {}", regex, error);
                return 1;
            }
        };
    }

    // read input line-by-line and echo a formatted version of the input
    let mut exit_code: i32 = 0;
    let mut output_line = String::new();
    if args.input_files.is_empty() {
        read_lines(output, input, &mut output_line, &args, &substitute_regex);
    } else {
        for input_file_path in &args.input_files {
            let mut input_file_reader = match File::open(input_file_path) {
                Ok(input_file) => BufReader::new(input_file),
                Err(error) => {
                    eprintln!("Unable to open \"{}\": {}", input_file_path, error);
                    exit_code = 1;
                    continue;
                }
            };
            read_lines(output, &mut input_file_reader, &mut output_line, &args, &substitute_regex);
        }
    }

    // print the last output line
    if args.rewrap {
        write_line(output, &output_line, &args);
    }

    exit_code
}

pub fn run(output: &mut dyn Write, input: &mut dyn BufRead) -> i32 {
    read_lines_from_input_or_files(output, input, &Cli::parse())
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
        read_lines_from_input_or_files(&mut output, &mut input, &args);

        // check the output
        let mut result = Vec::new();
        output.seek(SeekFrom::Start(0)).unwrap();
        output.read_to_end(&mut result).unwrap();
        assert_eq!(String::from_utf8(expected.to_vec()).unwrap(), String::from_utf8(result).unwrap());
    }

    #[test]
    fn test_simple_one_liner() {
        let mk_args = ||
            Cli{ max_line_length: 0, break_words: true, keep_trailing_whitespaces: true, preserve_list_indentation: false, rewrap: false, substitute_regex: Vec::new(), replacement: Vec::new(), input_files: Vec::new() };
        test_read_lines(b"foo\n", b"foo\n", &mk_args());
    }

    #[test]
    fn test_line_wrapping_with_word_breaks() {
        let mk_args = |max_line_length_: usize, keep_trailing_whitespaces_: bool|
            Cli{ max_line_length: max_line_length_, break_words: true, keep_trailing_whitespaces: keep_trailing_whitespaces_, preserve_list_indentation: false, rewrap: false, substitute_regex: Vec::new(), replacement: Vec::new(), input_files: Vec::new() };
        test_read_lines(b"foo bar ba\nz\n", b"foo bar baz\n", &mk_args(10, false));
        test_read_lines(b"foo bar ba\nz\n", b"foo bar baz\n", &mk_args(10, true));
        test_read_lines(b"fo\no\nba\nr\nba\nz\n", b"foo bar baz\n", &mk_args(2, false));
        test_read_lines(b"fo\no \nba\nr \nba\nz\n", b"foo bar baz\n", &mk_args(2, true));
        test_read_lines(b"fooba\nr\nbaz\n", b"foobar\nbaz\n", &mk_args(5, false));
    }

    #[test]
    fn test_line_wrapping_without_work_breaks() {
        let mk_args = |max_line_length_: usize, keep_trailing_whitespaces_: bool|
            Cli{ max_line_length: max_line_length_, break_words: false, keep_trailing_whitespaces: keep_trailing_whitespaces_, preserve_list_indentation: false, rewrap: false, substitute_regex: Vec::new(), replacement: Vec::new(), input_files: Vec::new() };
        test_read_lines(b"foo bar\nbaz\n", b"foo bar baz\n", &mk_args(10, false));
        test_read_lines(b"foo bar \nbaz\n", b"foo bar baz\n", &mk_args(10, true));
        test_read_lines(b"foo\nbar\nbaz\n", b"foo bar baz\n", &mk_args(2, false));
        test_read_lines(b"foobar\nbaz\nt1 t2\n", b"foobar\nbaz t1 t2\n", &mk_args(5, false));
    }

    #[test]
    fn test_list_handling_without_preserving_indentation() {
        let mk_args = |max_line_length_: usize|
            Cli{ max_line_length: max_line_length_, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: false, rewrap: false, substitute_regex: Vec::new(), replacement: Vec::new(), input_files: Vec::new() };
        test_read_lines(b"A list\nfollows:\n* foo bar baz\n* test1 test2\ntest3 test4\n", b"A list follows:\n* foo bar baz\n* test1 test2 test3 test4\n", &mk_args(14));
        test_read_lines(b"A list\nfollows:\n* foo bar baz\n* test1 test2\ntest3 test4\n", b"A list follows:\n* foo bar baz\n* test1 test2 test3 test4\n", &mk_args(13));
    }

    #[test]
    fn test_list_handling_with_preserving_indentation() {
        let mk_args = |max_line_length_: usize|
            Cli{ max_line_length: max_line_length_, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: false, substitute_regex: Vec::new(), replacement: Vec::new(), input_files: Vec::new() };
        test_read_lines(b"A list\nfollows:\n* foo bar baz\n* test1 test2\n  test3 test4\n", b"A list follows:\n* foo bar baz\n* test1 test2 test3 test4\n", &mk_args(13));
        test_read_lines(b"A list\nfollows:\n* foo bar baz\n  * test1\n    test2\n    test3\n    test4\n", b"A list follows:\n* foo bar baz\n  * test1 test2 test3 test4\n", &mk_args(13));
        test_read_lines(b"A list follows:\n* foo bar baz\n  * test1 test2\n    test3 test4\n", b"A list follows:\n* foo bar baz\n  * test1 test2 test3 test4\n", &mk_args(15));
    }

    #[test]
    fn test_rewrapping() {
        let mk_args = |max_line_length_: usize|
            Cli{ max_line_length: max_line_length_, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: true, substitute_regex: Vec::new(), replacement: Vec::new(), input_files: Vec::new() };
        test_read_lines(b"A list follows:\n* foo bar baz\n  * test1 test2\n    test3 test4\n", b"A list follows:\n* foo bar baz\n  * test1 test2 test3 test4\n", &mk_args(15));
        test_read_lines(b"A list follows:\n* foo bar baz\n  * test1 test2\n    test3 test4\n", b"A list\nfollows:\n* foo\n  bar baz\n  * test1 test2 test3 test4\n", &mk_args(15));
        test_read_lines(b"A list follows:\n* foo bar baz\n  * test1 test2 test3 test4\n", b"A list\nfollows:\n* foo\n  bar baz\n  * test1 test2 test3 test4\n", &mk_args(0));
    }

    #[test]
    fn test_reading_input_files() {
        let input_file_paths = Vec::from([String::from("testfiles/testinput1"), String::from("testfiles/testinput2")]);
        let mk_args = |max_line_length_: usize|
        Cli{ max_line_length: max_line_length_, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: true, substitute_regex: Vec::new(), replacement: Vec::new(), input_files: input_file_paths };
        test_read_lines(b"foo bar 1 2 3 4\n5 6 7 8 9 10 11\n12\n", b"", &mk_args(15));
    }

    #[test]
    fn test_substitution() {
        let mk_args = |_substitute_regex: Vec<String>, _replacement: Vec<String>|
        Cli{ max_line_length: 20, break_words: false, keep_trailing_whitespaces: false, preserve_list_indentation: true, rewrap: false, substitute_regex: _substitute_regex, replacement: _replacement, input_files: Vec::new() };
        test_read_lines(b"f00bar\nf00baz\n", b"foobar\nfoobaz\n", &mk_args(vec!["oo".to_owned(), "remove".to_owned()], vec!["00".to_owned()]));
    }
}
