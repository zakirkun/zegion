// Count words, characters, and lines in the input text.
fn main(input) {
    let chars = input.len();
    let words = 0;
    for w in input.split(" ") {
        if w.len() > 0 { words += 1; }
    }
    let lines = 0;
    for line in input.split("\n") { lines += 1; }
    `chars=${chars} words=${words} lines=${lines}`
}

main(input)
