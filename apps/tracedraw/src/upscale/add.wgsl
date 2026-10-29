// Two tensors added, each taken by its own coefficient.

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
