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

### Cutting the pieces apart

A picture on a transparent background often holds more than one subject: a
sheet of labels, a page of small parts, a photograph of several objects.
**Separate subjects** (`AiTools::matting_split`, on by default) takes the
subjects the matte keeps apart, by `matting::components`: two pixels belong
to the same subject when they touch, sides or corners, and a subject
smaller than a two thousandth of the picture (a hundred pixels at the
least) is left out, which is what keeps the specks a matte leaves behind
from becoming objects of their own. Each subject becomes an object of its
own, at the place it filled; the first keeps the picture's id, so one undo
step puts the whole cut-out back.

**Trim transparent edges** (`matting_trim`, on by default) cuts every piece
to what it fills: `matting::trimmed` takes the box the mask keeps, so no
transparent room above, below, left or right goes into the drawing, and a
piece lands where its pixels were rather than where the original picture's
edge was. With **Separate subjects** on, the pieces are trimmed whether this
is ticked or not.

## ID photo

With the portrait model chosen, **Make an ID photo** (`id_photo_on`) makes
an identification photo of the cut-out person instead of a cut-out.

| Setting | What it does |
| --- | --- |
| Size | One inch 25 x 35 mm, small one inch 22 x 32, two inch 35 x 49, small two inch 33 x 48, printed at 300 pixels to the inch |
| Background | White, blue, red, or a colour of its own |
| Room above the head | The part of the height left above the top of the person, 0.09 by default |
| Person's height | The part of the height the person takes, 0.86 by default |

`matting::id_photo` frames the person by their own alpha: the top of the
mask is the top of the hair, which is where the room above the head is
measured from. No face is looked for, so a picture from the waist up and one
from the knees up are both framed by their own outline. The room the crop
needs beyond the picture is filled with the background colour, which is what
the photo is printed on.

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
