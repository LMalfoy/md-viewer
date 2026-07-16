# MD Viewer showcase

This document exercises the main rendering features. The application is
**read-only**, native, and the window can be resized freely.

> [!NOTE]
> Markdown is rendered locally. Remote images are not downloaded automatically.

## Text formatting

Regular text can contain **bold**, *italic*, ~~strikethrough~~, and
[`links`](https://commonmark.org/).

### Lists and tasks

- A normal list item
- Another item
  - A nested item

1. First numbered item
2. Second numbered item

- [x] Viewer opens Markdown
- [x] Viewer stays read-only
- [ ] Add more documents

Task checks are temporary and reset when the file is reloaded.

### Table

| Feature | Shortcut | State |
|---|---:|:---:|
| Open file | `Ctrl+O` | Ready |
| Reload | `Ctrl+R` / `F5` | Ready |
| Zoom | `Ctrl++` / `Ctrl+-` | Ready |

### Code

```rust
fn main() {
    println!("Hello from a rendered code block!");
}
```

### Local image

The icon below is loaded from a path relative to this Markdown file:

![MD Viewer icon](../assets/md-viewer.svg)

---

Resize the window and the document will reflow to the available width.
