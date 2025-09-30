struct Term<'a> {
    term: &'a str,
    idf: i32,
}

impl<'a> Term<'a> {
    fn new(term: &'a str) -> Self {
        Term { term, idf: 0 }
    }
}

fn main() {
    println!("Hello, world!");
}
