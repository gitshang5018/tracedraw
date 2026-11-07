# Batch banners

The batch banners panel: the chip at the left edge of a phone's drawing,
row **批量条幅** (Batch banners). It writes a list of slogans on banners of
their own, one banner per slogan, and generates production-ready banners.

A banner is what a shop hangs over its door or across an avenue: standard roll widths,
clean slogans, tying margins, and grommet eyelets for hanging. A batch of them turns
a list of slogans into a complete production run with a single tap.

## The panel

| Field | Default | What it is |
|---|---|---|
| 标语 / Slogans | multiline text | the slogans, one per line; empty lines are skipped |
| 布料门幅 / Roll width | 700 mm | standard cloth roll height presets: 400, 500, 600, 700, 800, 900, 1000, 1200 mm |
| 长度模式 / Length mode | 按字数自适应 | auto length by character count or fixed total length |
| 两端留白 / Side margin | 400 mm | margin on each side for tying ropes |
| 上下边距 / Top/bottom | 50 mm | top and bottom safety margin |
| 文字间距 / Letter spacing | 50 mm | spacing between characters |
| 颜色方案 / Colors | 红底黄字 | classic sign shop presets (Red/Yellow, Red/White, Blue/White, White/Red, White/Black, Yellow/Red, Custom) |
| 标语标点 / Punctuation | 转空格 | clean punctuation: replace with space, strip, or keep |
| 输出方式 / Output | 独立页面 | independent page per banner with exact cloth dimensions, or stacked on current canvas |
| 索环打孔 / Grommets | off | 12 mm eyelet marks at corners and every 1.2 m for punching |
| 落款 / Signature | (optional) | signature under every slogan |

## Layout rules

- **Length formula:** Total width in auto mode = $2 \times \text{Margin}_{LR} + \sum W_i + (N - 1) \times \text{Spacing}$.
- **Character size:** Character height defaults to 75% of available cloth height ($H - 2 \times \text{Margin}_{TB}$).
- **Output:** Each banner is created as an independent page with page size matching the banner dimensions ($W \times H$).

## Tests

- `banner::tests::a_batch_of_banners_is_a_group_for_each_slogan`
- `banner::tests::a_blank_line_is_not_a_banner`
- `banner::tests::clean_slogan_replaces_punctuation`
- `banner::tests::multi_page_output_generates_pages`
