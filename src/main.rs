use std::io;
use formatter::run;

fn main() {
    std::process::exit(run(&mut io::stdout().lock(), &mut io::stdin().lock()));
}
