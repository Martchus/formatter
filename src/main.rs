use std::io;
use formatter::run;

fn main() {
    run(&mut io::stdout().lock(), &mut io::stdin().lock());
}
