use iced::widget::container;
use iced::{Background, Border, Theme};

pub fn header_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        text_color: Some(palette.background.weak.text),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn pane_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.base.color)),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}
