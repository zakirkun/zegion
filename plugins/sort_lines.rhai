// Sort the input's lines alphabetically.
fn main(input) {
    let lines = input.split("\n");
    lines.sort();
    let out = "";
    for l in lines { out += l + "\n"; }
    out
}

main(input)
