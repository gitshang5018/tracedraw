# Batch banners

The batch banners panel: the chip at the left edge of a phone's drawing,
row **批量条幅** (Batch banners). It writes a list of slogans on banners of
their own, one banner per slogan, and leaves them on the page.

A banner is what a shop hangs over its door: a rectangle of colour with the
slogan written across it and the signature under it. A batch of them is the
same banner made of every line of a list, so a hundred slogans for a hundred
shops take one panel and one tap.

## The panel

| Field | Default | What it is |
|---|---|---|
| 标语 / Slogans | two lines | the slogans, one per line; an empty line is left out |
| 落款 / Signature | 示例商贸 | written under every slogan; empty means no signature |
| 宽度 / Width | 800 mm | the width of a banner |
| 高度 / Height | 240 mm | the height of a banner |
| 间距 / Gap | 40 mm | the room between two banners |
| 标语字号 / Slogan size | 72 pt | the size of the slogan |
| 落款字号 / Signature size | 28 pt | the size of the signature |
| 底色 / Background | dark red | the colour of the banner |
| 标语色 / Slogan | pale gold | the colour of the slogan |
| 落款色 / Signature | white | the colour of the signature |
| 底边色 / Band | deeper red | the colour of the band along the bottom |
| 标语竖排 / Vertical | off | the slogan is written down the banner instead of across it |

The slogans are edited as one block of text, one line to a banner, so a
whole list can be pasted in at once.

## What a banner is made of

Four objects:

1. **the rectangle** of the banner's colour, its width and height as told;
2. **the band** along the bottom, in the band colour, an eighth of the
   banner's height;
3. **the slogan**, in the slogan colour, centred in the room above the
   band;
4. **the signature**, in the signature colour, centred on the band.

The four are grouped, so the banner is picked up and moved as one thing
and can be taken apart again afterwards. The whole batch is one undo step.

## Where they go

The banners are laid out down the page from its top, one under the other,
each a banner's height and the gap below the one before it, and centred
across the page's width. A banner wider than the page hangs over both
sides, which is what a banner wider than the door is.

## The numbers

The width, the height and the gap are millimetres, and the letter sizes
are points. They are the drawing's own units: the sizes are converted
with the document's unit setting, and the sizes of the letters are points
whatever the unit, as text sizes are everywhere else in the program.

## Tests

- `banner::tests::a_batch_of_banners_is_a_group_for_each_slogan`
- `banner::tests::a_blank_line_is_not_a_banner`
- `banner::tests::a_banner_is_the_size_it_was_given`
- `banner::tests::the_banners_are_one_under_the_other`
- `banner::tests::a_banner_without_a_signature_has_three_shapes`
