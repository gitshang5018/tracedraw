# 转曲保存 (Save as curves)

The Files sheet of the phone layout carries **转曲存 X8** beside Save and
Save As: the drawing with everything Convert to Curves would change turned
into curves, written as a `.cdr` of version 18 (CorelDRAW X8) into the
phone's Download folder, and then handed to the system's share sheet, where
WeChat, QQ and the rest of the installed apps take it. `curves_cdr.rs` holds
it; the name and the version mark follow the plugin this comes from.

## What becomes curves

Exactly what Convert to Curves does (`app.rs`,
`convert_shapes_to_curves`): text (artistic and paragraph), tables (cell
fills, borders and text), dimensions (their lines, arrowheads and label),
symbol instances (their symbol's objects) and groups and clip frames that
hold any of them, with envelopes, perspectives and distortions made part of
the curves. Bitmaps stay bitmaps.

The work happens on a **copy** of the drawing, in an app of its own
(`App::scratch`): the drawing on screen keeps its text, its selection and
its undo history, so a save that goes wrong leaves nothing to undo.

## While it runs

A page is converted per frame (`App::curves_tick`), so the box over the
interface carries a bar that moves and the interface keeps drawing between
pages; the file is written on the frame after the last one, so the bar
reaches the end before the write takes its time. A drawing of one page shows
the bar full at once, which is as fine as the work can be divided.

## The file

`<drawing>_<yyyy-M-d>_x8曲.cdr`, in the phone's Download folder, or beside
the drawing on a desktop. An older date suffix of the same shape
(`_2026-10-27_x8曲`) is dropped from the drawing's name first, so saving
twice in a day does not stack them, and a drawing with no name of its own
is 未命名.

From Android 10 the file goes in through `MediaStore`'s Download collection
(`android_system::save_and_share`): that is what gives it a `content://`
address other apps may read, which a plain path stopped being in Android 7.
The same address goes to `ACTION_SEND` with `FLAG_GRANT_READ_URI_PERMISSION`,
so the app the user picks may open it. A write that fails takes its row out
of the collection, so the Download folder is not left with a half file.
Before Android 10 there is no such collection: the file is written where the
app may write and no share sheet opens.

The type the share carries is `application/octet-stream`, since WeChat and
QQ take a file of any type that way and a `.cdr` type would leave them
without a handler.

## Checks

- `curves_cdr::tests::the_name_carries_the_day_and_the_version`
- `curves_cdr::tests::the_day_is_written_the_way_the_name_wants_it`
- The `.cdr` writer's own round-trip tests cover version 18 with the
  objects a drawing holds (`cdr-format.md`, Writing).
