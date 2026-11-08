//! Run with: cargo run -p mineral-empire --example big_numbers
use mineral_empire::big::Big;
fn main() {
    let a: Big = "6.25e999999999999999999999".parse().unwrap();
    let b: Big = "2.5e999999999999999999999".parse().unwrap();
    println!("a + b = {}", a.add(&b));
    println!("a - b = {}", a.sub(&b));
    println!("a * b = {}", a.mul(&b));
    println!("a / b = {}", a.div(&b));
    println!("serialized a: {}", a.storage());
}
