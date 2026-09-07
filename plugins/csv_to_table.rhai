// Convert simple CSV text into a pipe-separated text table.
fn main(input) {
    let out = "";
    for line in input.split("\n") {
        if line.len() > 0 {
            let cells = line.split(",");
            let row = "";
            for c in cells { row += c + " | "; }
            out += "| " + row + "\n";
        }
    }
    out
}

main(input)
