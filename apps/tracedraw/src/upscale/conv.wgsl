// One convolution: a filter over the picture, its bias, and the activation
// the layer carries.
//
// `gx` is how many workgroups the first axis took, so a kernel run in
// several rows still knows which flat index it is at: a picture four times
// its size has more elements than one axis can name.

struct ConvParams {
    w: u32,
    h: u32,
    in_c: u32,
    out_c: u32,
    pad: u32,
    kh: u32,
    kw: u32,
    slope: f32,
    gx: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var<uniform> cp: ConvParams;
@group(0) @binding(1) var<storage, read> conv_in: array<f32>;
@group(0) @binding(2) var<storage, read> conv_w: array<f32>;
@group(0) @binding(3) var<storage, read> conv_b: array<f32>;
@group(0) @binding(4) var<storage, read_write> conv_out: array<f32>;

@compute @workgroup_size(64)
fn conv(@builtin(global_invocation_id) gid: vec3<u32>) {
    let total = cp.w * cp.h * cp.out_c;
    let i = gid.y * (cp.gx * 64u) + gid.x;
    if (i >= total) {
        return;
    }
    let oc = i % cp.out_c;
    let rest = i / cp.out_c;
    let x = rest % cp.w;
    let y = rest / cp.w;
    var acc = 0.0;
    let iy0 = i32(y) - i32(cp.pad);
    let ix0 = i32(x) - i32(cp.pad);
    for (var ky = 0u; ky < cp.kh; ky = ky + 1u) {
        let iy = iy0 + i32(ky);
        if (iy < 0 || iy >= i32(cp.h)) {
            continue;
        }
        for (var kx = 0u; kx < cp.kw; kx = kx + 1u) {
            let ix = ix0 + i32(kx);
            if (ix < 0 || ix >= i32(cp.w)) {
                continue;
            }
            let base = (u32(iy) * cp.w + u32(ix)) * cp.in_c;
            let wbase = ((oc * cp.kh + ky) * cp.kw + kx) * cp.in_c;
            for (var ci = 0u; ci < cp.in_c; ci = ci + 1u) {
                acc = acc + conv_w[wbase + ci] * conv_in[base + ci];
            }
        }
    }
    acc = acc + conv_b[oc];
    if (acc < 0.0) {
        acc = acc * cp.slope;
    }
    conv_out[i] = acc;
}
