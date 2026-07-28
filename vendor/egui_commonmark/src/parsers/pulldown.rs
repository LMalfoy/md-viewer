use std::iter::Peekable;
use std::ops::Range;

use crate::{CommonMarkCache, CommonMarkOptions};

use egui::{self, Id, Pos2, TextStyle, Ui};

use crate::List;
use egui_commonmark_backend::elements::*;
use egui_commonmark_backend::misc::*;
use egui_commonmark_backend::pulldown::*;
use pulldown_cmark::{Alignment, CowStr, HeadingLevel};

const BODY_LINE_HEIGHT_FACTOR: f32 = 1.45;

/// Newline logic is constructed by the following:
/// All elements try to insert a newline before them (if they are allowed)
/// and end their own line.
struct Newline {
    /// Whether a newline should not be inserted before a widget. This is only for
    /// the first widget.
    should_not_start_newline_forced: bool,
    /// Whether an element should insert a newline before it
    should_start_newline: bool,
    /// Whether an element should end it's own line using a newline
    /// This will have to be set to false in cases such as when blocks are within
    /// a list.
    should_end_newline: bool,
    /// only false when the widget is the last one.
    should_end_newline_forced: bool,
}

impl Default for Newline {
    fn default() -> Self {
        Self {
            should_not_start_newline_forced: true,
            should_start_newline: true,
            should_end_newline: true,
            should_end_newline_forced: true,
        }
    }
}

impl Newline {
    pub fn can_insert_end(&self) -> bool {
        self.should_end_newline && self.should_end_newline_forced
    }

    pub fn can_insert_start(&self) -> bool {
        self.should_start_newline && !self.should_not_start_newline_forced
    }

    pub fn try_insert_start(&self, ui: &mut Ui) {
        if self.can_insert_start() {
            newline(ui);
        }
    }

    pub fn try_insert_end(&self, ui: &mut Ui) {
        if self.can_insert_end() {
            newline(ui);
        }
    }
}

#[derive(Default)]
struct DefinitionList {
    is_first_item: bool,
    is_def_list_def: bool,
}

pub struct CommonMarkViewerInternal {
    text_style: Style,
    list: List,
    link: Option<Link>,
    image: Option<Image>,
    line: Newline,
    code_block: Option<CodeBlock>,

    /// Only populated if the html_fn option has been set
    html_block: String,
    is_list_item: bool,
    def_list: DefinitionList,
    is_table: bool,
    table_alignments: Vec<Alignment>,
    is_blockquote: bool,
    checkbox_events: Vec<CheckboxClickEvent>,
    deferred_scroll_to_heading: Option<String>,
}

pub(crate) struct CheckboxClickEvent {
    pub(crate) checked: bool,
    pub(crate) span: Range<usize>,
}

impl CommonMarkViewerInternal {
    pub fn new() -> Self {
        Self {
            text_style: Style::default(),
            list: List::default(),
            link: None,
            image: None,
            line: Newline::default(),
            is_list_item: false,
            def_list: Default::default(),
            code_block: None,
            html_block: String::new(),
            is_table: false,
            table_alignments: Vec::new(),
            is_blockquote: false,
            checkbox_events: Vec::new(),
            deferred_scroll_to_heading: None,
        }
    }
}

fn parser_options_extras(
    is_math_enabled: bool,
    is_scroll_to_heading_enabled: bool,
) -> pulldown_cmark::Options {
    let mut result = parser_options();
    if is_math_enabled {
        result |= pulldown_cmark::Options::ENABLE_MATH;
    }
    if is_scroll_to_heading_enabled {
        result |= pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;
    }
    result
}

