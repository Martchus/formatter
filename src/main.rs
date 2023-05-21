use std::{io::{self, BufRead, Write}};
use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value_t = 0)]
    max_line_length: usize,
    #[arg(short, long, default_value_t = false)]
    break_words: bool,
    #[arg(short, long, default_value_t = false)]
    keep_trailing_whitespaces: bool,
}

fn write_line(output: &mut dyn Write, line: &String, args: &Cli) {
    if args.keep_trailing_whitespaces {
        write!(output, "{}\n", line).unwrap();
    } else {
        write!(output, "{}\n", line.trim_end()).unwrap();
    }
}

fn handle_next_line(output: &mut dyn Write, line: String, args: &Cli) {
    let mut output_line = String::new();
    let mut has_last_word_end = false;
    let mut last_word_end: usize = 0;

    for c in line.chars() {
        // handle the case when the current line is full
        if args.max_line_length != 0 && output_line.len() == args.max_line_length {
            if args.break_words {
                // print the output line we have so far and write further characters into a new/clear output line
                write_line(output, &output_line, &args);
                output_line.clear();
            } else if has_last_word_end {
                // print the output line we have so far but only until the last whitespace; keep further characters
                // the output line for the next line
                let output_line_until_last_whitespace: String = output_line.drain(..last_word_end + 1).collect();
                write_line(output, &output_line_until_last_whitespace, &args);
            } else {
                // print the output line we have so far but without trailing newline
                write!(output, "{}", output_line).unwrap();
                output_line.clear();
            }
            has_last_word_end = false;
        }

        // take note of word boundaries
        if c.is_whitespace() {
            last_word_end = output_line.len();
            has_last_word_end = true;
        }

        // add the current character to current line
        output_line.push(c);
    }

    // print what's left in the current output line
    write!(output, "{}\n", output_line).unwrap();
}

fn read_lines(output: &mut dyn Write, input: &mut dyn BufRead, args: &Cli) {
    for line in input.lines() {
        handle_next_line(output, line.unwrap(), &args);
    }
}

fn main() {
    read_lines(&mut io::stdout().lock(), &mut io::stdin().lock(), &Cli::parse());
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
        test_read_lines(b"foo\n", b"foo\n", &Cli{ max_line_length: 0, break_words: true, keep_trailing_whitespaces: true });
    }

    #[test]
    fn test_line_wrapping_with_word_breaks() {
        test_read_lines(b"foo bar ba\nz\n", b"foo bar baz\n", &Cli{ max_line_length: 10, break_words: true, keep_trailing_whitespaces: false });
        test_read_lines(b"foo bar ba\nz\n", b"foo bar baz\n", &Cli{ max_line_length: 10, break_words: true, keep_trailing_whitespaces: true });
        test_read_lines(b"fo\no\nba\nr\nba\nz\n", b"foo bar baz\n", &Cli{ max_line_length: 2, break_words: true, keep_trailing_whitespaces: false });
        test_read_lines(b"fo\no \nba\nr \nba\nz\n", b"foo bar baz\n", &Cli{ max_line_length: 2, break_words: true, keep_trailing_whitespaces: true });
        test_read_lines(b"fooba\nr\nbaz\n", b"foobar\nbaz\n", &Cli{ max_line_length: 5, break_words: true, keep_trailing_whitespaces: false });
    }

    #[test]
    fn test_line_wrapping_without_work_breaks() {
        test_read_lines(b"foo bar\nbaz\n", b"foo bar baz\n", &Cli{ max_line_length: 10, break_words: false, keep_trailing_whitespaces: false });
        test_read_lines(b"foo bar \nbaz\n", b"foo bar baz\n", &Cli{ max_line_length: 10, break_words: false, keep_trailing_whitespaces: true });
        test_read_lines(b"foo\nbar\nbaz\n", b"foo bar baz\n", &Cli{ max_line_length: 2, break_words: false, keep_trailing_whitespaces: false });
        test_read_lines(b"foobar\nbaz\nt1 t2\n", b"foobar\nbaz t1 t2\n", &Cli{ max_line_length: 5, break_words: false, keep_trailing_whitespaces: false });
    }
}
