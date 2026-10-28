# The interface on a phone

TraceDraw's workspace is laid out for a wide window: a menu bar, rows of
toolbars, a toolbox column, two docker columns, palette rows, rulers and a
status bar around the drawing. On a phone those panels would leave the
drawing a strip between two walls of chrome, and their buttons are sized
for a mouse pointer.

`ui::mobile` arranges the same panels for a screen held in one hand, in the
shape Android expects: a top app bar, a strip of tools, a navigation bar of
four destinations and bottom sheets, with 48 point targets, rounded state
layers and the theme's colours in the roles Material gives them. It is not
a second interface: every sheet draws the desktop's own widgets, so a
change to the property bar, a docker, the palette or a menu shows on both.

## When it is used

- Always in the Android build.
- In any other build started with `TRACEDRAW_MOBILE=1`, which is how the
  layout is checked on a desktop without a phone at hand. The file sheet
  then lists `TRACEDRAW_MOBILE_DIR` (the working folder when it is not
  set).

The desktop layout is untouched when the variable is not set.

## What is drawn

From the top down:

| Band | Height | Content |
| --- | --- | --- |
| Top app bar | 64 + status bar | the menu, the drawing's name at 22 point (a tap lists the open drawings), undo, redo |
| Drawing | the rest | the canvas, without rulers, scrollbars or navigator: pinch zooms, two fingers pan; a chip at the bottom right shows the zoom (a tap fits the page) and, in a drawing of several pages, the page (a tap lists them) |
| Tools | 64 | one target per toolbox group, the chosen one washed in the accent; a second tap on the chosen group, or a long press on any, lists its tools in a sheet |
| Navigation bar | 76 + gesture bar | Properties, Panels, Palette, Files: a destination each, which opens as a sheet and lights up while its sheet is open |
| Snackbar | 48 | what the last command said, over the bars, gone after four seconds |

The page sorter and the Welcome Screen take the whole screen, as they do
on the desktop.

## The system bars

A Material app draws edge to edge, and so does this one: the window covers
the status bar and the gesture bar, the interface paints its own background
behind them and keeps its buttons out of them. The heights come from the
window itself (`android_system.rs`, through the activity): Android 11 and
later are asked for the insets of `systemBars | ime` and the window is put
edge to edge with `setDecorFitsSystemWindows(false)`; older systems lay the
window out below the bars already and nothing is added.

Without that measurement the top app bar sits under the status bar and its
menu cannot be pressed, which is exactly what a first build without it did.

The icon colour of the bars follows the theme: dark icons on a light
interface, light ones on a dark one (`setSystemBarsAppearance`).

The insets are read again when the surface changes, so a rotation or the
soft keyboard moves the bars with it. Hiding the bars changes them without
touching the surface, so the request a reading was taken for is part of the
reading, and one that still reports the status bar is taken again until the
bars follow (up to forty frames, for a system that refuses): a stale
reading would keep the top app bar pushed down for as long as the phone
lies on its side.

## On its side

A phone on its side hides the system bars and draws edge to edge. The top
app bar sits on the very top edge of the screen, and the camera cutout in
the middle of a short edge is drawn over rather than kept clear of: the top
bar and the two rails run straight past it, so no icon is pushed in from
the edge and nothing is reserved beside it. The one room the bar still
keeps is what the cutout takes from the top edge itself, which is nothing
for a notch in the middle of an edge and its height for one in a corner. A
status bar that the system declines to hide is cleared as usual.

## The drawer, the pages and the sheets

The menu button opens a **navigation drawer** from the left, as an Android
app keeps its sections: the twelve menus, each opening a **page** of its
own with a back arrow, and Settings at the bottom. A page covers the whole
screen and scrolls; the back arrow, or the system's back gesture, returns
to where it came from.

Menus on a phone are drawn for a finger (`menus::set_touch`): rows 48
points tall with 15 point text, no access-key underlines (a phone has no
Alt), and submenus that open **under their row** by a tap, because a touch
screen has no pointer to hover one open with. That last part is what made
the menus unusable before: every submenu was a hover popup that never
opened.

Settings is the list of the desktop Options pages, grouped as the dialog
groups them; a page is drawn by `options::draw_page`, the same code the
desktop dialog uses, with its controls at 48 points. Every setting works
there as it does on the desktop.

The rest are **bottom sheets**: they slide over the drawing, 34 to 62 % of
the screen high, with a dimmed drawing behind them; a tap on the dimmed
part or the close button puts them away.

- **Properties**: the property bar of the selection, sliding sideways as it
  does in a narrow toolbar row (`overflow::row`).
- **Panels**: the docker column's tabs as a row of chips, the docker in
  front under them, at the sheet's width.
- **Palette**: the colour palette and the document's palette.
- **Files**: New and Open as filled buttons, Save and Save As as tonal
  ones, then a **file browser**: the folder it stands in, an arrow up, the
  folders and the drawings in it, a tap to walk in or to open. A dialog
  that asked for a file takes the tap as its answer.
- **Tools**: the other tools of a toolbox group, each with its icon and
  name; a tap picks it.
- **Drawings**: the open drawings, one button each.

## Files on a phone

Android gives a program no dialog of the system's without a good deal of
activity plumbing (the result of a system picker comes back through a
callback `android-activity` does not forward), so the file dialogs are the
interface's own: a browser over the folders the app may read.

