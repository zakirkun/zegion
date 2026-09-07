// Capitalize the first letter of each word.
fn main(input) {
    let out = "";
    let cap = true;
    for c in input.chars() {
        if c == ' ' || c == '\t' || c == '\n' {
            cap = true;
            out += c.to_string();
        } else if cap {
            out += c.to_upper().to_string();
            cap = false;
        } else {
            out += c.to_string();
        }
    }
    out
}

main(input)
