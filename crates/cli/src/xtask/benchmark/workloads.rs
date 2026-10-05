//! Continuous workloads keep both 60 Hz and 120 Hz frame budgets meaningful.
use super::*;

pub(super) fn run(scenario: Scenario, duration: Duration) -> Result<()> {
    let start = Instant::now();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let alternate = scenario != Scenario::CjkScroll;
    if alternate {
        write!(out, "\x1b[?1049h\x1b[?25l")?;
    }
    if scenario == Scenario::Graphics {
        // A small opaque RGBA checkerboard, uploaded once and moved thereafter.
        use base64::Engine;
        let mut pixels = Vec::with_capacity(64 * 64 * 4);
        for y in 0..64 {
            for x in 0..64 {
                pixels.extend_from_slice(if (x / 8 + y / 8) % 2 == 0 {
                    &[40, 160, 240, 255]
                } else {
                    &[240, 170, 40, 255]
                });
            }
        }
        let payload = base64::engine::general_purpose::STANDARD.encode(pixels);
        // Kitty transfers are chunked to the protocol's 4096-byte payload limit.
        for (index, chunk) in payload.as_bytes().chunks(4096).enumerate() {
            let more = usize::from((index + 1) * 4096 < payload.len());
            if index == 0 {
                write!(out, "\x1b_Ga=t,f=32,s=64,v=64,i=71,q=2,m={more};")?;
            } else {
                write!(out, "\x1b_Gm={more};")?;
            }
            out.write_all(chunk)?;
            write!(out, "\x1b\\")?;
        }
    }
    let mut frame = 0usize;
    while start.elapsed() < duration {
        if scenario == Scenario::CjkScroll {
            writeln!(
                out,
                "{frame:06} 中文测试 日本語かなカナ 한글 👩🏽‍💻 ❤️ 🇩🇰 e\u{301} 你好世界"
            )?;
        } else {
            write!(out, "\x1b[?2026h\x1b[H")?;
            for row in 0..28 {
                let color = 16 + (frame + row) % 216;
                writeln!(
                    out,
                    "\x1b[38;5;{color}m{frame:06} {row:02} 中文 日本語 한글 👩🏽‍💻 0123456789 abcdefghijklmnop\x1b[0m\x1b[K\r"
                )?;
            }
            if scenario == Scenario::Graphics {
                write!(
                    out,
                    "\x1b[{};{}H\x1b_Ga=p,i=71,p=1,c=8,r=4,q=2;\x1b\\",
                    2 + frame % 18,
                    2 + frame % 60
                )?;
            }
            write!(out, "\x1b[?2026l")?;
        }
        out.flush()?;
        frame += 1;
        thread::sleep(Duration::from_millis(4));
    }
    if alternate {
        write!(out, "\x1b[?25h\x1b[?1049l")?;
    }
    out.flush()?;
    Ok(())
}
