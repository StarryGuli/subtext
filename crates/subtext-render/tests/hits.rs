//! 候选格的命中矩形：鼠标点选靠它找是哪一行，错了会上屏错的词。

use subtext_render::{FontLibrary, Frame, Layout, Renderer, Row, Theme};

fn frame() -> Frame {
    Frame {
        rows: vec![
            Row::plain(0, "双语"),
            Row::plain(1, "双鱼"),
            Row::plain(2, "爽约"),
        ],
        highlighted: Some(0),
        ..Frame::default()
    }
}

#[test]
fn vertical_rows_are_stacked_non_overlapping_and_inside_the_window() {
    let mut renderer = Renderer::new(FontLibrary::system("zh-CN").unwrap());
    let rendered = renderer
        .render(&frame(), Layout::Vertical, &Theme::light(), 2.0, None)
        .unwrap();
    let (width, height) = rendered.content_size_points();
    assert_eq!(rendered.hits.len(), 3);
    for (index, hit) in rendered.hits.iter().enumerate() {
        assert_eq!(hit.row, index);
        assert!(hit.x >= 0.0 && hit.x + hit.width <= width + 0.5, "{hit:?}");
        assert!(
            hit.y >= 0.0 && hit.y + hit.height <= height + 0.5,
            "{hit:?}"
        );
    }
    for pair in rendered.hits.windows(2) {
        assert!(pair[1].y >= pair[0].y + pair[0].height - 0.01, "{pair:?}");
    }
}

#[test]
fn horizontal_items_are_laid_out_left_to_right() {
    let mut renderer = Renderer::new(FontLibrary::system("zh-CN").unwrap());
    let rendered = renderer
        .render(&frame(), Layout::Horizontal, &Theme::light(), 2.0, None)
        .unwrap();
    assert_eq!(rendered.hits.len(), 3);
    for pair in rendered.hits.windows(2) {
        assert!(pair[1].x >= pair[0].x + pair[0].width - 0.01, "{pair:?}");
    }
}

#[test]
fn a_second_render_does_not_inherit_the_previous_frames_hits() {
    let mut renderer = Renderer::new(FontLibrary::system("zh-CN").unwrap());
    renderer
        .render(&frame(), Layout::Vertical, &Theme::light(), 2.0, None)
        .unwrap();
    let one_row = Frame {
        rows: vec![Row::plain(0, "好")],
        highlighted: Some(0),
        ..Frame::default()
    };
    let rendered = renderer
        .render(&one_row, Layout::Vertical, &Theme::light(), 2.0, None)
        .unwrap();
    assert_eq!(rendered.hits.len(), 1);
}
