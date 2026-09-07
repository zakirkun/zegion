// Mask email addresses and long words (simple scrubber).
fn main(input) {
    let out = "";
    for word in input.split(" ") {
        if word.contains("@") {
            out += "[email] ";
        } else if word.len() >= 12 {
            out += "[long] ";
        } else {
            out += word + " ";
        }
    }
    out
}

main(input)
