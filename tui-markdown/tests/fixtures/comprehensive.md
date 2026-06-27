---
title: Comprehensive Markdown Test
author: tui-markdown
version: 1.0
---

# Heading 1

## Heading 2

### Heading 3

#### Heading 4

##### Heading 5

###### Heading 6

## Heading with attributes {#custom-id .highlight data-level=2}

## Text Formatting

**Bold text**, *italic text*, ~~strikethrough~~, `inline code`, H ~2~ O (subscript), E=mc ^2^ (superscript).

Combined: ***bold italic***, **bold with `code`**, ~~strikethrough with *italic*~~

## Links and Images

[Example Link](https://example.com)

[Link with **bold**](https://example.com)

![Alt text for image](https://example.com/image.png)

![](https://example.com/no-alt.png)

Text before ![photo](https://example.com/photo.png "Title") text after.

## Lists

### Unordered

- Item 1
- Item 2
- Item 3

### Ordered

1. First
2. Second
3. Third

### Nested

- Parent
  - Child 1
  - Child 2
    - Grandchild

### Task Lists

- [ ] Unchecked task
- [x] Checked task

1. [ ] Ordered incomplete
2. [x] Ordered complete

## Blockquotes

> Simple blockquote

> First paragraph.
>
> Second paragraph.

> Outer quote
>> Nested quote

## GFM Alerts

> [!NOTE]
> This is a note with useful information.

> [!TIP]
> This is a helpful tip.

> [!IMPORTANT]
> This is important information.

> [!WARNING]
> This is a warning message.

> [!CAUTION]
> This is a caution about something dangerous.

## Tables

| Name  | Age | City |
|-------|----:|:----:|
| Alice |  30 | NYC  |
| Bob   |  25 | LA   |

| Left   | Center | Right |
|:-------|:------:|------:|
| L      |   C    |     R |

## Code Blocks

```rust
fn main() {
    println!("Hello, world!");
}
```

```python
def hello():
    print("Hello!")
```

```
plain code block
no language specified
```

## Math

Inline math: $E = mc^2$

Display math:

$$
\sum_{i=1}^{n} x_i = x_1 + x_2 + \cdots + x_n
$$

## Footnotes

Here is a footnote reference[^1].

Another reference[^note].

[^1]: This is the first footnote definition.

[^note]: This is a named footnote.

## Definition Lists

Term 1
: Definition for term 1

Term 2
: Definition for term 2
: Another definition for term 2

## HTML

<div>This is an HTML block</div>

Inline <b>HTML</b> content.

## Horizontal Rules

Text above.

---

Text below.

## Paragraphs and Line Breaks

This is the first paragraph with a soft
break in the middle.

This is the second paragraph.

## Complex Nesting

**Bold text with *italic inside* and `code` too**

> A blockquote with **bold**, *italic*, and `code` formatting.

- List with **bold** item
- List with *italic* item
- List with `code` item
- List with [link](https://example.com)

## Edge Cases

Single word paragraph.



Multiple blank lines above (should collapse).

A very long line that should still render correctly even though it extends beyond the typical terminal width and contains many words that might need to be wrapped or truncated depending on the rendering context.

---

*End of comprehensive test*
