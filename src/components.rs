use crate::style;
use iced::widget::{container, row, text};
use iced::{Background, Border, Element, Length, Theme};

pub fn table_header<'a, Message: 'a>(titles: Vec<&'a str>) -> Element<'a, Message> {
    let mut r = row![];
    for title in titles {
        r = r.push(
            container(text(title).size(12).font(iced::font::Font::MONOSPACE))
                .width(Length::Fill)
                .padding(5)
                .style(|t: &Theme| {
                    let palette = t.extended_palette();
                    container::Style {
                        background: Some(Background::Color(palette.background.strong.color)),
                        border: Border {
                            width: 1.0,
                            color: palette.background.base.color,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }
                }),
        );
    }
    r.spacing(1).into()
}

pub fn table_row<'a, Message: 'a>(values: Vec<String>) -> Element<'a, Message> {
    let mut r = row![];
    for val in values {
        r = r.push(
            container(text(val).size(12))
                .width(Length::Fill)
                .padding(5)
                .style(style::pane_style),
        );
    }
    r.spacing(1).into()
}
