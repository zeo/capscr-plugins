#![cfg(target_arch = "wasm32")] // wasm-only plugin; empty lib on other targets
//! capscr plugin: downscale captures whose longest side exceeds a configured
//! limit, so saved/uploaded images stay small. The limit is read at runtime from
//! `config.toml` (`max_dimension`, default 1920). Box-average downscale by an
//! integer factor — dependency-free. Demonstrates `config_get` + the v0.5
//! image-blob `on_capture` API together. See docs/plugin-runtime.md for the ABI.

const DEFAULT_MAX_DIM: u32 = 1920;
const CONFIG_KEY: &str = "max_dimension";

// the host writes the input blob ([w][h][mode][rgba]) here via capscr_alloc, and
// reuses it for config_get's value too — so on_capture copies the input out
// first. OUTPUT holds the downscaled replacement ([w][h][rgba]).
static mut SCRATCH: Vec<u8> = Vec::new();
static mut OUTPUT: Vec<u8> = Vec::new();

#[no_mangle]
pub extern "C" fn capscr_alloc(size: i32) -> i32 {
    let size = size.max(0) as usize;
    // SAFETY: single-threaded wasm; calls are serialised by the host store lock
    unsafe {
        let buf = &mut *core::ptr::addr_of_mut!(SCRATCH);
        buf.clear();
        buf.reserve(size);
        buf.as_mut_ptr() as i32
    }
}

#[link(wasm_import_module = "capscr")]
extern "C" {
    fn config_get(key_ptr: i32, key_len: i32) -> i64;
}

/// read `max_dimension` from config, falling back to the default. only valid
/// after the caller has copied the input out of SCRATCH (config_get reuses it).
fn read_max_dim() -> u32 {
    let packed =
        unsafe { config_get(CONFIG_KEY.as_ptr() as i32, CONFIG_KEY.len() as i32) };
    if packed == 0 {
        return DEFAULT_MAX_DIM;
    }
    let ptr = ((packed as u64) >> 32) as usize;
    let len = (packed as u64 & 0xffff_ffff) as usize;
    let s = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    core::str::from_utf8(s)
        .ok()
        .and_then(|t| t.trim().parse::<u32>().ok())
        .filter(|&d| d > 0)
        .unwrap_or(DEFAULT_MAX_DIM)
}

/// on_capture: 0 = continue unchanged, >0 = packed (ptr<<32)|len of a
/// replacement [w:u32][h:u32][rgba] blob. (never cancels)
#[no_mangle]
pub extern "C" fn capscr_on_capture(ptr: i32, len: i32) -> i64 {
    if ptr < 0 || len < 12 {
        return 0;
    }
    // copy the input out of SCRATCH before config_get reuses that buffer
    let input = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) }.to_vec();
    let w = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    let h = u32::from_le_bytes([input[4], input[5], input[6], input[7]]);
    let rgba = &input[12..];
    let expected = (w as usize).saturating_mul(h as usize).saturating_mul(4);
    if rgba.len() != expected || expected == 0 {
        return 0;
    }

    let max_dim = read_max_dim();
    let longest = w.max(h);
    if longest <= max_dim {
        return 0; // already within the limit — leave it untouched
    }

    // integer box-average downscale: factor = ceil(longest / max_dim)
    let f = longest.div_ceil(max_dim).max(2);
    let nw = w.div_ceil(f);
    let nh = h.div_ceil(f);
    let row = w as usize * 4;
    let out_len = 8 + (nw as usize) * (nh as usize) * 4;

    // SAFETY: serialised calls; the host copies the bytes out before the next
    // call clears this buffer
    unsafe {
        let out = &mut *core::ptr::addr_of_mut!(OUTPUT);
        out.clear();
        out.reserve(out_len);
        out.extend_from_slice(&nw.to_le_bytes());
        out.extend_from_slice(&nh.to_le_bytes());
        for oy in 0..nh {
            for ox in 0..nw {
                // average the f×f input block (clamped at the edges)
                let (mut r, mut g, mut b, mut a, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
                let x0 = ox * f;
                let y0 = oy * f;
                for dy in 0..f {
                    let y = y0 + dy;
                    if y >= h {
                        break;
                    }
                    for dx in 0..f {
                        let x = x0 + dx;
                        if x >= w {
                            break;
                        }
                        let i = y as usize * row + x as usize * 4;
                        r += rgba[i] as u32;
                        g += rgba[i + 1] as u32;
                        b += rgba[i + 2] as u32;
                        a += rgba[i + 3] as u32;
                        n += 1;
                    }
                }
                let n = n.max(1);
                out.push((r / n) as u8);
                out.push((g / n) as u8);
                out.push((b / n) as u8);
                out.push((a / n) as u8);
            }
        }
        let p = out.as_ptr() as i64;
        (p << 32) | (out_len as i64 & 0xffff_ffff)
    }
}
