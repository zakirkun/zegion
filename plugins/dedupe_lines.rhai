// Remove duplicate lines, preserving first-seen order.
fn main(input) {
    let seen = [];
    let out = "";
    for line in input.split("\n") {
        if !seen.contains(line) {
            seen.push(line);
            out += line + "\n";
        }
    }
    out
}

main(input)
