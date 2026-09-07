// Reverse the input string (character-wise).
fn main(input) {
    let rev = "";
    for c in input.chars() {
        rev = c + rev;
    }
    rev
}

main(input)
