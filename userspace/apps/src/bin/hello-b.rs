//! Says hello five times, taking turns with hello-a.
#![no_std]
#![no_main]

use libhydrogen::{println, sleep_ms};

libhydrogen::entry!(main);

fn main() -> i32 {
    for i in 1..=5 {
        println!("B{i}");
        sleep_ms(20);
    }
    0
}
