// Hard-wrap text to a maximum line width (60 columns).
fn main(input) {
    let width = 60;
    let out = "";
    let col = 0;
    for word in input.split(" ") {
        if col + word.len() > width {
            out += "\n";
            col = 0;
        } else if col > 0 {
            out += " ";
            col += 1;
        }
        out += word;
        col += word.len();
    }
    out
}

main(input)
