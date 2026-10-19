use std::io;

fn parse_header(buf: &[u8]) -> u32 {
    let x = buf[0] as u32;
    x + 1
}

fn unchanged_a() {
    println!("a");
}

fn unchanged_b() {
    println!("b");
}

fn tail() {
    let v = vec![1, 2, 3];
    println!("{:?}", v);
}
