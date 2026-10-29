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
