use std::io;
use std::fmt;

fn parse_header(buf: &[u8]) -> Result<u32, io::Error> {
    let x = buf[0] as u32;
    Ok(x + 2)
}

fn unchanged_a() {
    println!("a");
}

fn unchanged_b() {
    println!("b");
}

fn tail() {
    let v = vec![1, 2, 3, 4];
}
