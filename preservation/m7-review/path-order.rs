#![forbid(unsafe_code)]
fn main() {
    let paths = [std::path::PathBuf::from("usr/lib-x"), std::path::PathBuf::from("usr/lib/a")];
    let sorted: std::collections::BTreeSet<_> = paths.into_iter().collect();
    let text: Vec<_> = sorted.iter().map(|p| p.to_str().unwrap()).collect();
    println!("Path ordering: {text:?}");
    println!("String validator accepts: {}", text.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(text, ["usr/lib/a", "usr/lib-x"]);
    assert!(!text.windows(2).all(|w| w[0] < w[1]));
}