impl CommonMarkViewerInternal {
    /// Be aware that this acquires egui::Context internally.
    /// If split Id is provided then split points will be populated
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
        split_points_id: Option<Id>,
    ) -> (egui::InnerResponse<()>, Vec<CheckboxClickEvent>) {
        let max_width = options.max_width(ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        let re = ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            let mut events = pulldown_cmark::Parser::new_ext(
                text,
                parser_options_extras(options.math_fn.is_some(), options.enable_scroll_to_heading),
            )
            .into_offset_iter()
            .enumerate()
            .peekable();

            while let Some((index, (e, src_span))) = events.next() {
                let start_position = ui.next_widget_position();
                let is_element_end = matches!(e, pulldown_cmark::Event::End(_));
                let should_add_split_point = self.list.is_inside_a_list() && is_element_end;

                if events.peek().is_none() {
                    self.line.should_end_newline_forced = false;
                }

                self.process_event(ui, &mut events, e, src_span, cache, options, max_width);

                if let Some(source_id) = split_points_id
                    && should_add_split_point
                {
                    let scroll_cache = scroll_cache(cache, &source_id);
                    let end_position = ui.next_widget_position();

                    let split_point_exists = scroll_cache
                        .split_points
                        .iter()
                        .any(|(i, _, _)| *i == index);

                    if !split_point_exists {
                        scroll_cache
                            .split_points
                            .push((index, start_position, end_position));
                    }
                }

                if index == 0 {
                    self.line.should_not_start_newline_forced = false;
                }
            }

            // deferral to make it consistent no matter whether the target is before or after the link
            *cache.scroll_to_id_target_mut() = self.deferred_scroll_to_heading.take();

            if let Some(source_id) = split_points_id {
                scroll_cache(cache, &source_id).page_size =
                    Some(ui.next_widget_position().to_vec2());
            }
        });

        (re, std::mem::take(&mut self.checkbox_events))
    }

    pub(crate) fn show_scrollable(
        &mut self,
        source_id: Id,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
    ) {
        let available_size = ui.available_size();
        let scroll_id = source_id.with("_scroll_area");

        let Some(page_size) = scroll_cache(cache, &source_id).page_size else {
            egui::ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.show(ui, cache, options, text, Some(source_id));
                });
            // Prevent repopulating points twice at startup
            scroll_cache(cache, &source_id).available_size = available_size;
            return;
        };

        let events = pulldown_cmark::Parser::new_ext(
            text,
            parser_options_extras(options.math_fn.is_some(), options.enable_scroll_to_heading),
        )
        .into_offset_iter()
        .collect::<Vec<_>>();

        let num_rows = events.len();

        egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            // Elements have different widths, so the scroll area cannot try to shrink to the
            // content, as that will mean that the scroll bar will move when loading elements
            // with different widths.
            .auto_shrink([false, true])
            .show_viewport(ui, |ui, viewport| {
                ui.set_height(page_size.y);
                let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

                let max_width = options.max_width(ui);
                ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let scroll_cache = scroll_cache(cache, &source_id);

                    // finding the first element that's not in the viewport anymore
                    let (first_event_index, _, first_end_position) = scroll_cache
                        .split_points
                        .iter()
                        .filter(|(_, _, end_position)| end_position.y < viewport.min.y)
                        .nth_back(1)
                        .copied()
                        .unwrap_or((0, Pos2::ZERO, Pos2::ZERO));

                    // finding the last element that's just outside the viewport
                    let last_event_index = scroll_cache
                        .split_points
                        .iter()
                        .filter(|(_, start_position, _)| start_position.y > viewport.max.y)
                        .nth(1)
                        .map(|(index, _, _)| *index)
                        .unwrap_or(num_rows);

                    ui.allocate_space(first_end_position.to_vec2());

                    // only rendering the elements that are inside the viewport
                    let mut events = events
                        .into_iter()
                        .enumerate()
                        .skip(first_event_index)
                        .take(last_event_index - first_event_index)
                        .peekable();

                    while let Some((i, (e, src_span))) = events.next() {
                        if events.peek().is_none() {
                            self.line.should_end_newline_forced = false;
                        }

                        self.process_event(ui, &mut events, e, src_span, cache, options, max_width);

                        if i == 0 {
                            self.line.should_not_start_newline_forced = false;
                        }
                    }
                });
            });

        // Forcing full re-render to repopulate split points for the new size
        let scroll_cache = scroll_cache(cache, &source_id);
        if available_size != scroll_cache.available_size {
            scroll_cache.available_size = available_size;
            scroll_cache.page_size = None;
            scroll_cache.split_points.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_event<'e>(
        &mut self,
        ui: &mut Ui,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        self.event(ui, event, src_span, cache, options, max_width);

        self.def_list_def_wrapping(events, max_width, cache, options, ui);
        self.item_list_wrapping(events, max_width, cache, options, ui);
        self.table(events, cache, options, ui, max_width);
        self.blockquote(events, max_width, cache, options, ui);
    }

    fn def_list_def_wrapping<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.def_list.is_def_list_def {
            self.def_list.is_def_list_def = false;

            let item_events = delayed_events(events, |tag| {
                matches!(tag, pulldown_cmark::TagEnd::DefinitionListDefinition)
            });

            let mut events_iter = item_events.into_iter().enumerate().peekable();

            self.line.try_insert_start(ui);

            // Proccess a single event separately so that we do not insert spaces where we do not
            // want them
            self.line.should_start_newline = false;
            if let Some((_, (e, src_span))) = events_iter.next() {
                self.process_event(ui, &mut events_iter, e, src_span, cache, options, max_width);
            }

            ui.label(" ".repeat(options.indentation_spaces));
            self.line.should_start_newline = true;
            self.line.should_end_newline = false;
            // Required to ensure that the content is aligned with the identation
            ui.horizontal_wrapped(|ui| {
                while let Some((_, (e, src_span))) = events_iter.next() {
                    self.process_event(
                        ui,
                        &mut events_iter,
                        e,
                        src_span,
                        cache,
                        options,
                        max_width,
                    );
                }
            });
            self.line.should_end_newline = true;

            // Only end the definition items line if it is not the last element in the list
            if !matches!(
                events.peek(),
                Some((
                    _,
                    (
                        pulldown_cmark::Event::End(pulldown_cmark::TagEnd::DefinitionList),
                        _
                    )
                ))
            ) {
                self.line.try_insert_end(ui);
            }
        }
    }

    fn item_list_wrapping<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.is_list_item {
            self.is_list_item = false;

            let item_events = delayed_events_list_item(events);
            let is_task = item_events
                .iter()
                .take_while(|(event, _)| {
                    matches!(
                        event,
                        pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph)
                            | pulldown_cmark::Event::TaskListMarker(_)
                    )
                })
                .any(|(event, _)| matches!(event, pulldown_cmark::Event::TaskListMarker(_)));
            let mut events_iter = item_events.into_iter().enumerate().peekable();

            // Give every item the full readable width and keep wrapped lines
            // aligned beneath the item text rather than beneath its marker.
            ui.horizontal_top(|ui| {
                self.list.show_item_marker(ui, options, is_task);
                let item_width = ui.available_width().min(max_width);
                let layout = egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true);
                ui.allocate_ui_with_layout(egui::vec2(item_width, 0.0), layout, |ui| {
                    ui.set_width(item_width);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    while let Some((_, (e, src_span))) = events_iter.next() {
                        self.process_event(
                            ui,
                            &mut events_iter,
                            e,
                            src_span,
                            cache,
                            options,
                            item_width,
                        );
                    }
                });
            });
        }
    }

    fn blockquote<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.is_blockquote {
            let mut collected_events = delayed_events(events, |tag| {
                matches!(tag, pulldown_cmark::TagEnd::BlockQuote(_))
            });
            self.line.try_insert_start(ui);

            // Currently the blockquotes are made in such a way that they need a newline at the end
            // and the start so when this is the first element in the markdown the newline must be
            // manually enabled
            self.line.should_not_start_newline_forced = false;
            if let Some(alert) = parse_alerts(&options.alerts, &mut collected_events) {
                egui_commonmark_backend::alert_ui(alert, ui, |ui| {
                    for (event, src_span) in collected_events {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                })
            } else {
                blockquote(ui, ui.visuals().weak_text_color(), |ui| {
                    self.text_style.quote = true;
                    for (event, src_span) in collected_events {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                    self.text_style.quote = false;
                });
            }

            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            self.line.try_insert_end(ui);
            self.is_blockquote = false;
        }
    }

    fn table<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
        max_width: f32,
    ) {
        if self.is_table {
            self.line.try_insert_start(ui);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                let Table { header, rows } = parse_table(events);
                let column_count = header
                    .len()
                    .max(rows.iter().map(Vec::len).max().unwrap_or(0))
                    .max(1);
                let available_width = ui.available_width().min(max_width);
                let desired_spacing = ui.spacing().item_spacing.x;
                let column_spacing = desired_spacing
                    .min(available_width / (column_count as f32 * 4.0))
                    .max(1.0);
                let gaps = column_spacing * column_count.saturating_sub(1) as f32;
                let usable_width = (available_width - gaps).max(column_count as f32);
                let mut column_weights = vec![8_usize; column_count];
                for row in std::iter::once(&header).chain(rows.iter()) {
                    for (column_index, cell) in row.iter().enumerate() {
                        let visible_characters = cell
                            .iter()
                            .map(|(event, _)| match event {
                                pulldown_cmark::Event::Text(text)
                                | pulldown_cmark::Event::Code(text)
                                | pulldown_cmark::Event::InlineHtml(text)
                                | pulldown_cmark::Event::Html(text)
                                | pulldown_cmark::Event::InlineMath(text)
                                | pulldown_cmark::Event::DisplayMath(text)
                                | pulldown_cmark::Event::FootnoteReference(text) => {
                                    text.chars().count()
                                }
                                pulldown_cmark::Event::TaskListMarker(_) => 3,
                                _ => 0,
                            })
                            .sum::<usize>()
                            .clamp(8, 80);
                        column_weights[column_index] =
                            column_weights[column_index].max(visible_characters);
                    }
                }
                let total_weight = column_weights.iter().sum::<usize>() as f32;
                let column_widths = column_weights
                    .into_iter()
                    .map(|weight| usable_width * weight as f32 / total_weight)
                    .collect::<Vec<_>>();
                let alignments = std::mem::take(&mut self.table_alignments);

                ui.vertical(|ui| {
                    ui.set_width(available_width);
                    self.table_row(
                        ui,
                        header,
                        &column_widths,
                        column_spacing,
                        true,
                        true,
                        &alignments,
                        cache,
                        options,
                    );

                    for (row_index, row) in rows.into_iter().enumerate() {
                        self.table_row(
                            ui,
                            row,
                            &column_widths,
                            column_spacing,
                            false,
                            row_index % 2 == 1,
                            &alignments,
                            cache,
                            options,
                        );
                    }
                });
            });

            self.is_table = false;
            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            self.line.try_insert_end(ui);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn table_row<'e>(
        &mut self,
        ui: &mut Ui,
        row: Vec<Vec<(pulldown_cmark::Event<'e>, Range<usize>)>>,
        column_widths: &[f32],
        column_spacing: f32,
        header: bool,
        shaded: bool,
        alignments: &[Alignment],
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
    ) {
        let mut cells = row.into_iter();
        let background = if shaded {
            ui.visuals().faint_bg_color
        } else {
            egui::Color32::TRANSPARENT
        };

        egui::Frame::NONE.fill(background).show(ui, |ui| {
            let vertical_padding = (ui.spacing().item_spacing.y * 0.25).max(2.0);
            ui.add_space(vertical_padding);
            ui.spacing_mut().item_spacing.x = column_spacing;
            ui.horizontal_top(|ui| {
                for (column_index, &cell_width) in column_widths.iter().enumerate() {
                    let cell = cells.next().unwrap_or_default();
                    let main_align = match alignments
                        .get(column_index)
                        .copied()
                        .unwrap_or(Alignment::None)
                    {
                        Alignment::Center => egui::Align::Center,
                        Alignment::Right => egui::Align::Max,
                        Alignment::None | Alignment::Left => egui::Align::Min,
                    };
                    let layout = egui::Layout::left_to_right(egui::Align::Min)
                        .with_main_align(main_align)
                        .with_main_wrap(true);
                    let previous_strong = self.text_style.strong;

                    ui.allocate_ui_with_layout(egui::vec2(cell_width, 0.0), layout, |ui| {
                        ui.set_width(cell_width);
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        self.text_style.strong = previous_strong || header;
                        for (event, src_span) in cell {
                            let previous_start =
                                std::mem::replace(&mut self.line.should_start_newline, false);
                            let previous_end =
                                std::mem::replace(&mut self.line.should_end_newline, false);
                            self.event(ui, event, src_span, cache, options, cell_width);
                            self.line.should_start_newline = previous_start;
                            self.line.should_end_newline = previous_end;
                        }
                        self.text_style.strong = previous_strong;
                    });
                }
            });
            ui.add_space(vertical_padding);
        });
    }

    fn event(
        &mut self,
        ui: &mut Ui,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, cache, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(ui, tag, cache, options, max_width),
            pulldown_cmark::Event::Text(text) => {
                self.event_text(text, ui);
            }
            pulldown_cmark::Event::Code(text) => {
                self.text_style.code = true;
                self.event_text(text, ui);
                self.text_style.code = false;
            }
            pulldown_cmark::Event::InlineHtml(text) => {
                self.event_text(text, ui);
            }

            pulldown_cmark::Event::Html(text) => {
                if options.html_fn.is_some() {
                    self.html_block.push_str(&text);
                } else {
                    self.event_text(text, ui);
                }
            }
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                footnote_start(ui, &footnote);
            }
            pulldown_cmark::Event::SoftBreak => {
                soft_break(ui);
            }
            pulldown_cmark::Event::HardBreak => newline(ui),
            pulldown_cmark::Event::Rule => {
                self.line.try_insert_start(ui);
                rule(ui, self.line.can_insert_end());
            }
            pulldown_cmark::Event::TaskListMarker(mut checkbox) => {
                if options.mutable {
                    if ui
                        .add(egui::Checkbox::without_text(&mut checkbox))
                        .clicked()
                    {
                        self.checkbox_events.push(CheckboxClickEvent {
                            checked: checkbox,
                            span: src_span,
                        });
                    }
                } else {
                    ui.add(ImmutableCheckbox::without_text(&mut checkbox));
                }
            }
            pulldown_cmark::Event::InlineMath(tex) => {
                if let Some(math_fn) = options.math_fn {
                    math_fn(ui, &tex, true);
                }
            }
            pulldown_cmark::Event::DisplayMath(tex) => {
                if let Some(math_fn) = options.math_fn {
                    math_fn(ui, &tex, false);
                }
            }
        }
    }

    fn event_text(&mut self, text: CowStr, ui: &mut Ui) {
        let mut rich_text = self.text_style.to_richtext(ui, &text);
        // Headings keep their font metrics; forcing the body height could clip large glyphs.
        if self.text_style.heading.is_none() {
            let body_size = TextStyle::Body.resolve(ui.style()).size;
            rich_text = rich_text.line_height(Some(body_size * BODY_LINE_HEIGHT_FACTOR));
        }
        if let Some(image) = &mut self.image {
            image.alt_text.push(rich_text);
        } else if let Some(block) = &mut self.code_block {
            block.content.push_str(&text);
        } else if let Some(link) = &mut self.link {
            link.text.push(rich_text);
        } else {
            ui.label(rich_text);
        }
    }

    fn start_tag(
        &mut self,
        ui: &mut Ui,
        tag: pulldown_cmark::Tag,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
    ) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                self.line.try_insert_start(ui);
            }
            pulldown_cmark::Tag::Heading { level, id, .. } => {
                if let Some(scroll_target) = cache.scroll_to_id_target()
                    && let Some(id) = id
                    && id.into_string() == scroll_target
                {
                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                    cache.scroll_to_id_target_mut().take();
                }

                // Headings should always insert a newline even if it is at the start.
                // Whether this is okay in all scenarios is a different question.
                newline(ui);
                self.text_style.heading = Some(match level {
                    HeadingLevel::H1 => 0,
                    HeadingLevel::H2 => 1,
                    HeadingLevel::H3 => 2,
                    HeadingLevel::H4 => 3,
                    HeadingLevel::H5 => 4,
                    HeadingLevel::H6 => 5,
                });
            }

            // deliberately not using the built in alerts from pulldown-cmark as
            // the markdown itself cannot be localized :( e.g: [!TIP]
            pulldown_cmark::Tag::BlockQuote(_) => {
                self.is_blockquote = true;
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                match c {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        self.code_block = Some(crate::CodeBlock {
                            lang: Some(lang.to_string()),
                            content: "".to_string(),
                        });
                    }
                    pulldown_cmark::CodeBlockKind::Indented => {
                        self.code_block = Some(crate::CodeBlock {
                            lang: None,
                            content: "".to_string(),
                        });
                    }
                }
                self.line.try_insert_start(ui);
            }

            pulldown_cmark::Tag::List(point) => {
                if !self.list.is_inside_a_list() && self.line.can_insert_start() {
                    newline(ui);
                }

                if let Some(number) = point {
                    self.list.start_level_with_number(number);
                } else {
                    self.list.start_level_without_number();
                }
                self.line.should_start_newline = false;
                self.line.should_end_newline = false;
            }

            pulldown_cmark::Tag::Item => {
                self.is_list_item = true;
                self.list.start_item(ui);
            }

            pulldown_cmark::Tag::FootnoteDefinition(note) => {
                self.line.try_insert_start(ui);

                self.line.should_start_newline = false;
                self.line.should_end_newline = false;
                footnote(ui, &note);
            }
            pulldown_cmark::Tag::Table(alignments) => {
                self.is_table = true;
                self.table_alignments = alignments;
            }
            pulldown_cmark::Tag::TableHead => {}
            pulldown_cmark::Tag::TableRow => {}
            pulldown_cmark::Tag::TableCell => {}
            pulldown_cmark::Tag::Emphasis => {
                self.text_style.emphasis = true;
            }
            pulldown_cmark::Tag::Strong => {
                self.text_style.strong = true;
            }
            pulldown_cmark::Tag::Strikethrough => {
                self.text_style.strikethrough = true;
            }
            pulldown_cmark::Tag::Link { dest_url, .. } => {
                self.link = Some(crate::Link {
                    destination: dest_url.to_string(),
                    text: Vec::new(),
                });
            }
            pulldown_cmark::Tag::Image { dest_url, .. } => {
                self.image = Some(crate::Image::new(&dest_url, options));
            }
            pulldown_cmark::Tag::HtmlBlock => {
                self.line.try_insert_start(ui);
            }
            pulldown_cmark::Tag::MetadataBlock(_) => {}

            pulldown_cmark::Tag::DefinitionList => {
                self.line.try_insert_start(ui);
                self.def_list.is_first_item = true;
            }
            pulldown_cmark::Tag::DefinitionListTitle => {
                // we disable newline as the first title should not insert a newline
                // as we have already done that upon the DefinitionList Tag
                if !self.def_list.is_first_item {
                    self.line.try_insert_start(ui)
                } else {
                    self.def_list.is_first_item = false;
                }
            }
            pulldown_cmark::Tag::DefinitionListDefinition => {
                self.def_list.is_def_list_def = true;
            }
            // Not yet supported
            pulldown_cmark::Tag::Superscript | pulldown_cmark::Tag::Subscript => {}
        }
    }

    fn end_tag(
        &mut self,
        ui: &mut Ui,
        tag: pulldown_cmark::TagEnd,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        match tag {
            pulldown_cmark::TagEnd::Paragraph => {
                self.line.try_insert_end(ui);
            }
            pulldown_cmark::TagEnd::Heading { .. } => {
                self.line.try_insert_end(ui);
                self.text_style.heading = None;
            }
            pulldown_cmark::TagEnd::BlockQuote(_) => {}
            pulldown_cmark::TagEnd::CodeBlock => {
                self.end_code_block(ui, cache, options, max_width);
            }

            pulldown_cmark::TagEnd::List(_) => {
                if self.list.is_last_level() {
                    self.line.should_start_newline = true;
                    self.line.should_end_newline = true;
                }

                self.list.end_level(ui, self.line.can_insert_end());

                if !self.list.is_inside_a_list() {
                    // Reset all the state and make it ready for the next list that occurs
                    self.list = List::default();
                }
            }
            pulldown_cmark::TagEnd::Item => {}
            pulldown_cmark::TagEnd::FootnoteDefinition => {
                self.line.should_start_newline = true;
                self.line.should_end_newline = true;
                self.line.try_insert_end(ui);
            }
            pulldown_cmark::TagEnd::Table => {}
            pulldown_cmark::TagEnd::TableHead => {}
            pulldown_cmark::TagEnd::TableRow => {}
            pulldown_cmark::TagEnd::TableCell => {
                // Ensure space between cells
                ui.label("  ");
            }
            pulldown_cmark::TagEnd::Emphasis => {
                self.text_style.emphasis = false;
            }
            pulldown_cmark::TagEnd::Strong => {
                self.text_style.strong = false;
            }
            pulldown_cmark::TagEnd::Strikethrough => {
                self.text_style.strikethrough = false;
            }
            pulldown_cmark::TagEnd::Link => {
                if let Some(link) = self.link.take() {
                    link.end(ui, cache, options, &mut self.deferred_scroll_to_heading);
                }
            }
            pulldown_cmark::TagEnd::Image => {
                if let Some(image) = self.image.take() {
                    image.end(ui, options);
                }
            }
            pulldown_cmark::TagEnd::HtmlBlock => {
                if let Some(html_fn) = options.html_fn {
                    html_fn(ui, &self.html_block);
                    self.html_block.clear();
                }
            }

            pulldown_cmark::TagEnd::MetadataBlock(_) => {}

            pulldown_cmark::TagEnd::DefinitionList => self.line.try_insert_end(ui),
            pulldown_cmark::TagEnd::DefinitionListTitle
            | pulldown_cmark::TagEnd::DefinitionListDefinition => {}
            pulldown_cmark::TagEnd::Superscript | pulldown_cmark::TagEnd::Subscript => {}
        }
    }

    fn end_code_block(
        &mut self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        if let Some(block) = self.code_block.take() {
            block.end(ui, cache, options, max_width);
            self.line.try_insert_end(ui);
        }
    }
}
