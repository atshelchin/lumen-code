//! Writes the self-test's synthetic camera frame as frame.nv12 (the `grab` format), for
//! comparing with real captures: cargo run --release --example synthframe -- <out file>
use lumen::transfer::{prepare, Sender};
use lumen::Renderer;
use lumen_app::engine::bench::camera_image;
use lumen_app::geometry::{compute, Density, Viewport, QUIET};

fn main() {
    let out = std::env::args().nth(1).expect("an output file");
    let vp = Viewport { width_px: 1170.0, height_px: 2350.0, scale: 3.0, mm_per_px: 0.0533 };
    let g = compute(&vp, Density::Auto);
    let data: Vec<u8> = (0..24_000u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8).collect();
    let mut tx = Sender::new(&prepare(&data, "s.bin", false), 1, 1, g.x, g.y, 0x5e1f, 0).unwrap();
    let r = Renderer::new(&tx.layout, tx.mode, 7, QUIET);
    let mut code = vec![0u32; r.width * r.height];
    r.draw(&tx.next_frame().cells, &mut code);
    let (fw, fh) = (1920usize, 1080usize);
    let (mut y, mut uv) = (vec![0u8; fw * fh], vec![0u8; fw * fh / 2]);
    camera_image(&code, r.width, r.height, &mut y, &mut uv, fw, fh);
    let mut f: Vec<u8> = [fw as u32, fh as u32, fw as u32, fw as u32].iter().flat_map(|v| v.to_le_bytes()).collect();
    f.extend_from_slice(&y);
    f.extend_from_slice(&uv);
    std::fs::write(out, f).unwrap();
    println!("grid {}x{}, {} codewords", g.x, g.y, tx.per_frame);
}
