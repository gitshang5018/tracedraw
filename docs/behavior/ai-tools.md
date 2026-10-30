# The image tools on a phone

Two tools sit behind one chip at the left edge of the drawing, opposite
the zoom chip on the right: **Matting** and **Variable data**. A tap on the
chip opens the two of them just above it, and one of the two opens its own
sheet.

The model the first one uses is not in the app. They are served from
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

## Matting

The panel carries the same settings the desktop dialog does (tolerance,
edge softness, Contiguous, Remove specks, Clean edge colors), and applies
them to every selected picture through `App::apply_to_bitmaps`, the way
Bitmaps > Remove Background does. The colours the background is learnt from
come from the picture's border, so a picture with a busy edge wants the
tolerance raised before it is applied.

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
