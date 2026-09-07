// Convert a title into a URL-friendly slug (lowercase, dashes).
fn main(input) {
    let out = "";
    for c in input.to_lower().chars() {
        let alnum = (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9');
        if alnum {
            out += c.to_string();
        } else if (c == ' ' || c == '-' || c == '_') && out.len() > 0 {
            out += "-";
        }
    }
    out
}

main(input)
