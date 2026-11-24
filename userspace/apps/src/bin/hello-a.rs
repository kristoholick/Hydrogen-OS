//! Says hello five times, taking turns with hello-b.
#![no_std]
#![no_main]

use libhydrogen::{println, sleep_ms};

libhydrogen::entry!(main);

fn main() -> i32 {
    for i in 1..=5 {
        println!("A{i}");
        sleep_ms(20);
    }
    0
}
