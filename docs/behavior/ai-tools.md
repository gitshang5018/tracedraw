# The image tools on a phone

Three tools sit behind one chip at the left edge of the drawing, opposite
the zoom chip on the right: **Matting**, **Upscale** and **Variable data**.
A tap on the chip opens the three of them just above it, and one of the
three opens its own sheet.

The models the first two use are not in the app. They are in the project's
own bucket, under `models/`, at the address the phone's own update comes
from (`TRACEDRAW_APK_HOST`, the `UPDATE_HOST` repository variable). A panel
names its models, shows which of them is on the phone, and fetches the one
that is not: `ai_tools::download` writes the files into the app's folder
(`getFilesDir()/models` on a phone, `~/.tracedraw/models` elsewhere), and
the panel shows the bytes as they arrive. Nothing is fetched twice at once,
and a failed download says what it stopped on.

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
