// Nearest neighbour by a whole factor: each pixel becomes a square of them.

struct ResizeParams {
    in_w: u32,
    in_h: u32,
    c: u32,
    scale: u32,
    gx: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var<uniform> rp: ResizeParams;
@group(0) @binding(1) var<storage, read> small: array<f32>;
@group(0) @binding(2) var<storage, read_write> big: array<f32>;

@compute @workgroup_size(64)
fn resize(@builtin(global_invocation_id) gid: vec3<u32>) {
    let ow = rp.in_w * rp.scale;
    let oh = rp.in_h * rp.scale;
    let i = gid.y * (rp.gx * 64u) + gid.x;
    if (i >= ow * oh * rp.c) {
        return;
    }
    let c = i % rp.c;
    let rest = i / rp.c;
    let x = rest % ow;
    let y = rest / ow;
    let sx = x / rp.scale;
    let sy = y / rp.scale;
    big[i] = small[(sy * rp.in_w + sx) * rp.c + c];
}
