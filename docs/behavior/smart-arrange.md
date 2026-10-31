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

The panel is in the drawing's own unit: the three numbers are millimetres
and are converted with the document's unit setting.

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

With the switch on, each part is measured by its own outline, simplified at
0.2 mm and taken as its convex hull. Twelve angles 15° apart are tried and
the one whose box is smallest is used, which is what a long thin part
gains from.

After the boxes are placed, every part is slid down and then left until its
outline comes within the spacing of another part or of the sheet's edge.
Two convex outlines sliding in a straight line overlap over one stretch of
travel, so the longest free slide is found by halving that range rather
than by stepping a millimetre at a time. Six passes, last placed first,
since the parts placed last are on top of the pile.

This is not a full no-fit-polygon nest: parts do not interlock by being
moved around each other, only by settling into the hollows below them. It
packs tighter than the boxes it starts from, and less tight than a
dedicated nesting program.

## Where the result goes

The first sheet's top left corner is the selection's own top left, so the
packed block stays where the work was. Each further sheet goes to the right
of the one before, a sheet's width plus the spacing apart.

Each part is moved, and turned when it was turned, by one transform per
object: the whole thing is one undo step, "Smart arrange".