Since Android 11 an app cannot even see its own `Android/data` folder from a
file manager, so drawings saved there could never be opened again by hand.
The app asks for **all files access** instead (`MANAGE_EXTERNAL_STORAGE`,
declared in the manifest, granted in a screen of the system's that the Files
sheet opens when it is missing) and then reads and writes the shared
folders: `/storage/emulated/0/Documents` once granted, its own folder
before that.

The dialogs themselves:

- `files::Dialog::pick_file`, `pick_files` and `pick_into` do not block:
  they leave a request that the Files sheet answers with the path the user
  taps, and the closure the caller handed over runs on the next frame. The
  same machinery the browser build uses (`files::run_finished`) runs it.
- `files::Dialog::save_path` answers a name in the documents folder,
  `Drawing (2).tdraw` when the name is taken, without asking. The desktop
  dialogs do the asking there.
- `files::pick_folder` answers the app's documents folder.

Preferences, logs and autosaves live in the app's internal folder, which
no file manager shows and nothing needs to.

Reading the phone's own clipboard needs the activity and a permission the
interface does not ask for yet: copy and paste work inside TraceDraw.

### A drawing tapped in a file manager

`.cdr` and `.tdraw` are registered with the system, so a tap in a file
manager opens TraceDraw with that drawing: the manifest lists the address a
manager may send (`file://` on old systems, `content://` since Android 7)
and the types it may give it, and since a type is `application/octet-stream`
for an extension the system has no table entry for, a filter takes any type
held to the extension by the address's path. The drawing is copied into the
app's folder and opened from there (`android_system::shared_drawing`).

### Fonts of your own

A phone installs no fonts, so the font lists would only ever hold the ones
a drawing carries (and the phone's own, from `/system/fonts`). Drop `.ttf`,
`.otf` or `.ttc` files into **`Fonts`** inside the shared **Download**
folder (`/storage/emulated/0/Download/Fonts`, the folder a chat app saves
to) and they join the lists: they are read at start, again when a drawing
is opened (so its text is not reported missing what it was set in), and
from **Reload fonts** in the Files sheet. A file already read is not read
again, so each of those costs a folder listing (`fonts_folder.rs`).

## Typing on a phone

`winit` 0.30 has no text input on Android: its backend forwards physical key
presses and nothing else, and `android-activity`'s native activity keeps no
text buffer to read (its `text_input_state` is empty and
`set_text_input_state` does nothing). A soft keyboard therefore has nothing
to type into, and a paragraph frame or a field in a dialog stayed empty.

So the app brings its own field: while a text field has the focus, or while
the canvas edits text (`app.text_edit`), a one pixel `EditText` is added to
the window, made invisible, focused and given the keyboard
(`android_system::set_keyboard`). The keyboard edits it as it would any
field; every frame reads what it holds and hands the difference to the
interface as `Event::Text` and `Backspace` key presses, so the canvas text
editing and every field of a dialog type as they do on a desktop. The field
is taken out of the window as soon as the typing stops.

The interface language follows the phone's, unless the user picked one in
Options: `Locale.getDefault().toLanguageTag()` (`zh-CN`, `pt-BR`) is read
through the activity as the app starts and handed to
`i18n::set_system_locale`, which `i18n::system_language` reads before the
`LANG` a desktop would have. Options > General > Language offers "Same as
the system" to go back to it.

## Building it

```
scripts/android.sh --release
```

The script needs the Android SDK (`ANDROID_HOME`), its NDK
(`ANDROID_NDK_ROOT`, or the newest under `$ANDROID_HOME/ndk`), the Rust
Android targets and `cargo install cargo-apk`. It writes
`target/release/apk/tracedraw.apk`.

A release APK must be signed and `cargo-apk` refuses to pack one without a
key: set `CARGO_APK_RELEASE_KEYSTORE` and
`CARGO_APK_RELEASE_KEYSTORE_PASSWORD`, or let the script fall back to the
debug keystore every Android tool installs (fine for trying a build on a
phone, not for publishing).

Two traps met on the way, worth knowing:

- `ANDROID_NDK_ROOT` must use forward slashes on Windows: with backslashes
  the tab escape of `\toolchains` arrives at the linker as a tab character
  and every link fails.
- `cargo-apk` looks for `[lib] crate-type = ["cdylib"]` and the
  `android_main` symbol; the crate that holds the interface is a plain
  library (`apps/tracedraw`), and `apps/tracedraw-android` is the shim that
  exports it.

## Checking it without a phone

`TRACEDRAW_MOBILE=1` lays a desktop build out the phone way, in a
412 by 915 point window (`TRACEDRAW_MOBILE_SIZE=915x412` for landscape),
and `scripts/visual/mobile_shot.ps1` starts it, clicks where a finger
would and captures the window. The clicks are in the window's own pixels.
The script makes itself DPI aware and puts the window on top first: without
both, the capture holds another window's pixels.

```
mobile_shot.ps1 -File sample.tdraw -Out shot.png -Clicks "36,34"
```

## Sizes and shapes

Targets are 48 points, whatever size the desktop was set to
(`ButtonSize::Large`), with 8 by 6 points between them, so a finger hits
what it aims at. A chosen destination sits in a 56 by 32 pill; a chosen
tool in a rounded square; buttons are 40 points high and fully rounded;
sheets have 28 point top corners and a drag handle. The colours are the
theme's own (`Tokens`), used in Material's roles: surface, container, on
surface, on surface variant, primary and primary container.

## Not done yet

- The system file picker (Storage Access Framework): import and export
  dialogs that need to reach files outside the app's folder.
- The system clipboard.
- Printing, PDF export through a print service, and the MCP bridge.
- A back-button handler: the system's back closes the activity, not the
  open sheet.
