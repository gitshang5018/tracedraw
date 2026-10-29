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

The panel fetches the model it is told to. The upscaling itself reads the
NCNN model (`.param` is the network as text, `.bin` the weights, layer after
layer in the order the `.param` names them) with no C++ runtime, so the
build stays Rust only and still compiles for the browser. Until that pass is
wired up the panel says so rather than pretending: `ai.upscale_pending`.

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
