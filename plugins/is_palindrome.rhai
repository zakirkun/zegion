// Check whether the input (letters/digits only, case-insensitive) is a palindrome.
fn main(input) {
    let clean = "";
    for c in input.to_lower().chars() {
        let alnum = (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9');
        if alnum { clean += c.to_string(); }
    }
    let rev = "";
    for c in clean.chars() { rev = c.to_string() + rev; }
    if clean == rev { "palindrome" } else { "not a palindrome" }
}

main(input)
