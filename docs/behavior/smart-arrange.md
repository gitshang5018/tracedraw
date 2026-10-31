# Smart arrange

The smart arrange panel: the chip at the left edge of a phone's drawing,
row **智能排布** (Smart arrange). It packs the selected objects onto sheets
of a size the panel is told, and leaves them packed.

## The numbers

| Field | Default | What it is |
|---|---|---|
| 宽度 / Width | 1200 mm | the sheet's width |
| 高度 / Height | 2400 mm | the sheet's height |
| 间距 / Spacing | 10 mm | the least room left between two parts, and between a part and the sheet's edge |
| 允许旋转 / Allow rotation | on | a part may be turned a quarter turn to make it fit |
| 异形拼版 / Nest by outline | off | a part is turned to the angle its own box is smallest at, and then slid down and left until its outline meets something |
| 自动拆分 / Split groups first | off | the groups in the selection are opened before the packing, so each member is a part of its own |

The panel is in the drawing's own unit: the three numbers are millimetres
and are converted with the document's unit setting.

## Split groups first

A group is packed as one part, and the room between its members is then
wasted: a word turned into curves and left as one group nests as a block of
letters rather than as letters. With the switch on, every group in the
selection is opened before the packing, and its members are what is placed.

The opening is repeated while groups remain, so a group inside a group comes
apart too, and it stops at four hundred pieces: a drawing nested that deeply
is left as it is rather than taken apart into thousands of shapes, and the
status bar says why.

Curves are not taken apart by this. Splitting a letter into its outline and
the hole in it would leave the hole free to be nested somewhere else, which
is not what a cut sheet wants; that split stays a command of its own.

## How the boxes are placed

Maximal free rectangles. A sheet starts as one free rectangle; each part
takes the free rectangle a heuristic likes best, and every free rectangle
it overlaps is split around it and the pieces another rectangle already
holds are dropped.

Four heuristics and three orders are all run, and the run that fills the
fewest sheets wins; ties go to the run whose last sheet is filled the
least deep:

| Heuristic | Picks the free rectangle with |
|---|---|
| short side | the smallest leftover on its shorter side |
| long side | the smallest leftover on its longer side |
| area | the least area left over |
| bottom left | the lowest top edge, then the leftmost |

| Order | Largest first by |
|---|---|
| area | width × height |
| long side | the longer side |
| width | width |

Spacing is carried by the parts rather than by the sheet: a part is packed
as its size plus the spacing, so neighbours keep their distance without the
packer remembering anything.

A part too big for an empty sheet still gets a sheet, at its corner, rather
than vanishing.

## Nest by outline

With the switch on, the shapes themselves are packed rather than the boxes
around them, by no-fit polygon.

**Parts.** Each outline is simplified at 0.2 mm and cut into convex parts:
ears are taken off one at a time and the triangles merged back while the
merge stays convex.

**Forbidden region.** For two convex parts, the translations that make them
overlap are their no-fit polygon, which is the Minkowski sum of one part and
the other mirrored: start at the sum of their lowest corners, then walk both
rings' edges in order of angle. A piece's forbidden region against
everything already placed is the union of those, one per pair of parts, which
is why a piece can be placed in the hollow of another rather than only beside
it.

**Where it goes.** The lowest free place is always a corner of the sheet or a
vertex of the forbidden region on it, so the search takes those points, sorts
them by height and then by distance from the left edge, and keeps the first
that is not inside any no-fit polygon. A point on the boundary counts as
free, which is what lets parts touch.

**Spacing** is carried by the geometry rather than checked for: each part is
grown by half the spacing before the no-fit polygons are worked out, as its
Minkowski sum with a square of that half side, so two parts that touch after
growing are exactly the spacing apart.

**Angles.** A part is tried at eighth turns as well as quarter ones, since a
shape with a long diagonal in it uses the sheet better lying along that
diagonal, and the placement that comes out lowest wins.

**Holes.** What is packed is the material: each outline with its holes taken
out. Every hole is cut open into its outline by a slit of no width (from the
hole's rightmost vertex to the outline), the ring is cut into convex parts,
and those parts are used only when their area is the outline's less the
holes'. Otherwise the holes are filled in, which wastes the room inside them
but never packs a part into another part. Because the hole is not material,
a part nested inside the counter of an O is placed there by the same search
as any other.

**Resting.** The first free candidate is not where a part is left. It is slid
towards the sheet's top left corner until it meets something, the first
contact along a straight line being found exactly rather than stepped
towards, and then left, and round again. That is what puts a part exactly
against another one, including where two pieces meet at a corner that is not
a vertex of either no-fit polygon.

## Where the result goes

The first sheet's top left corner is the selection's own top left, so the
packed block stays where the work was. Each further sheet goes to the right
of the one before, a sheet's width plus the spacing apart.

Each part is moved, and turned when it was turned, by one transform per
object: the whole thing is one undo step, "Smart arrange".
