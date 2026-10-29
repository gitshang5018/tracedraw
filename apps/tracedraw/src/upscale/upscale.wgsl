// The upscaler's layers on the graphics card.
//
// Tensors are channel-last, the same order the CPU pass uses, and the
// weights are already turned to `[out][kh][kw][in]`, so one filter tap is a
// run of channels with nothing in between. Every kernel takes a flat index
// and works out the pixel and channel it belongs to.

// `gx` is how many workgroups the first axis took, so a kernel run in
// several rows still knows which flat index it is at. A picture four times
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

struct AddParams {
    n: u32,
    k0: f32,
    k1: f32,
    pad: u32,
    gx: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var<uniform> ap: AddParams;
@group(0) @binding(1) var<storage, read> add_a: array<f32>;
@group(0) @binding(2) var<storage, read> add_b: array<f32>;
@group(0) @binding(3) var<storage, read_write> add_out: array<f32>;

@compute @workgroup_size(64)
fn add(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.y * (ap.gx * 64u) + gid.x;
    if (i >= ap.n) {
        return;
    }
    add_out[i] = add_a[i] * ap.k0 + add_b[i] * ap.k1;
}

// One input's channels into their place in the joined tensor.
struct CopyParams {
    px: u32,
    src_c: u32,
    dst_c: u32,
    dst_off: u32,
    gx: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var<uniform> sp: CopyParams;
@group(0) @binding(1) var<storage, read> copy_src: array<f32>;
@group(0) @binding(2) var<storage, read_write> copy_dst: array<f32>;

@compute @workgroup_size(64)
fn copy_channels(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.y * (sp.gx * 64u) + gid.x;
    if (i >= sp.px * sp.src_c) {
        return;
    }
    let pixel = i / sp.src_c;
    let c = i % sp.src_c;
    copy_dst[pixel * sp.dst_c + sp.dst_off + c] = copy_src[i];
}

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
