# The image tools on a phone

Three tools sit behind one chip at the left edge of the drawing, opposite
the zoom chip on the right: **Matting**, **Upscale** and **Variable data**.
A tap on the chip opens the three of them just above it, and one of the
three opens its own sheet.

The models the first two use are not in the app. They are served from
`https://tracedraw.metoo.dpdns.org/models` (`ai_tools::HOST`), and land in
**`Download/models`**: on a phone that is the shared storage's own Download
folder, where a file manager can drop a model as easily as the app can, so
one copied by hand is found like a downloaded one. Without the storage grant
a phone falls back to its own folder (`getFilesDir()/models`); a desktop
uses `~/Download/models`.

They are fetched from **Help > Updates**, which is a page rather than a
message box: the version, what the check found, and below it every model
with what it is doing. `ai_tools::download` writes the files as they arrive,
nothing is fetched twice at once, and a failed download says what it stopped
on. The image tools' own panels name the same models and can fetch them too,
so a model is one tap from wherever it is missed.

| Tool | Models in the bucket | What it does |
| --- | --- | --- |
| Matting | `modnet.onnx` (people), `rmbg-1.4.onnx` (anything) | Takes the background out of the selected pictures |
| Upscale | `ultrasharp-4x.param` + `.bin`, `digital-art-4x.param` + `.bin` | Makes the selected picture four times its size |

## Matting

The panel carries the same settings the desktop dialog does (tolerance,
edge softness, Contiguous, Remove specks, Clean edge colors), and applies
them to every selected picture through `App::apply_to_bitmaps`, the way
Bitmaps > Remove Background does. The colours the background is learnt from
come from the picture's border, so a picture with a busy edge wants the
tolerance raised before it is applied.

## Upscale

Two models only, both 4x: UltraSharp for photographs, Digital Art for
illustrations. The size is fixed at four times; there is no half step,
since neither model was trained for one.

The upscaling itself reads the NCNN model with no C++ runtime, so the build
stays Rust only and still compiles for the browser (`upscale/`):

- `param.rs` reads the `.param` (one layer to a line: its type, its name,
  how many blobs it reads and writes, their names, then `key=value` pairs,
  where a key may be negative and a value may be a list) and the `.bin`.
  **The weights are `f16`, not `f32`**, with a four byte mark before each
  layer's weights and a single precision bias after them; which layout a
  file is in is not written anywhere, so it is told by the reading that
  ends exactly at the end of the file. Both models in the bucket check out
  to the byte.
- `net.rs` runs the layers the upscalers use: a convolution with its own
  activation (`9=2` is a leaky ReLU whose slope is `-23310`), a split, a
  concat, an element-wise add and a nearest resize. The add is **weighted**
  when the layer gives coefficients (`-23301=2,0.2,1.0`): an upscaler's
  residual block adds its own input back at a fifth, and adding it whole
  turns the picture to mud. Feature maps are dropped as soon as nothing
  reads them again, which a thousand layers need.
- `mod.rs` cuts the picture into 160 pixel tiles, each carried 12 pixels
  beyond its own edge so the seams have something to read, and throws the
  margin away at the join. The alpha channel is enlarged on its own with a
  smooth filter, so a picture cut out of its background keeps the cut.

The network reads colour in `[0,1]` and writes the same; nothing is
normalised by a mean or a standard deviation, which is what Real-ESRGAN
does. A phone runs a tile in seconds, so a large picture takes a while: the
panel shows the tiles as they finish, and the work is on a thread of its
own, so the sheet stays live.

### On the graphics card

The window already draws with `wgpu`, so the card is there to use and
nothing is carried for it: `upscale/gpu.rs` takes the device and queue the
window was made with (`Shell::ui` hands them over once) and runs the same
network in compute passes, four kernels in `upscale.wgsl`: a convolution
with its activation, a weighted add, a channel copy for a concat and a
nearest resize.

- The whole network goes into **one command buffer** and the picture comes
  back once at the end, so a thousand layers cost one wait, not a thousand.
- Feature maps go back into a **pool** as soon as nothing reads them; a
  split puts one buffer under several names, and a buffer goes back only
  when the last of them lets go. Without that, a thousand layers would ask
  for gigabytes.
- A dispatch is split over **two axes**: a picture four times its size has
  more elements than one axis of 65535 workgroups can name.
- A kernel the phone's driver will not take is a **validation error**, and
  an uncaught one stops the app. The build is wrapped in an error scope and
  a single pixel is put through the whole network before any picture is
  sent: a card that refuses leaves the processor to do the work, and the
  panel says which one ran.

A phone's card is ten to thirty times the arithmetic of its processor
(an Adreno 840 is a few TFLOPS against a hundred GFLOPS of cores), which is
what makes an upscaler usable rather than a coffee break.

## Variable data

The desktop's Print Merge panel, unchanged, in the phone's sheet: a table
read from a CSV or TSV file (a quoted field may hold the separator), the
field each column fills, and the copies laid out across and down a page.

`ai_tools::Table::parse` reads the table for the phone's own preview; the
panel itself is `ui::print_merge_docker::print_merge`, the same code the
desktop draws, so a change there is a change here.

## Where the state lives

The three panels keep what the user picked in one `ai_tools::AiTools` in the
context's data, so a sheet can be closed and opened again without losing the
model chosen or the settings typed. The models themselves are files: what is
on the phone is read from the folder each time a panel is drawn, so a file
copied there by hand is seen at once.
