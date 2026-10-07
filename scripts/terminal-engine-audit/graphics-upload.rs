// Standalone audit probe; compile the identical source against both release libraries.
use std::{hint::black_box, time::Instant};
use termy_core::terminal_engine::{Engine, Options, Size};
fn run(size: Size, virtual_on: bool, payload: &[Vec<u8>]) -> f64 {
    let mut e = Engine::new(size, Options::default());
    if virtual_on {
        e.feed(b"\x1b_Ga=T,f=32,s=1,v=1,i=1,c=1,r=1,U=1,q=2;AQID/w==\x1b\\");
    }
    let start = Instant::now();
    for chunk in payload {
        e.feed(black_box(chunk));
    }
    black_box(e.graphics_revision());
    start.elapsed().as_secs_f64() * 1000.
}
fn main() {
    let mut payload = Vec::new();
    for i in 0..512 {
        let header = if i == 0 {
            "\x1b_Ga=t,f=32,s=512,v=768,i=2,m=1,q=2;"
        } else if i == 511 {
            "\x1b_Gm=0;"
        } else {
            "\x1b_Gm=1;"
        };
        let mut chunk = header.as_bytes().to_vec();
        chunk.extend(std::iter::repeat_n(b'A', 4096));
        chunk.extend_from_slice(b"\x1b\\");
        payload.push(chunk);
    }
    for size in [
        Size {
            cols: 120,
            rows: 40,
        },
        Size {
            cols: 240,
            rows: 80,
        },
        Size {
            cols: 1024,
            rows: 1024,
        },
    ] {
        for pair in 0..5 {
            let (a, b) = if pair % 2 == 0 {
                (run(size, false, &payload), run(size, true, &payload))
            } else {
                let b = run(size, true, &payload);
                (run(size, false, &payload), b)
            };
            println!(
                "{}x{} pair{} no_virtual_ms={:.3} virtual_ms={:.3} ratio={:.3}",
                size.cols,
                size.rows,
                pair,
                a,
                b,
                b / a
            );
        }
    }
}
